---
phase: 02-tui-shell-spaces
plan: "05"
subsystem: tui
tags: [tui, ratatui, event-loop, tokio, crossterm, open-crate, async, spaces]

# Dependency graph
requires:
  - phase: 02-tui-shell-spaces/02-01
    provides: list_all_spaces, get_space_detail, resolve_webui_url, Space, SpaceLinks
  - phase: 02-tui-shell-spaces/02-02
    provides: CLI dispatch wiring; bare ccli routes to tui::run()
  - phase: 02-tui-shell-spaces/02-03
    provides: App state machine, AppState enum, KeyAction enum, App::handle_key(), App::tick(), pending_preview_key
  - phase: 02-tui-shell-spaces/02-04
    provides: render_spaces() pure render function; screens module
provides:
  - src/tui/mod.rs — complete event loop; pub async fn run() entry point
  - ratatui::init/restore terminal lifecycle with panic hook
  - 100ms poll loop with spinner tick integration
  - async space-list pre-fetch via oneshot channel (D-14)
  - 150ms debounce preview fetch via pending_preview_key + mpsc channel (D-19)
  - browser open via open::that() with SSH/headless fallback (D-22/D-23)
  - URL resolution from _links.webui via resolve_webui_url (D-24)
affects: [phase-03-read-edit, any future TUI plans]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "oneshot channel: async pre-fetch communicates result to sync event loop via try_recv"
    - "mpsc channel: per-preview fetches spawned into tokio tasks, results sent back to event loop"
    - "poll+try_recv: non-blocking channel checks interleaved with 100ms crossterm::event::poll()"
    - "SSH fallback: ratatui::restore() + eprintln then ratatui::init() re-entry on open::that() failure"

key-files:
  created: []
  modified:
    - src/tui/mod.rs
    - src/api/client.rs

key-decisions:
  - "poll() on async thread is acceptable for single-user CLI; spawn_blocking not needed (RESEARCH.md Open Q3)"
  - "Client derives Clone because reqwest::Client is Arc-backed — cheap clone, no credential duplication"
  - "base_url extracted from client before move into tokio::spawn; threaded through run_app and handle_open_browser"
  - "60s timeout on SSH-fallback keypress loop prevents infinite wait in headless envs"
  - "preview fetch errors are silently swallowed — non-fatal; pane retains last cached content"

patterns-established:
  - "Channel-per-fetch pattern: oneshot for initial load, mpsc for repeated preview fetches"
  - "try_recv at top of draw loop: channels polled every frame at ~10fps with zero blocking"
  - "handle_open_browser as standalone fn: receives &mut App, space_key, base_url, &mut terminal"

requirements-completed: [TUI-01, TUI-02, TUI-03, TUI-04, TUI-05, TUI-06, SPC-01, SPC-02, SPC-03, SPC-04]

# Metrics
duration: ~35min
completed: "2026-04-28"
---

# Phase 2 Plan 05: TUI Event Loop Summary

**Complete Ratatui event loop wiring all Plans 01-04 — bare `ccli` launches a navigable Spaces TUI with async pre-fetch spinner, 150ms debounce preview, real-time fuzzy filter, browser open with SSH fallback, and always-called terminal restore.**

## Performance

- **Duration:** ~35 min (task implementation + human-verify checkpoint)
- **Started:** 2026-04-28T07:54:26Z (wave 4 start)
- **Completed:** 2026-04-28
- **Tasks:** 1 auto + 1 human-verify checkpoint (approved)
- **Files modified:** 2

## Accomplishments

- Replaced `tui/mod.rs` stub with 204-line complete event loop connecting all prior plans
- Async space-list pre-fetch runs before ratatui::init() so spinner starts immediately (D-14)
- Preview fetch pipeline: pending_preview_key debounce → tokio::spawn → mpsc → try_recv on each frame (D-19)
- Browser open with SSH/headless fallback: ratatui::restore + URL to stderr + keypress await + ratatui::init (D-23)
- Added `#[derive(Clone)]` to Client (reqwest::Client is Arc-backed; cheap; PAT not duplicated — T-02-18)
- Full Phase 2 human-verified: navigation, filter, modal, browser open, quit, `ccli space list` — all approved

## Task Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1: Implement complete src/tui/mod.rs event loop | dbd39ed | feat(02-05): implement complete src/tui/mod.rs event loop |
| Task 2: Human-verify checkpoint | — | Approved by user — all TUI behaviors verified |

## Files Created/Modified

- `src/tui/mod.rs` — full event loop: ratatui::init/restore, run_app(), handle_open_browser(); replaces stub
- `src/api/client.rs` — added `#[derive(Clone)]` to Client struct

## Decisions Made

