//! TUI shell — terminal lifecycle, event loop, and top-level dispatch.
//!
//! Terminal lifecycle:
//! - ratatui::init() handles raw mode, alternate screen, AND panic hook atomically.
//! - ratatui::restore() undoes all of it — always called even on error.
//!
//! Security:
//! - PAT token never logged. Client passed to API calls only, not stored in TUI state.

pub mod app;
pub mod screens;

use std::time::Duration;

use anyhow::Context;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseEventKind,
};
use crossterm::execute;
use tokio::sync::{mpsc, oneshot};

use crate::api::comment::{list_comments, Comment};
use crate::api::page::{
    get_page_detail, list_all_pages, update_page, ContentType, Page, PageDetail,
};
use crate::api::{get_space_detail, list_all_spaces, resolve_webui_url, AppError, Client, Space};
use crate::config;
use crate::tui::app::{
    App, CommentsBrowseState, KeyAction, PagesBrowseState, Screen, StatusMessage, StatusStyle,
};
use crate::tui::screens::comments::render_comments;
use crate::tui::screens::pages::render_pages;
use crate::tui::screens::spaces::render_spaces;

/// TUI entry point. Called by main when no subcommand is given.
pub async fn run() -> anyhow::Result<()> {
    let config = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context(
            "No configuration found. Run 'ccli init' to configure before launching the TUI.",
        )?;

    let client = Client::new(&config)?;
    let base_url = client.base_url().to_string();

    // Spawn space list fetch before entering the TUI; Client clone is cheap (Arc-backed).
    let (spaces_tx, spaces_rx) = oneshot::channel::<Result<Vec<Space>, AppError>>();
    let fetch_client = client.clone();
    tokio::spawn(async move {
        let result = list_all_spaces(&fetch_client).await;
        let _ = spaces_tx.send(result);
    });

    // Enter alternate screen + raw mode + install panic hook
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);

    let result = run_app(&mut terminal, client, base_url, spaces_rx).await;

    // Always restore terminal, even on error (T-02-21)
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();

    result
}

