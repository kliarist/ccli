//! TUI shell — terminal lifecycle, event loop, and top-level dispatch.
//!
//! Locked decisions implemented:
//! - D-10: bare `ccli` launches this TUI on the Spaces browse view
//! - D-12: `q` always quits; `Esc` closes overlays (filter/modal) or quits at top-level
//! - D-13: static help bar + `?` modal
//! - D-14: pre-fetch all spaces on launch with spinner
//! - D-19: 150ms debounce before preview fetch; session cache in App
//! - D-22: use `open` crate for browser launch
//! - D-23: SSH/headless fallback — suspend TUI, print URL, await keypress, restore
//! - D-24: URL from _links.webui via resolve_webui_url()
//! - D-30: Enter drills into PagesBrowse; Esc pops back to SpacesBrowse
//! - D-31: screen_stack render dispatch (Plan 06)
//! - D-32: Enter == o on PagesBrowse → open page URL
//! - D-41: e on PagesBrowse → suspend TUI, $EDITOR, PUT, restore
//! - D-43: 150ms debounce + preview cache on PagesBrowse (Plan 06)
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
use crossterm::event::{self, Event, KeyEventKind};
use tokio::sync::{mpsc, oneshot};

use crate::api::{
    get_space_detail, list_all_spaces, resolve_webui_url, AppError, Client, Space,
};
use crate::api::page::{
    get_page_detail, list_all_pages, update_page, ContentType, Page, PageDetail,
};
use crate::config;
use crate::tui::app::{
    App, KeyAction, PagesBrowseState, Screen, StatusMessage, StatusStyle,
};
use crate::tui::screens::pages::render_pages;
use crate::tui::screens::spaces::render_spaces;

