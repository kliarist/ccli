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
use tokio::sync::oneshot;

use crate::api::{get_space_detail, list_all_spaces, resolve_webui_url, AppError, Client, Space};
use crate::config;
use crate::tui::app::{App, KeyAction};
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

    loop {
        // Draw current state
        terminal.draw(|f| render_spaces(f, &mut app, f.area()))?;

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

        // Poll for key events with 100ms timeout (drives spinner tick, RESEARCH.md Pitfall 4)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let action = app.handle_key(key.code);
                    match action {
                        KeyAction::Quit => break,
                        KeyAction::OpenBrowser(space_key) => {
                            handle_open_browser(&mut app, &space_key, &base_url, terminal)?;
                        }
                        KeyAction::None => {}
                        // Plan 06 will wire screen-stack dispatch; no-ops for now.
                        KeyAction::DrillDown(_) | KeyAction::PopScreen | KeyAction::EditPage(_) => {}
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
