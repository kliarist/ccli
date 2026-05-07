---
phase: 04
plan: 05
subsystem: tui-render
tags: [tui, ratatui, render, event-loop, async-fetch, comments]
dependency_graph:
  requires: [04-01, 04-03]
  provides: [render_comments, CommentsBrowse-render-dispatch]
  affects: [src/tui/screens/comments.rs, src/tui/screens/mod.rs, src/tui/screens/pages.rs, src/tui/mod.rs]
tech_stack:
  added: []
  patterns: [ratatui-TestBackend-tests, vertical-4-row-layout, middle-dot-D50, async-drain-loop]
key_files:
  created:
    - src/tui/screens/comments.rs
  modified:
    - src/tui/screens/mod.rs
    - src/tui/screens/pages.rs
    - src/tui/mod.rs
decisions:
  - modal_h increased from 16 to 17 to accommodate new c-View-comments row
  - render_comments tick wired in timeout branch (not poll branch) to match PagesBrowse pattern
metrics:
  duration: 15min
  completed: "2026-05-07"
  tasks: 3
  files: 4
---

# Phase 4 Plan 05: CommentsBrowse Render + TUI Wiring Summary

Complete CMNT-01: CommentsBrowse TUI screen rendered and wired into the event loop. Pressing 'c' on a page shows a full Comments browse screen with author·date·first-line list rows, full-body preview pane, loading spinner, and Esc-to-pop navigation.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create src/tui/screens/comments.rs and register module | 1f5500a | src/tui/screens/comments.rs, src/tui/screens/mod.rs |
| 2 | Update pages.rs help bar and modal for c-comments binding | 2e63026 | src/tui/screens/pages.rs |
| 3 | Wire render_comments into tui event loop | ed7d0d8 | src/tui/mod.rs |

## What Was Built

### src/tui/screens/comments.rs (new)
Pure render function `render_comments(f, state, area)` following the vertical 4-row split pattern from `render_pages`:
- **Title bar**: "Comments — {PAGE-ID}" bold left + "{N} total / Loading…" dim right
- **Body**: 40/60 horizontal split — list pane (left) + preview pane (right)
- **List pane**: `{author} · {date} · {first_line}` per D-50, middle-dot separator (U+00B7), truncated with "…"; Loading/empty states handled
- **Preview pane**: Author + Date labels (bold, 14-char padded) + stripped body text; "—" in DarkGray+DIM for missing values
- **Status bar**: spinner+Cyan in Loading, "{N} comments" in DarkGray in Browse, "Error: {msg}" in Red
- **Help bar**: `o  open   ?  help   Esc  back`

### src/tui/screens/mod.rs
Added `pub mod comments;` registration line.

### src/tui/screens/pages.rs
- `render_help_bar`: extended with `key("c") + Span::raw("  comments   ")` before `?` (D-49)
- `render_help_modal`: added `row("c", "View comments")` after `e — Edit in $EDITOR`; `modal_h` increased 16→17 to fit the new row
- Tests: `render_pages_help_bar_shows_required_bindings` extended with `"comments"` token; new `render_pages_help_modal_lists_view_comments_action` test added

### src/tui/mod.rs
- Added `use crate::tui::screens::comments::render_comments` import
- Replaced the Plan 04 placeholder render arm (blank draw) with `render_comments(f, state.as_mut(), f.area())`
- Added CommentsBrowse spinner tick in the timeout branch after PagesBrowse tick block
- All other wiring (channel, drain loop, DrillDownComments arm, key routing) was already present from Plan 04

## Test Results

- 7 new render_comments tests: all pass
- 2 new/modified pages tests: all pass
- Full suite: **235 tests passing**, 0 failures
- `cargo clippy --all-targets -- -D warnings`: clean (0 errors, 0 warnings)
- `cargo build`: clean

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing functionality] Increased help modal height from 16 to 17**
- **Found during:** Task 2
- **Issue:** Adding `c — View comments` row to the 14-line modal content made it 15 lines; the 16-height modal (14 content + 2 border) would clip the last "q — Quit" row
- **Fix:** Changed `modal_h` from 16 to 17
- **Files modified:** src/tui/screens/pages.rs
- **Commit:** 2e63026

## Known Stubs

None — all plan goals are fully implemented. The `o` key in CommentsBrowse is intentionally a no-op in v1 (plan notes: CommentsBrowseState::handle_key returns KeyAction::None for 'o'; future work to open comment webui link). Help bar shows `o  open` as a hint per the plan's threat model T-04-31 acceptance.

## Threat Flags

None — all files stay within the threat model defined in the plan. `render_comments` uses `Span::raw` for comment body text (T-04-25 defense-in-depth via Ratatui buffer cell API). No new network endpoints or auth paths introduced.

## Self-Check: PASSED

Files created/modified:
- FOUND: /Users/thekliar/Dev/projects/5soft/confluence-cli/src/tui/screens/comments.rs
- FOUND: /Users/thekliar/Dev/projects/5soft/confluence-cli/src/tui/screens/mod.rs
- FOUND: /Users/thekliar/Dev/projects/5soft/confluence-cli/src/tui/screens/pages.rs
- FOUND: /Users/thekliar/Dev/projects/5soft/confluence-cli/src/tui/mod.rs

Commits verified: 1f5500a, 2e63026, ed7d0d8

Task 4 (checkpoint:human-verify) awaiting user verification.