/// TUI entry point. Called by main.rs when no subcommand is given (D-10).
///
/// Lifecycle:
/// 1. Load config (error if missing — user must run `ccli init` first)
/// 2. Build Client
/// 3. Spawn async task to pre-fetch all spaces (D-14)
/// 4. ratatui::init() — raw mode + alt screen + panic hook
/// 5. Run event loop (async, using spawn_blocking for poll to avoid blocking tokio executor)
/// 6. ratatui::restore() — always, even on error
pub async fn run() -> anyhow::Result<()> {
    let config = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure before launching the TUI.")?;

    let client = Client::new(&config)?;
    let base_url = client.base_url().to_string();

    // Spawn the initial space list fetch before entering the TUI (D-14).
    // Client is cloned (cheap — reqwest::Client is Arc-backed).
    let (spaces_tx, spaces_rx) = oneshot::channel::<Result<Vec<Space>, AppError>>();
    let fetch_client = client.clone();
    tokio::spawn(async move {
        let result = list_all_spaces(&fetch_client).await;
        let _ = spaces_tx.send(result);
    });

    // Enter alternate screen + raw mode + install panic hook
    let mut terminal = ratatui::init();

    // Run the event loop — poll() is synchronous so we keep it on the async executor
    // thread, which is acceptable for a single-user CLI (RESEARCH.md Pitfall 6 /
    // Open Question 3). API fetches run in their own tokio::spawn tasks and communicate
    // via channels.
    let result = run_app(&mut terminal, client, base_url, spaces_rx).await;

    // Always restore terminal, even on error (T-02-21)
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
    mut spaces_rx: oneshot::Receiver<Result<Vec<Space>, AppError>>,
) -> anyhow::Result<()> {
    let mut app = App::new();

    // Channel for preview detail fetches — buffer 4 to avoid blocking spawned tasks
    let (preview_tx, mut preview_rx) = tokio::sync::mpsc::channel::<(
        String,
        Result<crate::api::space::SpaceDetail, AppError>,
    )>(4);

    // Phase 3: pages list fetch results — keyed by space_key so multiple drill-downs
    // can be in flight concurrently. Buffer 4 (same as spaces preview).
    let (pages_list_tx, mut pages_list_rx) =
        mpsc::channel::<(String, Result<Vec<Page>, AppError>)>(4);

    // Phase 3: pages preview detail fetches — keyed by page_id (D-43).
    let (pages_preview_tx, mut pages_preview_rx) =
        mpsc::channel::<(String, Result<PageDetail, AppError>)>(4);

    loop {
        // D-31: route render based on top of screen_stack.
        terminal.draw(|f| match app.screen_stack.last_mut() {
            Some(Screen::PagesBrowse { state, .. }) => {
                render_pages(f, state.as_mut(), f.area());
            }
            None | Some(Screen::SpacesBrowse) => {
                render_spaces(f, &mut app, f.area());
            }
        })?;

        // Check for completed space list fetch (non-blocking try_recv)
        if let Ok(fetch_result) = spaces_rx.try_recv() {
            match fetch_result {
                Ok(spaces) => app.set_spaces(spaces),
                Err(e) => app.set_fetch_error(&e),
            }
        }

        // Check for completed preview fetch (non-blocking try_recv)
        // preview errors are non-fatal; pane shows previous content
        if let Ok((key, Ok(detail))) = preview_rx.try_recv() {
            app.cache_detail(key, detail);
        }

        // Phase 3: drain pages list fetch results (D-30, PAGE-01)
        while let Ok((space_key, fetch_result)) = pages_list_rx.try_recv() {
            // Find the matching PagesBrowse screen (most recently pushed) and update.
            for screen in app.screen_stack.iter_mut().rev() {
                if let Screen::PagesBrowse { space_key: sk, state, .. } = screen {
                    if *sk == space_key {
                        match fetch_result {
                            Ok(pages) => state.set_pages(pages),
                            Err(ref e) => state.set_fetch_error(e),
                        }
                        break;
                    }
                }
            }
        }

        // Phase 3: drain pages preview fetch results (D-43)
        while let Ok((page_id, Ok(detail))) = pages_preview_rx.try_recv() {
            if let Some(Screen::PagesBrowse { state, .. }) = app.screen_stack.last_mut() {
                state.cache_detail(page_id, detail);
            }
        }

        // If a preview fetch is pending (set by tick() after 150ms debounce), spawn it (D-19)
        if let Some(key) = app.pending_preview_key.take() {
            let fetch_client = client.clone();
            let tx = preview_tx.clone();
            let k = key.clone();
            tokio::spawn(async move {
                let result = get_space_detail(&fetch_client, &k).await;
                let _ = tx.send((k, result)).await;
            });
        }

        // Phase 3: tick pages screen + dispatch pending preview fetch
        if let Some(Screen::PagesBrowse { state, .. }) = app.screen_stack.last_mut() {
            // Tick: advance spinner, fire debounce → set pending_preview_id
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

        // Poll for key events with 100ms timeout (drives spinner tick, RESEARCH.md Pitfall 4)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Route the keypress to the correct handler depending on the top
                    // of screen_stack. PagesBrowse delegates to its own state struct;
                    // empty stack or SpacesBrowse uses the App handler (Phase 2 flow).
                    let action = if let Some(Screen::PagesBrowse { state, .. }) =
                        app.screen_stack.last_mut()
                    {
                        state.handle_key(key.code)
                    } else {
                        app.handle_key(key.code)
                    };

                    match action {
                        KeyAction::Quit => break,
                        KeyAction::OpenBrowser(payload) => {
                            // Two contexts:
                            //   - Top is PagesBrowse: payload is a webui PATH (D-32, WR-02).
                            //     Resolve and open directly here.
                            //   - Top is SpacesBrowse / empty: payload is a space key.
                            //     Existing handle_open_browser resolves it from app.spaces.
                            if matches!(
                                app.screen_stack.last(),
                                Some(Screen::PagesBrowse { .. })
                            ) {
                                handle_open_page_browser(&payload, &base_url, terminal)?;
                            } else {
                                handle_open_browser(&mut app, &payload, &base_url, terminal)?;
                            }
                        }
                        KeyAction::DrillDown(space_key) => {
                            // D-30: push PagesBrowse and spawn list_all_pages fetch
                            let new_state = Box::new(PagesBrowseState::new(
                                space_key.clone(),
                                ContentType::Page,
                            ));
                            app.screen_stack.push(Screen::PagesBrowse {
                                space_key: space_key.clone(),
                                content_type: ContentType::Page,
                                state: new_state,
                            });
                            let fetch_client = client.clone();
                            let tx = pages_list_tx.clone();
                            let key_tag = space_key.clone();
                            tokio::spawn(async move {
                                let result = list_all_pages(
                                    &fetch_client,
                                    &key_tag,
                                    ContentType::Page,
                                )
                                .await;
                                let _ = tx.send((key_tag, result)).await;
                            });
                        }
                        KeyAction::PopScreen => {
                            // D-30: Esc on PagesBrowse → pop back to SpacesBrowse
                            app.screen_stack.pop();
                        }
                        KeyAction::EditPage(page_id) => {
                            // D-41: suspend TUI, run editor, PUT, restore TUI
                            // CR-02: read content_type from the active screen so blog posts
                            // are updated as "blogpost", not "page".
                            let ct = app.screen_stack.last()
                                .and_then(|s| if let Screen::PagesBrowse { content_type, .. } = s {
                                    Some(*content_type)
                                } else { None })
                                .unwrap_or(ContentType::Page);
                            handle_edit_page(&mut app, &client, &page_id, ct, terminal).await;
                        }
                        KeyAction::None => {}
                    }
                }
            }
        } else {
            // Timeout — advance spinner frame and check 150ms debounce (D-19)
            app.tick();
        }
    }

    Ok(())
}