/// Main event loop. Runs until user quits.
///
/// Uses crossterm::event::poll(100ms) to drive both key events and the spinner tick.
/// API fetch results arrive via channels from tokio tasks.
async fn run_app(
    terminal: &mut ratatui::DefaultTerminal,
    client: Client,
    base_url: String,
    spaces_rx: oneshot::Receiver<Result<Vec<Space>, AppError>>,
) -> anyhow::Result<()> {
    let mut app = App::new();

    // Load spaces from disk cache immediately so the TUI shows data without waiting for the
    // network. The background network fetch (spaces_rx) always runs and updates the list when
    // it completes, so cached data is at most 5 minutes stale.
    let mut spaces_from_cache = false;
    if let Some(cached) = crate::cache::load_spaces(&base_url) {
        app.set_spaces(cached);
        spaces_from_cache = true;
    }

    // Wrap in Option so we stop polling once the channel delivers its value.
    let mut spaces_rx: Option<oneshot::Receiver<Result<Vec<Space>, AppError>>> = Some(spaces_rx);

    // Channel for preview detail fetches — buffer 4 to avoid blocking spawned tasks
    let (preview_tx, mut preview_rx) =
        tokio::sync::mpsc::channel::<(String, Result<crate::api::space::SpaceDetail, AppError>)>(4);

    // Phase 3: pages list fetch results — keyed by space_key so multiple drill-downs
    // can be in flight concurrently. Buffer 4 (same as spaces preview).
    let (pages_list_tx, mut pages_list_rx) =
        mpsc::channel::<(String, Result<Vec<Page>, AppError>)>(4);

    // Phase 4: comments list fetch results — keyed by page_id (D-48).
    let (comments_list_tx, mut comments_list_rx) =
        mpsc::channel::<(String, Result<Vec<Comment>, AppError>)>(4);

    // Phase 3: pages preview detail fetches — keyed by page_id (D-43).
    let (pages_preview_tx, mut pages_preview_rx) =
        mpsc::channel::<(String, Result<PageDetail, AppError>)>(4);

    loop {
        terminal.draw(|f| match app.screen_stack.last_mut() {
            Some(Screen::CommentsBrowse { state, .. }) => {
                render_comments(f, state.as_mut(), f.area());
            }
            Some(Screen::PagesBrowse { state, .. }) => {
                render_pages(f, state.as_mut(), f.area());
            }
            None | Some(Screen::SpacesBrowse) => {
                render_spaces(f, &mut app, f.area());
            }
        })?;

        // Check for completed space list fetch; set to None after receiving to stop polling.
        if let Some(rx) = spaces_rx.as_mut() {
            if let Ok(fetch_result) = rx.try_recv() {
                spaces_rx = None; // stop polling
                match fetch_result {
                    Ok(spaces) => {
                        let base_clone = base_url.clone();
                        let spaces_for_cache = spaces.clone();
                        tokio::task::spawn_blocking(move || {
                            crate::cache::save_spaces(&base_clone, &spaces_for_cache);
                        });
                        if spaces_from_cache {
                            app.refresh_spaces(spaces);
                        } else {
                            app.set_spaces(spaces);
                        }
                    }
                    Err(e) => {
                        if !spaces_from_cache {
                            app.set_fetch_error(&e);
                        }
                        // With cached data showing, network errors are silently ignored.
                    }
                }
            }
        }

        // Check for completed preview fetch (non-blocking try_recv)
        // preview errors are non-fatal; pane shows previous content
        if let Ok((key, Ok(detail))) = preview_rx.try_recv() {
            app.cache_detail(key, detail);
        }

        // Drain pages list fetch results.
        while let Ok((space_key, fetch_result)) = pages_list_rx.try_recv() {
            // Find the matching PagesBrowse screen (most recently pushed) and update.
            for screen in app.screen_stack.iter_mut().rev() {
                if let Screen::PagesBrowse {
                    space_key: sk,
                    state,
                    ..
                } = screen
                {
                    if *sk == space_key {
                        match fetch_result {
                            Ok(pages) => {
                                let had_cached = !state.pages.is_empty();
                                let base_clone = base_url.clone();
                                let sk_clone = space_key.clone();
                                let pages_for_cache = pages.clone();
                                tokio::task::spawn_blocking(move || {
                                    crate::cache::save_pages(
                                        &base_clone,
                                        &sk_clone,
                                        &pages_for_cache,
                                    );
                                });
                                if had_cached {
                                    state.refresh_pages(pages);
                                } else {
                                    state.set_pages(pages);
                                }
                            }
                            Err(ref e) => {
                                if state.pages.is_empty() {
                                    state.set_fetch_error(e);
                                }
                                // With cached data showing, network errors are silently ignored.
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Drain pages preview fetch results.
        while let Ok((page_id, Ok(detail))) = pages_preview_rx.try_recv() {
            if let Some(Screen::PagesBrowse { state, .. }) = app.screen_stack.last_mut() {
                state.cache_detail(page_id, detail);
            }
        }

        // Drain comments list fetch results.
        while let Ok((page_id, fetch_result)) = comments_list_rx.try_recv() {
            for screen in app.screen_stack.iter_mut().rev() {
                if let Screen::CommentsBrowse {
                    page_id: pid,
                    state,
                } = screen
                {
                    if *pid == page_id {
                        match fetch_result {
                            Ok(comments) => state.set_comments(comments),
                            Err(ref e) => state.set_fetch_error(e),
                        }
                        break;
                    }
                }
            }
        }

        // Spawn pending preview fetch (set by tick() after 150ms debounce).
        if let Some(key) = app.pending_preview_key.take() {
            let fetch_client = client.clone();
            let tx = preview_tx.clone();
            let k = key.clone();
            tokio::spawn(async move {
                let result = get_space_detail(&fetch_client, &k).await;
                let _ = tx.send((k, result)).await;
            });
        }

        if let Some(Screen::PagesBrowse { state, .. }) = app.screen_stack.last_mut() {
            state.tick();
            if let Some(id) = state.pending_preview_id.take() {
                let fetch_client = client.clone();
                let tx = pages_preview_tx.clone();
                let id_tag = id.clone();
                tokio::spawn(async move {
                    let result = get_page_detail(&fetch_client, &id_tag).await;
                    let _ = tx.send((id_tag, result)).await;
                });
            }
        }

        if let Some(Screen::CommentsBrowse { state, .. }) = app.screen_stack.last_mut() {
            state.tick();
        }

        // Poll for key/mouse events with 100ms timeout (drives spinner tick).
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Mouse(mouse) => {
                    // Fall back to 80×24 on error rather than 0×0, which would make
                    // split_col = 0 and misroute all scroll events to the preview pane.
                    let size = terminal.size().unwrap_or(ratatui::layout::Size {
                        width: 80,
                        height: 24,
                    });
                    // Body split: left 40% = list pane, right 60% = preview pane
                    let split_col = size.width * 40 / 100;
                    let in_preview = mouse.column >= split_col;
                    let scroll_down = matches!(mouse.kind, MouseEventKind::ScrollDown);
                    let scroll_up = matches!(mouse.kind, MouseEventKind::ScrollUp);
                    if scroll_down || scroll_up {
                        match app.screen_stack.last_mut() {
                            Some(Screen::PagesBrowse { state, .. }) => {
                                if in_preview {
                                    if scroll_down {
                                        state.preview_scroll =
                                            state.preview_scroll.saturating_add(3);
                                    } else {
                                        state.preview_scroll =
                                            state.preview_scroll.saturating_sub(3);
                                    }
                                } else if scroll_down {
                                    state.select_next();
                                } else {
                                    state.select_prev();
                                }
                            }
                            _ => {
                                if !in_preview {
                                    if scroll_down {
                                        app.select_next();
                                    } else {
                                        app.select_prev();
                                    }
                                }
                            }
                        }
                    }
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    // Route the keypress to the correct handler depending on the top
                    // of screen_stack. PagesBrowse and CommentsBrowse delegate to their
                    // own state structs; empty stack or SpacesBrowse uses the App handler.
                    let action = match app.screen_stack.last_mut() {
                        Some(Screen::CommentsBrowse { state, .. }) => state.handle_key(key.code),
                        Some(Screen::PagesBrowse { state, .. }) => state.handle_key(key.code),
                        _ => app.handle_key(key.code),
                    };

                    match action {
                        KeyAction::Quit => break,
                        KeyAction::OpenBrowser(payload) => {
                            // PagesBrowse: payload is a webui PATH — resolve and open directly.
                            // SpacesBrowse / empty: payload is a space key — resolve from app.spaces.
                            if matches!(app.screen_stack.last(), Some(Screen::PagesBrowse { .. })) {
                                handle_open_page_browser(&payload, &base_url, terminal)?;
                            } else {
                                handle_open_browser(&mut app, &payload, &base_url, terminal)?;
                            }
                        }
                        KeyAction::DrillDown(space_key) => {
                            // Populate from disk cache immediately if available, then spawn network fetch.
                            let new_state = Box::new(PagesBrowseState::new(
                                space_key.clone(),
                                ContentType::Page,
                            ));
                            app.screen_stack.push(Screen::PagesBrowse {
                                space_key: space_key.clone(),
                                content_type: ContentType::Page,
                                state: new_state,
                            });
                            if let Some(cached_pages) =
                                crate::cache::load_pages(&base_url, &space_key)
                            {
                                if let Some(Screen::PagesBrowse { state, .. }) =
                                    app.screen_stack.last_mut()
                                {
                                    state.set_pages(cached_pages);
                                }
                            }
                            let fetch_client = client.clone();
                            let tx = pages_list_tx.clone();
                            let key_tag = space_key.clone();
                            tokio::spawn(async move {
                                let result =
                                    list_all_pages(&fetch_client, &key_tag, ContentType::Page)
                                        .await;
                                let _ = tx.send((key_tag, result)).await;
                            });
                        }
                        KeyAction::PopScreen => {
                            app.screen_stack.pop();
                        }
                        KeyAction::EditPage(page_id) => {
                            // Read content_type from the active screen so blog posts are updated as "blogpost".
                            let ct = app
                                .screen_stack
                                .last()
                                .and_then(|s| {
                                    if let Screen::PagesBrowse { content_type, .. } = s {
                                        Some(*content_type)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(ContentType::Page);
                            handle_edit_page(&mut app, &client, &page_id, ct, terminal).await;
                        }
                        KeyAction::DrillDownComments(page_id) => {
                            let new_state = Box::new(CommentsBrowseState::new(page_id.clone()));
                            app.screen_stack.push(Screen::CommentsBrowse {
                                page_id: page_id.clone(),
                                state: new_state,
                            });
                            let fetch_client = client.clone();
                            let tx = comments_list_tx.clone();
                            let id_tag = page_id.clone();
                            tokio::spawn(async move {
                                let result = list_comments(&fetch_client, &id_tag).await;
                                let _ = tx.send((id_tag, result)).await;
                            });
                        }
                        KeyAction::None => {}
                    }
                }
                _ => {}
            }
        } else {
            app.tick();
        }
    }

    Ok(())
}

/// Open the selected space in the system browser.
///
/// URL is resolved from _links.webui via resolve_webui_url — never hand-built.
/// SSH/headless fallback: if open::that() fails, suspend TUI, print URL to stderr,
/// wait for a keypress, then re-enter TUI.
fn handle_open_browser(
    app: &mut App,
    space_key: &str,
    base_url: &str,
    terminal: &mut ratatui::DefaultTerminal,
) -> anyhow::Result<()> {
    let space = app
        .list_state
        .selected()
        .and_then(|i| app.filtered_indices.get(i))
        .and_then(|&idx| app.spaces.get(idx));

    let url = match space {
        Some(s) => match &s.links.webui {
            Some(webui) => resolve_webui_url(base_url, webui),
            None => {
                app.error = Some(format!("No URL available for space {}", space_key));
                return Ok(());
            }
        },
        None => return Ok(()),
    };

    match open::that(&url) {
        Ok(_) => {}
        Err(_) => {
            // SSH/headless fallback: suspend TUI, print URL, await keypress, restore.
            ratatui::restore();
            eprintln!("Browser unavailable — open this URL manually:");
            eprintln!("  {}", url);
            eprintln!();
            eprintln!("Press any key to continue.");
            loop {
                if event::poll(Duration::from_secs(60))? {
                    if let Event::Key(k) = event::read()? {
                        if k.kind == KeyEventKind::Press {
                            break;
                        }
                    }
                } else {
                    // 60s timeout with no input — give up waiting and re-enter TUI
                    break;
                }
            }

            // Re-enter TUI
            *terminal = ratatui::init();
        }
    }

    Ok(())
}

/// Open a page's webui URL in the system browser.
///
/// Unlike `handle_open_browser`, the path comes directly from the API response
/// and is joined with `base_url` here to produce the absolute URL.
fn handle_open_page_browser(
    webui_path: &str,
    base_url: &str,
    terminal: &mut ratatui::DefaultTerminal,
) -> anyhow::Result<()> {
    let url = resolve_webui_url(base_url, webui_path);
    match open::that(&url) {
        Ok(_) => Ok(()),
        Err(_) => {
            // SSH/headless fallback: suspend TUI, print URL, await keypress, restore.
            ratatui::restore();
            eprintln!("Browser unavailable — open this URL manually:");
            eprintln!("  {}", url);
            eprintln!();
            eprintln!("Press any key to continue.");
            loop {
                if event::poll(Duration::from_secs(60))? {
                    if let Event::Key(k) = event::read()? {
                        if k.kind == KeyEventKind::Press {
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
            *terminal = ratatui::init();
            Ok(())
        }
    }
}

/// Suspend TUI, fetch current XML, open $EDITOR, PUT to Confluence, restore TUI.
///
/// Errors are surfaced via `state.status_message` on the top PagesBrowse screen.
/// The TUI is ALWAYS restored on exit, even on error.
async fn handle_edit_page(
    app: &mut App,
    client: &Client,
    page_id: &str,
    content_type: ContentType,
    terminal: &mut ratatui::DefaultTerminal,
) {
    // Validate id is digits-only to prevent path traversal in /tmp paths.
    if page_id.is_empty() || !page_id.chars().all(|c| c.is_ascii_digit()) {
        set_pages_status(
            app,
            "Invalid page id (must be numeric)".to_string(),
            StatusStyle::Error,
        );
        return;
    }

    // Fetch detail BEFORE suspending — keeps TUI responsive on slow networks.
    let detail = match get_page_detail(client, page_id).await {
        Ok(d) => d,
        Err(e) => {
            set_pages_status(
                app,
                format!("Error fetching page: {}", e),
                StatusStyle::Error,
            );
            return;
        }
    };
    let title = detail.title.clone();
    let current_version = match detail.version.as_ref().map(|v| v.number) {
        Some(v) => v,
        None => {
            set_pages_status(
                app,
                "Page version missing from API — cannot safely update.".to_string(),
                StatusStyle::Error,
            );
            return;
        }
    };
    let space_key = match extract_space_key_from_webui(&detail) {
        Some(k) => k,
        None => {
            set_pages_status(
                app,
                "Could not determine space key from page _links.webui".to_string(),
                StatusStyle::Error,
            );
            return;
        }
    };
    let body_xml = detail
        .body
        .as_ref()
        .and_then(|b| b.storage.as_ref())
        .and_then(|s| s.value.clone())
        .unwrap_or_default();

    let using_pandoc = crate::edit::pandoc_available();

    ratatui::restore();

    // Always restore the TUI before returning, no matter what fails.
    let editor_result = crate::edit::run_edit_session(&body_xml, page_id, false);
    *terminal = ratatui::init();

    let new_xml = match editor_result {
        Ok(s) => s,
        Err(e) => {
            set_pages_status(app, format!("Editor failed: {}", e), StatusStyle::Error);
            return;
        }
    };

    let put_result = update_page(
        client,
        page_id,
        &title,
        &space_key,
        &new_xml,
        content_type,
        current_version,
    )
    .await;

    let ext = if using_pandoc { "md" } else { "xml" };
    let temp_path = std::env::temp_dir().join(format!("ccli-edit-{}.{}", page_id, ext));

    match put_result {
        Ok(()) => {
            let _ = std::fs::remove_file(&temp_path);
            let msg = if using_pandoc {
                "Saved.".to_string()
            } else {
                "Saved. (install pandoc for Markdown editing)".to_string()
            };
            set_pages_status(app, msg, StatusStyle::Success);
        }
        Err(AppError::Api(msg)) if msg.starts_with("Conflict:") => {
            // Preserve the temp file so the user can recover their edits.
            set_pages_status(
                app,
                format!(
                    "{} Your edits saved at {} — re-open to merge manually.",
                    msg,
                    temp_path.display()
                ),
                StatusStyle::Error,
            );
        }
        Err(e) => {
            set_pages_status(app, format!("Error saving: {}", e), StatusStyle::Error);
        }
    }
}

/// Extract the space key from a PageDetail.
///
/// Strategy (most to least reliable):
/// 1. `detail.space.key` — returned by the API by default, works for Cloud and DC.
/// 2. URL parsing of `_links.webui`:
///    - DC format:    `/display/SPACE_KEY/Page+Title`
///    - Cloud format: `/spaces/SPACE_KEY/pages/123/Page+Title`
///      or            `/wiki/spaces/SPACE_KEY/pages/123/Page+Title`
fn extract_space_key_from_webui(detail: &PageDetail) -> Option<String> {
    if let Some(key) = detail.space.as_ref().map(|s| s.key.clone()) {
        return Some(key);
    }
    let webui = detail.links.webui.as_deref()?;
    let mut segs = webui.split('/').filter(|s| !s.is_empty());
    while let Some(seg) = segs.next() {
        if seg == "display" || seg == "spaces" {
            return segs.next().map(|s| s.to_string());
        }
    }
    None
}

/// Set status_message on the top PagesBrowse screen. No-op if the top is not PagesBrowse.
fn set_pages_status(app: &mut App, text: String, style: StatusStyle) {
    if let Some(Screen::PagesBrowse { state, .. }) = app.screen_stack.last_mut() {
        state.status_message = Some(StatusMessage { text, style });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::page::{PageDetail, PageLinks, PageSpace};

    fn detail_with_webui(webui: &str) -> PageDetail {
        PageDetail {
            id: "1".into(),
            title: "T".into(),
            content_type: "page".into(),
            version: None,
            ancestors: None,
            body: None,
            space: None,
            links: PageLinks {
                webui: Some(webui.to_string()),
                self_url: None,
            },
        }
    }

    fn detail_with_space_key(key: &str) -> PageDetail {
        PageDetail {
            id: "1".into(),
            title: "T".into(),
            content_type: "page".into(),
            version: None,
            ancestors: None,
            body: None,
            space: Some(PageSpace {
                key: key.to_string(),
            }),
            links: PageLinks {
                webui: None,
                self_url: None,
            },
        }
    }

    #[test]
    fn extract_space_key_from_webui_returns_segment_after_display() {
        assert_eq!(
            extract_space_key_from_webui(&detail_with_webui("/display/DEV/Page+Title")),
            Some("DEV".to_string())
        );
    }

    #[test]
    fn extract_space_key_from_webui_returns_none_for_unknown_path() {
        assert_eq!(
            extract_space_key_from_webui(&detail_with_webui("/pages/viewpage.action?pageId=1")),
            None
        );
    }

    #[test]
    fn extract_space_key_from_webui_handles_long_titles_with_slashes() {
        // Confluence URL-encodes spaces as '+', not '/', but defensive handling here
        assert_eq!(
            extract_space_key_from_webui(&detail_with_webui("/display/PROJ/Long+Title")),
            Some("PROJ".to_string())
        );
    }

    #[test]
    fn extract_space_key_prefers_space_field_over_webui_parsing() {
        let mut detail = detail_with_webui("/display/WRONG/Title");
        detail.space = Some(PageSpace {
            key: "CORRECT".to_string(),
        });
        assert_eq!(
            extract_space_key_from_webui(&detail),
            Some("CORRECT".to_string())
        );
    }

    #[test]
    fn extract_space_key_uses_space_field_when_webui_absent() {
        assert_eq!(
            extract_space_key_from_webui(&detail_with_space_key("MYSPACE")),
            Some("MYSPACE".to_string())
        );
    }

    #[test]
    fn extract_space_key_from_webui_cloud_spaces_format() {
        assert_eq!(
            extract_space_key_from_webui(&detail_with_webui("/spaces/TEAM/pages/123/My+Page")),
            Some("TEAM".to_string())
        );
    }

    #[test]
    fn extract_space_key_from_webui_cloud_wiki_spaces_format() {
        assert_eq!(
            extract_space_key_from_webui(&detail_with_webui("/wiki/spaces/TEAM/pages/123/My+Page")),
            Some("TEAM".to_string())
        );
    }

    #[test]
    fn run_editor_session_returns_error_when_editor_fails() {
        // Set EDITOR to a binary that does not exist; the call must produce an Err.
        std::env::set_var("EDITOR", "this-binary-does-not-exist-xyzzy-12345");
        let result = crate::edit::run_edit_session("<p>test</p>", "99998", true);
        std::env::remove_var("EDITOR");
        assert!(
            result.is_err(),
            "expected editor launch failure to surface as Err"
        );
    }
}
