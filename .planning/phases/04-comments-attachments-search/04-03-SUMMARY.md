---
phase: 04
plan: 03
subsystem: tui
tags: [tui, ratatui, state-machine, screen-stack, comments]
dependency_graph:
  requires: [04-01]
  provides: [CommentsBrowseState, Screen::CommentsBrowse, KeyAction::DrillDownComments]
  affects: [src/tui/app.rs, src/tui/mod.rs]
tech_stack:
  added: []
  patterns: [state-machine, screen-stack-push-pop, channel-dispatch]
key_files:
  created: []
  modified:
    - src/tui/app.rs
    - src/tui/mod.rs
decisions:
  - "Screen enum variant names mandated by D-48/D-61; clippy::enum_variant_names suppressed with comment"
  - "CommentsBrowse render in tui/mod.rs uses minimal stub (no-op draw) — full render_comments() deferred to Plan 05"
metrics:
  duration: 18min
  completed: "2026-05-07"
  tasks: 1
  files: 2
---

# Phase 4 Plan 3: CommentsBrowse Screen State Summary

**One-liner:** CommentsBrowseState struct and Screen::CommentsBrowse variant with DrillDownComments key action wiring into the TUI event loop.

## Tasks Completed

| Task | Description | Commit | Files |
|------|-------------|--------|-------|
| 1 | Add CommentsBrowseState, Screen::CommentsBrowse, KeyAction::DrillDownComments, 'c' handler | 77f135b | src/tui/app.rs, src/tui/mod.rs |

## What Was Built

### src/tui/app.rs

- `Screen::CommentsBrowse { page_id: String, state: Box<CommentsBrowseState> }` — third screen stack variant alongside SpacesBrowse and PagesBrowse (D-48)
- `CommentsBrowseState` struct — owns page_id, comments Vec, list_state, browse_state (AppState), spinner_frame, error
- Full `impl CommentsBrowseState` with: `new()`, `with_comments()` (test-only), `set_comments()`, `set_fetch_error()`, `selected_comment()`, `select_next/prev/first/last()`, `tick()`, `visible_count()`, `handle_key()`
- `KeyAction::DrillDownComments(String)` — new variant emitted when 'c' is pressed with a selected page
- `PagesBrowseState::handle_key` extended with `KeyCode::Char('c')` branch in Browse state — emits `DrillDownComments(selected_id)` or `None` when no selection (T-04-17 mitigation)
- 11 new unit tests covering all 11 specified behaviors
- `#[allow(clippy::enum_variant_names)]` on Screen enum with explanatory comment (variant names mandated by design decisions D-48, D-61)

### src/tui/mod.rs

- Import additions: `list_comments`, `Comment`, `CommentsBrowseState`
- `comments_list_tx/rx` channel for async comment fetch results
- Render dispatch extended with `Some(Screen::CommentsBrowse { .. })` arm (stub — render_comments deferred to Plan 05)
- Key routing updated to `match` on screen stack top, routing CommentsBrowse keys to `CommentsBrowseState::handle_key`
- Comments drain loop: routes `(page_id, Result<Vec<Comment>, AppError>)` to the matching CommentsBrowse screen
- `KeyAction::DrillDownComments(page_id)` arm: pushes `Screen::CommentsBrowse`, spawns `list_comments` fetch task

## Verification Results

```
cargo test tui::app → 48 passed; 0 failed (37 existing + 11 new)
cargo test         → 214 passed; 0 failed
cargo build        → exit 0, no errors
cargo clippy -- -D warnings → exit 0, clean
```

## Acceptance Criteria Met

- `grep -c 'CommentsBrowse {' src/tui/app.rs` → 1 ✓
- `grep -c '^pub struct CommentsBrowseState' src/tui/app.rs` → 1 ✓
- `grep -c '^impl CommentsBrowseState' src/tui/app.rs` → 1 ✓
- `grep -c 'pub fn new(page_id: String) -> Self' src/tui/app.rs` → 1 ✓
- `grep -c 'pub fn handle_key' src/tui/app.rs` → 3 (≥2) ✓
- `grep -c 'DrillDownComments' src/tui/app.rs` → 5 (≥3) ✓
- `grep -c "KeyCode::Char('c')" src/tui/app.rs` → 3 (≥1) ✓
- `grep -c 'use crate::api::comment::Comment' src/tui/app.rs` → 1 ✓

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] tui/mod.rs non-exhaustive match patterns**
- **Found during:** Task 1 compilation
- **Issue:** Adding `Screen::CommentsBrowse` and `KeyAction::DrillDownComments` caused two non-exhaustive match errors in `src/tui/mod.rs` (render dispatch + key action match)
- **Fix:** Extended render dispatch with `CommentsBrowse` stub arm; extended key action match with `DrillDownComments` arm; added channel, drain loop, and import additions needed for full wiring
- **Files modified:** src/tui/mod.rs
- **Commit:** 77f135b (included in same commit)

**2. [Rule 2 - Clippy] clippy::enum_variant_names on Screen enum**
- **Found during:** Task 1 clippy run
- **Issue:** All three Screen variants end with `Browse`; clippy -D warnings treats this as an error
- **Fix:** Added `#[allow(clippy::enum_variant_names)]` with explanatory comment — renaming would break D-48/D-61 design decisions
- **Files modified:** src/tui/app.rs
- **Commit:** 77f135b

## Known Stubs

| Stub | File | Line | Reason |
|------|------|------|--------|
| CommentsBrowse render arm (no-op) | src/tui/mod.rs | ~120 | Full `render_comments()` is Plan 05's deliverable; stub prevents panic while state layer plan is isolated |

## Threat Surface Scan

No new network endpoints, auth paths, or schema changes introduced. New surface:
- `KeyAction::DrillDownComments` spawns `list_comments` fetch — same pattern as existing `DrillDown` (list_all_pages), same auth model (PAT via Client)
- T-04-17 mitigated: `'c'` key emits `DrillDownComments` only inside `AppState::Browse` arm AND only with a valid selection (test 11 verifies no-selection case)
- T-04-14 deferred (as planned): `CommentsBrowseState` stores raw `Comment` structs; sanitization at render time (Plan 05)

## Self-Check: PASSED

- src/tui/app.rs exists and contains all required symbols: FOUND
- src/tui/mod.rs exists with CommentsBrowse wiring: FOUND
- Commit 77f135b exists: FOUND
- 214 tests pass: CONFIRMED