/// Open the selected space in the system browser (D-22, D-23, D-24).
///
/// URL is resolved from _links.webui (D-24) via resolve_webui_url — never hand-built.
///
/// SSH/headless fallback (D-23): if open::that() fails, suspend TUI, print URL to stderr,
/// wait for a keypress, then re-enter TUI.
///
/// Security: URL is a Confluence page URL joined from base_url (config) + webui (API).
/// URL is passed to the OS browser via open::that(), not to a shell (T-02-19).
fn handle_open_browser(
    app: &mut App,
    space_key: &str,
    base_url: &str,
    terminal: &mut ratatui::DefaultTerminal,
) -> anyhow::Result<()> {
    // Resolve the absolute URL from _links.webui (D-24) — never hand-build from key
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
            // D-23: SSH/headless fallback — suspend TUI, print URL to stderr, await keypress, restore
            ratatui::restore();
            eprintln!("Browser unavailable — open this URL manually:");
            eprintln!("  {}", url);
            eprintln!();
            eprintln!("Press any key to continue.");

            // Block-read one keypress (URL is non-sensitive Confluence page URL — T-02-17)
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

/// Open a page's webui URL in the system browser (D-22, D-23, D-24).
///
/// Unlike `handle_open_browser` which resolves a space key against `app.spaces`,
/// the pages variant receives the webui PATH directly from `PagesBrowseState`
/// (which read it from the API response — D-24). The path is joined with
/// `base_url` here to produce the absolute URL.
fn handle_open_page_browser(
    webui_path: &str,
    base_url: &str,
    terminal: &mut ratatui::DefaultTerminal,
) -> anyhow::Result<()> {
    let url = resolve_webui_url(base_url, webui_path);
    match open::that(&url) {
        Ok(_) => Ok(()),
        Err(_) => {
            // D-23: SSH/headless fallback — suspend TUI, print URL, await keypress, restore.
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

/// D-41 editor workflow: suspend TUI, fetch current XML, $EDITOR, PUT, restore TUI.
///
/// Errors are surfaced via `state.status_message` on the top PagesBrowse screen
/// (Success / Error). The TUI is ALWAYS restored on exit, even on error
/// (Pitfall 4 from RESEARCH.md).
async fn handle_edit_page(
    app: &mut App,
    client: &Client,
    page_id: &str,
    content_type: ContentType,
    terminal: &mut ratatui::DefaultTerminal,
) {
    // Validate id is digits-only (path traversal mitigation — T-03-20)
    if page_id.is_empty() || !page_id.chars().all(|c| c.is_ascii_digit()) {
        set_pages_status(
            app,
            "Invalid page id (must be numeric)".to_string(),
            StatusStyle::Error,
        );
        return;
    }

    // Phase 1: fetch detail BEFORE suspending — keep TUI responsive on slow networks.
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
    let current_version = detail.version.as_ref().map(|v| v.number).unwrap_or(1);
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

    let temp_path = std::env::temp_dir().join(format!("ccli-edit-{}.xml", page_id));

    // Phase 2: suspend TUI — ratatui::restore() handles raw mode + alt screen + panic hook.
    ratatui::restore();

    // Always restore the TUI before returning, no matter what fails (Pitfall 4).
    let editor_result = run_editor_workflow(&temp_path, &body_xml);

    // Phase 3: regardless of editor outcome, re-enter the TUI before any further work.
    *terminal = ratatui::init();

    let new_xml = match editor_result {
        Ok(s) => s,
        Err(e) => {
            set_pages_status(app, format!("Editor failed: {}", e), StatusStyle::Error);
            return;
        }
    };

    // Phase 4: PUT to Confluence
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

    match put_result {
        Ok(()) => {
            // Best-effort cleanup of temp file
            let _ = std::fs::remove_file(&temp_path);
            set_pages_status(app, "Saved.".to_string(), StatusStyle::Success);
        }
        Err(AppError::Api(msg)) if msg.starts_with("Conflict:") => {
            // D-39: preserve temp file; status message includes the path
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

/// Run the editor (Command::new(editor).arg(path).status()) and read back the file.
/// Returns the new XML on success. NEVER invokes a shell.
fn run_editor_workflow(
    path: &std::path::Path,
    initial_content: &str,
) -> anyhow::Result<String> {
    use anyhow::Context;
    std::fs::write(path, initial_content)
        .with_context(|| format!("Failed to write temp file {}", path.display()))?;
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());
    // Security T-03-21: arg() avoids shell — no injection via crafted $EDITOR.
    let status = std::process::Command::new(&editor)
        .arg(path)
        .status()
        .with_context(|| format!("Failed to launch $EDITOR ({})", editor))?;
    if !status.success() {
        anyhow::bail!("$EDITOR exited non-zero (status {:?})", status.code());
    }
    let new_xml = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read edited content {}", path.display()))?;
    Ok(new_xml)
}

/// Extract the space key from `_links.webui` (e.g. `/display/DEV/Page+Title` → `DEV`).
fn extract_space_key_from_webui(detail: &PageDetail) -> Option<String> {
    let webui = detail.links.webui.as_deref()?;
    let mut segs = webui.split('/').filter(|s| !s.is_empty());
    while let Some(seg) = segs.next() {
        if seg == "display" {
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
    use crate::api::page::{PageDetail, PageLinks};

    fn detail_with_webui(webui: &str) -> PageDetail {
        PageDetail {
            id: "1".into(),
            title: "T".into(),
            content_type: "page".into(),
            version: None,
            ancestors: None,
            body: None,
            links: PageLinks {
                webui: Some(webui.to_string()),
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
    fn run_editor_workflow_returns_error_when_editor_fails() {
        // Set EDITOR to a binary that does not exist; the call must produce an Err.
        let path = std::env::temp_dir().join("ccli-test-editor-fail.xml");
        std::env::set_var("EDITOR", "this-binary-does-not-exist-xyzzy-12345");
        let result = run_editor_workflow(&path, "<p>test</p>");
        std::env::remove_var("EDITOR");
        assert!(result.is_err(), "expected editor launch failure to surface as Err");
        let _ = std::fs::remove_file(&path);
    }
}
