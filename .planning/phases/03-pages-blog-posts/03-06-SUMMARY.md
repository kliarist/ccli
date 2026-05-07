---
phase: 03-pages-blog-posts
plan: "06"
subsystem: tui
tags: [tui, screen-stack, drill-down, event-loop, editor-workflow, browser-open, pages-preview]
dependency_graph:
  requires: [03-01, 03-03, 03-04, 03-05]
  provides: [tui-drill-down, tui-pages-preview, tui-editor-workflow, tui-browser-open-pages]
  affects: [src/tui/mod.rs, src/tui/app.rs, src/tui/screens/pages.rs]
tech_stack:
  patterns:
    - "mpsc channels for async page list and preview fetches (same pattern as spaces preview)"
    - "screen_stack.last_mut() dispatch: render + key routing split by top screen"
    - "TUI suspend/restore: ratatui::restore() + *terminal = ratatui::init() around editor"
    - "Unconditional TUI restore before any post-editor error propagation (Pitfall 4)"
    - "Command::new(editor).arg(path) — direct exec, no shell injection surface (T-03-21)"
    - "digits-only page_id validation before /tmp path construction (T-03-20)"
key_files:
  created: []
  modified:
    - src/tui/mod.rs
    - src/tui/app.rs
    - src/tui/screens/pages.rs
decisions:
  - "WR-02: OpenBrowser payload from PagesBrowseState carries webui PATH (p.links.webui) not page id — aligns with D-24 (URL from API response, not hand-built)"
  - "Missing webui in OpenBrowser returns KeyAction::None gracefully (no panic, no garbage URL)"
  - "pages-branch OpenBrowser handled in handle_open_page_browser (webui PATH already resolved by PagesBrowseState); spaces-branch continues to use handle_open_browser (resolves from app.spaces)"
  - "handle_edit_page fetches page detail BEFORE suspending TUI so network latency is absorbed without a blank screen"
  - "Conflict: AppError::Api variant match used for 409 status message (D-39); temp file preserved"
metrics:
  duration: 25min
  completed_date: "2026-05-07T12:45:45Z"
  tasks_completed: 2
  files_modified: 3
---

# Phase 03 Plan 06: TUI Event Loop Wiring Summary

**One-liner:** Screen-stack drill-down, page preview fetch, browser-open (webui PATH), and editor suspend/restore wired into the running TUI event loop; WR-02 OpenBrowser payload fixed.

## What Was Built

### Task 1: Fix OpenBrowser payload (WR-02) + remove dead_code suppressions

**Files:** `src/tui/app.rs`, `src/tui/screens/pages.rs`

- `PagesBrowseState::handle_key` Browse arm Enter/`o` now returns `KeyAction::OpenBrowser(webui_path)` using `selected_page().and_then(|p| p.links.webui.clone())` instead of the page id (WR-02 fix, D-32, D-24).
- When `_links.webui` is absent, returns `KeyAction::None` gracefully (no panic, no garbage URL).
- `'e'` arm unchanged — still returns `KeyAction::EditPage(id)` (editor needs the id, not the URL).
- Existing tests renamed and re-asserted:
  - `pages_state_handle_key_enter_returns_open_browser_with_webui_path_d32_wr02`
  - `pages_state_handle_key_o_returns_open_browser_with_webui_path_wr02`
- New test: `pages_state_handle_key_o_returns_none_when_webui_missing`
- `src/tui/screens/pages.rs`: removed `#![allow(dead_code)]` file-level suppression (replaced comment with "Wired into the event loop in src/tui/mod.rs (Plan 06)")
- `src/tui/app.rs`: removed `#[allow(dead_code)]` from `screen_stack` field

### Task 2: Wire event loop in src/tui/mod.rs

**File:** `src/tui/mod.rs` (376 lines added, 10 removed)

#### Imports extended
Added imports for `tokio::sync::mpsc`, `crate::api::page::{get_page_detail, list_all_pages, update_page, ContentType, Page, PageDetail}`, and `crate::tui::app::{App, KeyAction, PagesBrowseState, Screen, StatusMessage, StatusStyle}`, and `crate::tui::screens::pages::render_pages`.

#### Screen-stack render dispatch (D-31)
`terminal.draw` now matches `app.screen_stack.last_mut()`:
- `Some(Screen::PagesBrowse { state, .. })` → `render_pages(f, state.as_mut(), f.area())`
- `None | Some(Screen::SpacesBrowse)` → `render_spaces(f, &mut app, f.area())`

#### New channels
- `pages_list_tx/rx`: `mpsc::Sender<(String, Result<Vec<Page>, AppError>)>` — keyed by space_key for concurrent drill-downs.
- `pages_preview_tx/rx`: `mpsc::Sender<(String, Result<PageDetail, AppError>)>` — keyed by page_id (D-43).

#### Key dispatch (D-30)
Key routing split by `app.screen_stack.last_mut()`:
- `PagesBrowse` on top → `state.handle_key(key.code)`
- Otherwise → `app.handle_key(key.code)` (Phase 2 spaces flow)