- **poll() on async thread (not spawn_blocking):** crossterm::event::poll() blocks the tokio executor for ≤100ms per iteration. For a single-user CLI this is acceptable. spawn_blocking adds complexity with no benefit (RESEARCH.md Open Question 3).
- **base_url extracted before client.clone() for spawn:** `client.base_url().to_string()` called before the `fetch_client = client.clone()` move into the spawn closure, then threaded as a `String` parameter through `run_app` and `handle_open_browser`.
- **60s timeout on SSH-fallback keypress:** prevents infinite hang in headless CI environments; restores TUI after 60s regardless.
- **Preview fetch errors silently swallowed:** `Err(_) => {}` in preview_rx arm — non-fatal, pane retains last cached content. Prevents error messages interrupting TUI for transient preview failures.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added Client::clone() support via #[derive(Clone)]**
- **Found during:** Task 1 — implementing `let fetch_client = client.clone()`
- **Issue:** The plan specified `client.clone()` for the tokio::spawn closure but `Client` in `src/api/client.rs` did not derive `Clone`.
- **Fix:** Added `#[derive(Clone)]` to the `Client` struct. reqwest::Client is Arc-backed internally so clone is O(1) and does not duplicate the PAT token in memory.
- **Files modified:** `src/api/client.rs`
- **Verification:** `cargo build --release` exits 0; `cargo test` 108 passed
- **Committed in:** dbd39ed (included in task commit)

---

**Total deviations:** 1 auto-fixed (Rule 2 — missing critical for cloning)
**Impact on plan:** Required for both the initial space fetch and repeated preview fetches. No scope creep.

## Threat Model Coverage

| Threat | Mitigation | Status |
|--------|------------|--------|
| T-02-17: Info Disclosure — URL to stderr | URL is non-sensitive Confluence page URL; PAT never included; printing is the explicit SSH fallback design (D-23) | Accepted |
| T-02-18: Info Disclosure — Client in tokio::spawn | Client cloned via Arc (not credential copy); PAT held in reqwest header pool, never extracted to String; `#[instrument(skip(client))]` on API calls | Mitigated |
| T-02-19: Tampering — open::that(url) | URL = resolve_webui_url(base_url from config, webui from API); passed to OS browser not shell; None case handled | Mitigated |
| T-02-20: DoS — 100ms poll tick | Single-user CLI; minimal CPU overhead; accepted | Accepted |
| T-02-21: Info Disclosure — panic hook | ratatui::init() installs panic hook calling restore(); no credentials in TUI render state | Mitigated |
| T-02-22: Info Disclosure — preview cache | Ephemeral process memory; no persistence; no logging; cleared on exit | Accepted |

## Known Stubs

None — src/tui/mod.rs is fully implemented. The TUI is complete end-to-end.

## Self-Check: PASSED

- [x] `src/tui/mod.rs` exists and is not the stub (no "Stub: replaced in Plan 05")
- [x] `src/tui/mod.rs` contains `ratatui::init()` (4 occurrences — doc comment + 2 call sites + SSH re-entry)
- [x] `src/tui/mod.rs` contains `ratatui::restore()` (4 occurrences — doc comment + 2 call sites + SSH suspend)
- [x] `src/tui/mod.rs` contains `event::poll(` (3 occurrences)
- [x] `src/tui/mod.rs` contains `open::that(` (3 occurrences — doc comment + 1 call)
- [x] `src/tui/mod.rs` contains `tokio::spawn` (3 occurrences)
- [x] `src/tui/mod.rs` contains `render_spaces(` (2 occurrences)
- [x] `src/tui/mod.rs` contains `resolve_webui_url(` (4 occurrences)
- [x] `src/tui/mod.rs` contains `app.tick()` (1 occurrence)
- [x] `cargo build --release` exits 0 (0 errors)
- [x] `cargo test` exits 0 — 108 tests pass, 0 regressions
- [x] Commit dbd39ed exists
- [x] `src/api/client.rs` contains `#[derive(Clone)]`

## Next Phase Readiness

Phase 2 is complete. All 10 plans executed across 4 waves:

- Wave 1 (02-01): Spaces API foundation — Space/SpaceDetail structs, list_all_spaces, get_space_detail, resolve_webui_url
- Wave 2 (02-02): CLI dispatch — ccli space list, --plain/--no-headers/piped output, bare ccli routes to TUI
- Wave 3a (02-03): App state machine — AppState, KeyAction, nucleo fuzzy filter, 150ms debounce
- Wave 3b (02-04): Spaces render — render_spaces() pure function, all widget states, 40/60 split, filter overlay, help modal
- Wave 4 (02-05): Event loop — full TUI wiring, async fetches, browser open, SSH fallback

Phase 3 (read/edit) can begin. Key provided interfaces:
- `pub async fn run() -> anyhow::Result<()>` in `src/tui/mod.rs`
- All Space API functions in `src/api/space.rs`
- Client with Clone support in `src/api/client.rs`

---
*Phase: 02-tui-shell-spaces*
*Completed: 2026-04-28*
