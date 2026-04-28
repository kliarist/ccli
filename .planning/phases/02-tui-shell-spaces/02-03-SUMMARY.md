---
phase: "02"
plan: "03"
subsystem: tui
tags: [tui, app-state, nucleo-matcher, fuzzy-filter, debounce, ratatui, crossterm]
dependency_graph:
  requires: [src/api/space.rs, src/api/error.rs, ratatui, crossterm, nucleo-matcher]
  provides: [src/tui/app.rs, App, AppState, KeyAction, SPINNER_FRAMES]
  affects: [src/tui/mod.rs]
tech_stack:
  added: []
  patterns: [app-state-machine, nucleo-fuzzy-filter, debounce-cache, pure-state-testable-seam]
key_files:
  created: [src/tui/app.rs]
  modified: [src/tui/mod.rs]
decisions:
  - "D-12: q always quits from any state; Esc closes overlays (Filter/Modal) or quits at top-level Browse"
  - "D-14: spaces pre-fetched on TUI launch; stored in App.spaces Vec<Space>"
  - "D-15: real-time fuzzy filter on /; Esc closes and restores full list"
  - "D-16: nucleo-matcher Pattern::parse on '{key} {name}' combined haystack"
  - "D-19: 150ms debounce via last_selection_change Instant; pending_preview_key set by tick()"
  - "Pitfall 5: apply_filter always resets ListState to Some(0) or None to prevent stale-index panic"
metrics:
  duration_minutes: 3
  completed_date: "2026-04-28"
  tasks_completed: 1
  tasks_total: 1
  files_created: 1
  files_modified: 1
  tests_added: 17
  tests_total: 108
---

# Phase 2 Plan 03: App State Machine Summary

**One-liner:** Pure-logic TUI App state machine with nucleo-matcher fuzzy filter on "{key} {name}" haystack, 150ms debounce/preview cache, and all key-event handlers — 17 unit tests, no terminal required.

## What Was Built

**Task 1: src/tui/mod.rs updated + src/tui/app.rs created**

**src/tui/mod.rs** — updated to declare `pub mod app;` submodule with decision-ID doc comment covering D-10, D-12, D-13.

**src/tui/app.rs** — complete App state machine implementation:

- `SPINNER_FRAMES`: 8-frame spinner array for Loading state (100ms per frame)
- `PREVIEW_DEBOUNCE`: 150ms constant (D-19)
- `AppState` enum: `Loading`, `Browse`, `Filter { query: String }`, `Modal` — exact variants from UI-SPEC State Machine
- `App` struct: owns `Vec<Space>`, `ListState`, `filtered_indices: Vec<usize>`, `AppState`, `preview_cache: HashMap<String, SpaceDetail>`, `pending_preview_key`, `last_selection_change`, `spinner_frame`, `spaces_fetched_count`, `error`
- `App::new()`: Loading state, no selection
- `App::with_spaces()` (`#[cfg(test)]`): test constructor that calls `set_spaces()`
- `App::set_spaces()`: transitions to Browse, initialises filtered_indices, selects first
- `App::set_fetch_error()`: transitions out of Loading for error display
- `App::cache_detail()`: inserts into preview_cache, clears pending_preview_key
- `App::selected_key()`: reads through filtered_indices indirection
- `App::select_next()` / `select_prev()`: wrap-around navigation with debounce reset
- `App::select_first()` / `select_last()`: g/G jump keys per UI-SPEC keyboard contract
- `App::apply_filter()`: nucleo-matcher on `"{key} {name}"` haystack (D-16); empty query restores full list; always resets ListState to Some(0) or None (Pitfall 5 mitigation)
- `App::tick()`: advances `spinner_frame`; fires `pending_preview_key` after 150ms debounce (D-19)
- `App::handle_key()`: full key-routing per D-12 and UI-SPEC keyboard contract; returns `KeyAction`
- `KeyAction` enum: `None`, `Quit`, `OpenBrowser(String)`

## Tests

17 new unit tests (all pure state transitions, no terminal needed); 0 regressions (108 total).

| Test | What it covers |
|------|---------------|
| `new_app_is_loading_with_no_selection` | Initial state invariants |
| `set_spaces_transitions_to_browse_and_selects_first` | set_spaces() transition |
| `select_next_wraps_at_end` | Wrap-around at last item |
| `select_prev_wraps_at_start` | Wrap-around at first item |
| `apply_filter_empty_restores_all_indices` | Empty query restores full list |
| `apply_filter_matches_by_key` | Fuzzy match on key prefix |
| `apply_filter_resets_selection_to_zero_pitfall5` | Pitfall 5: stale index prevention |
| `apply_filter_no_matches_clears_selection` | No-match deselects list |
| `handle_key_q_always_quits_from_browse` | q → Quit from Browse |
| `handle_key_q_quits_from_filter_mode` | q → Quit from Filter (D-12) |
| `handle_key_slash_opens_filter_overlay` | / → Filter state |
| `handle_key_esc_in_filter_closes_overlay_and_restores_list` | Esc closes filter, restores list |
| `handle_key_esc_at_browse_top_level_quits` | Esc at Browse top-level → Quit (D-12) |
| `handle_key_o_returns_open_browser_action_with_selected_key` | o → OpenBrowser(key) |
| `handle_key_question_mark_opens_modal` | ? → Modal state |
| `spinner_frame_advances_on_tick` | tick() advances spinner |
| `select_first_and_last_jump_to_extremes` | g/G keyboard contract |

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 | b93defe | feat(02-03): implement src/tui/app.rs — App state machine with nucleo filter |

## Deviations from Plan

None — plan executed exactly as written.

Note: The `match_list` return type in nucleo-matcher 0.3.1 is `Vec<(&str, u32)>` when passing `&str` items (not `String`). The implementation maps the results to owned `String` to recover original indices, which matches the documented plan approach.

## Threat Model Coverage

| Threat | Mitigation | Status |
|--------|------------|--------|
| T-02-09: DoS via long filter query | nucleo-matcher does not use regexes; bounded by space list size | Accepted |
| T-02-10: SpaceDetail in memory | Cache is in-process only; no persistence; no logging of cache contents | Accepted |
| T-02-11: KeyAction::OpenBrowser(key) | Space key is non-sensitive public metadata; URL construction deferred to event loop | Accepted |
| T-02-12: Filter query injection via KeyCode::Char | Characters from crossterm keyboard event parser; no shell injection surface | Accepted |

## Known Stubs

None — `App` is fully implemented. The `tui::run()` function in `mod.rs` remains a stub (intentional — Plan 05 replaces it).

## Self-Check: PASSED

- [x] `src/tui/app.rs` exists
- [x] `src/tui/app.rs` contains `pub struct App {`
- [x] `src/tui/app.rs` contains `pub enum AppState {`
- [x] `src/tui/app.rs` contains `pub enum KeyAction {`
- [x] `src/tui/app.rs` contains `pub const SPINNER_FRAMES`
- [x] `src/tui/app.rs` contains `Pattern::parse` (nucleo-matcher usage)
- [x] `src/tui/app.rs` contains `PREVIEW_DEBOUNCE`
- [x] `src/tui/mod.rs` contains `pub mod app;`
- [x] 108 tests pass, 0 fail (17 new tui::app::tests)
- [x] `cargo build --release` exits 0 (0 errors)
- [x] Commit b93defe exists