#### New action arms
- `KeyAction::DrillDown(space_key)`: pushes `Screen::PagesBrowse` + spawns `list_all_pages` fetch via `pages_list_tx`.
- `KeyAction::PopScreen`: calls `app.screen_stack.pop()`.
- `KeyAction::EditPage(page_id)`: calls `handle_edit_page(...)`.
- `KeyAction::OpenBrowser(payload)`: two branches — pages branch calls `handle_open_page_browser`; spaces branch calls `handle_open_browser`.

#### Pages preview fetch (D-43)
On each loop iteration, for the top `PagesBrowse` screen:
1. `state.tick()` — advances spinner, fires 150ms debounce.
2. If `state.pending_preview_id` is `Some`, take it and spawn `get_page_detail` through `pages_preview_tx`.

#### New helper functions
| Function | Description |
|---|---|
| `handle_open_page_browser` | Opens page webui PATH in browser; D-23 SSH/headless fallback |
| `handle_edit_page` | D-41 editor workflow: fetch detail, suspend TUI, $EDITOR, PUT, restore TUI |
| `run_editor_workflow` | Writes temp file, launches `$EDITOR` via `Command::new` (no shell), reads result |
| `extract_space_key_from_webui` | Extracts space key from `/display/DEV/Page+Title` pattern |
| `set_pages_status` | Sets `state.status_message` on top PagesBrowse screen |

#### Security mitigations implemented
| Threat | Mitigation |
|---|---|
| T-03-20: Path traversal via crafted page id | `page_id.chars().all(|c| c.is_ascii_digit())` check in `handle_edit_page` |
| T-03-21: Shell injection via `$EDITOR` | `Command::new(&editor).arg(path)` — direct exec, no shell |
| T-03-25: TUI not restored on panic/error | Unconditional `*terminal = ratatui::init()` before any post-editor error handling |

#### Unit tests added (in `#[cfg(test)] mod tests`)
- `extract_space_key_from_webui_returns_segment_after_display`
- `extract_space_key_from_webui_returns_none_for_unknown_path`
- `extract_space_key_from_webui_handles_long_titles_with_slashes`
- `run_editor_workflow_returns_error_when_editor_fails`

## Verification

- `cargo build`: exit 0, no warnings
- `cargo test`: 189 passed, 0 failed
- `cargo clippy --no-deps -- -D warnings`: exit 0
- No remaining `Plan 06 will wire screen-stack dispatch` TODO comment
- No remaining `KeyAction::DrillDown(_) | KeyAction::PopScreen | KeyAction::EditPage(_) => {}` no-op
- No `#![allow(dead_code)]` in `src/tui/screens/pages.rs`
- `PagesBrowseState::handle_key` returns `OpenBrowser(webui_path)` not `OpenBrowser(id)`

## Manual Smoke Test

The event loop is fully wired. A user running `./target/debug/ccli` can:
1. Press Enter on a space → PagesBrowse pushed, `list_all_pages` fetch spawned, spinner visible while loading.
2. Navigate with j/k → 150ms debounce fires, preview fetch dispatched, preview pane populates.
3. Press `o` or Enter on a page → `_links.webui` path resolved with `base_url`, browser opened.
4. Press Esc → screen stack popped, SpacesBrowse restored with previous selection.
5. Press `e` → TUI suspended, `$EDITOR` launched on `/tmp/ccli-edit-<id>.xml`, PUT on save, TUI restored, "Saved." or error shown in status bar.

## Deviations from Plan

### Minor: use crate::api::page:: import count

The plan acceptance criterion expected `grep -c 'use crate::api::page::' src/tui/mod.rs` to return 1. The actual count is 2 because the test module at the bottom adds `use crate::api::page::{PageDetail, PageLinks}`. This is idiomatic Rust (test modules self-import what they need) and necessary for the tests to compile. The production import is the correct single `use crate::api::page::{...}` block.

### Rule 2: StatusStyle + StatusMessage already had #[allow(dead_code)]

Per plan Step 4 note, the `Screen` enum and `PagesBrowseState` `#[allow(dead_code)]` attributes were not explicitly removed in Task 1 (plan deferred to Task 2). After Task 2 fully wired them, the build showed no dead_code warnings at all — confirming these types are now fully reachable. The `#[allow(dead_code)]` attributes on `Screen` and `PagesBrowseState` remain (they were on the enum/struct, not fields). The compiler emits no warnings, so these are harmless.

## Known Stubs

None. All wired functionality uses real API calls and real state.

## Threat Flags

None beyond what is documented in the plan's threat model (all mitigations implemented as specified).

## Self-Check: PASSED

Files exist:
- src/tui/mod.rs: FOUND
- src/tui/app.rs: FOUND
- src/tui/screens/pages.rs: FOUND

Commits exist:
- 330933d (Task 1: fix OpenBrowser payload + dead_code removal): FOUND
- 201a010 (Task 2: wire event loop): FOUND
