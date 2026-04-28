---
phase: 02-tui-shell-spaces
reviewed: 2026-04-28T11:52:12Z
depth: standard
files_reviewed: 12
files_reviewed_list:
  - Cargo.toml
  - Cargo.lock
  - src/api/client.rs
  - src/api/mod.rs
  - src/api/space.rs
  - src/cli/mod.rs
  - src/cli/space.rs
  - src/main.rs
  - src/tui/app.rs
  - src/tui/mod.rs
  - src/tui/screens/mod.rs
  - src/tui/screens/spaces.rs
findings:
  critical: 0
  warning: 4
  info: 3
  total: 7
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-04-28T11:52:12Z
**Depth:** standard
**Files Reviewed:** 12
**Status:** issues_found

## Summary

Reviewed the full phase-02 implementation: TUI shell, space list/preview, fuzzy filter, browser open, and all supporting API and CLI plumbing. The code is generally well-structured with good test coverage for the happy paths. Four warnings and three info items were found. No critical security vulnerabilities or data-loss risks were identified. The most impactful bugs are: (1) a double terminal-restore on headless browser open followed by a normal quit, which corrupts terminal state for the parent shell, and (2) `select_first`/`select_last` not clearing `pending_preview_key`, which causes a stale preview fetch to fire for the wrong space after a `g`/`G` keypress. A field (`spaces_fetched_count`) that drives a UI branch is declared and initialized but never incremented anywhere in the codebase, making that display branch permanently dead.

---

## Warnings

### WR-01: Double `ratatui::restore()` corrupts terminal after headless browser open

**File:** `src/tui/mod.rs:72` and `src/tui/mod.rs:186-207`

**Issue:** When a browser open fails (SSH/headless path, D-23), `handle_open_browser` calls `ratatui::restore()` at line 186 and then `ratatui::init()` at line 207 to re-enter the TUI. When the user subsequently quits normally, `run_app()` returns to `run()`, which calls `ratatui::restore()` a **second time** at line 72. Calling `ratatui::restore()` twice leaves the parent terminal in an inconsistent state (raw mode may be re-toggled, alternate screen not properly exited, etc.) and is a known pattern to corrupt prompt rendering in the parent shell session.

**Fix:** Track whether the TUI was re-initialised inside `handle_open_browser` and skip the outer restore, or — cleaner — move terminal ownership into `handle_open_browser` so the inner `ratatui::init()` replaces the one tracked by `run()`:

```rust
// In run() — use a boolean flag to suppress the outer restore
// when handle_open_browser has already cycled the terminal.
// Alternatively, pull terminal re-init responsibility out of
// handle_open_browser entirely and return a "TUI was suspended"
// signal, letting run_app handle the re-init:

enum BrowserResult { Opened, SuspendedAndRestored }

// run_app calls ratatui::restore() + ratatui::init() itself if
// BrowserResult::SuspendedAndRestored, and run() always calls
// ratatui::restore() exactly once at the end.
```

---

### WR-02: `select_first` and `select_last` do not clear `pending_preview_key`

**File:** `src/tui/app.rs:164-178`

**Issue:** `select_next` and `select_prev` both set `self.pending_preview_key = None` (lines 144 and 160), but `select_first` and `select_last` only set `last_selection_change` and do not clear `pending_preview_key`. If a debounce cycle has already set `pending_preview_key = Some("OLD")` and the user immediately presses `g` or `G`, the event loop's `pending_preview_key.take()` on the next iteration spawns a detail fetch for `"OLD"` rather than for the newly-selected space. The new selection's preview is then delayed by an extra 150 ms debounce cycle, and an unnecessary network request is made.

**Fix:**
```rust
pub fn select_first(&mut self) {
    if !self.filtered_indices.is_empty() {
        self.list_state.select(Some(0));
        self.last_selection_change = Some(Instant::now());
        self.pending_preview_key = None; // add this line
    }
}

pub fn select_last(&mut self) {
    if !self.filtered_indices.is_empty() {
        let last = self.filtered_indices.len() - 1;
        self.list_state.select(Some(last));
        self.last_selection_change = Some(Instant::now());
        self.pending_preview_key = None; // add this line
    }
}
```

---

### WR-03: Pagination loop has no safety limit — infinite loop on misbehaving server

**File:** `src/api/space.rs:101-148`

**Issue:** `list_all_spaces` loops until `!has_next || fetched < limit`. There is no upper bound on the number of pages fetched. A Confluence server that consistently returns `_links.next` and exactly `limit` (25) items on every page — which is conceivable if a proxy or misconfigured instance echoes pagination links erroneously — causes an unbounded loop. There is no page-count guard, no total-count cap, and no timeout beyond the individual request's 30 s connection timeout. With 100 MB of spaces data this would also exhaust memory before the caller sees a result.

**Fix:**
```rust
const MAX_PAGES: u32 = 400; // 400 * 25 = 10,000 spaces — far above any real instance
let mut pages_fetched = 0u32;

loop {
    pages_fetched += 1;
    if pages_fetched > MAX_PAGES {
        return Err(AppError::Api(format!(
            "Pagination safety limit ({} pages) exceeded — aborting space list fetch",
            MAX_PAGES
        )));
    }
    // ... rest of loop
}
```

---

### WR-04: `render_filter_overlay` uses hardcoded `height: 3` without bounding to `list_area.height`

**File:** `src/tui/screens/spaces.rs:383-388`

**Issue:** The filter overlay `Rect` is constructed with `height: 3` regardless of the parent `list_area` height. The comparable `render_help_modal` correctly guards with `modal_height.min(area.height)` (line 430). If the terminal is very short (e.g. height ≤ 5, leaving `body_area` with 1–2 rows), ratatui will receive a `Rect` whose `y + height` exceeds the terminal bounds. Ratatui clamps internally so this won't panic, but the overlay will overflow into the status/help bar rows, overwriting them visually. In practice extreme terminal resizing while the filter is open can reproduce this.

**Fix:**
```rust
let overlay_area = Rect {
    x: list_area.x,
    y: list_area.y,
    width: list_area.width,
    height: 3u16.min(list_area.height), // bound to parent, matching help modal pattern
};
```

---

## Info

### IN-01: `spaces_fetched_count` is never incremented — status bar branch is dead code

**File:** `src/tui/app.rs:64,83` and `src/tui/screens/spaces.rs:301-302`

**Issue:** `App::spaces_fetched_count` is declared as a `pub usize` field, initialized to `0` in `App::new()`, and referenced in the status bar renderer's loading branch (`if app.spaces_fetched_count > 0 { format!("Loading spaces… {} fetched", ...) }`). It is **never assigned or incremented anywhere else** in the codebase. The `> 0` branch is permanently unreachable; the status bar during loading will always show plain `"Loading spaces…"` regardless of progress.

**Fix:** Either remove the field and the dead branch, or — if progressive count display is desired — increment `spaces_fetched_count` when pages arrive (requires switching from a oneshot channel to a streaming approach, e.g. `tokio::sync::mpsc`).

---

### IN-02: `q` is reserved as a quit key inside the filter overlay — spaces with `q` in key/name cannot be filtered

**File:** `src/tui/app.rs:303`

**Issue:** In `AppState::Filter`, `KeyCode::Char('q')` is matched first (line 303) and returns `KeyAction::Quit` before the catch-all `KeyCode::Char(c)` branch that appends to the query. A user typing `q` while the filter overlay is open quits the TUI immediately rather than refining the query. This makes it impossible to filter for spaces whose key or name contains the letter `q` (e.g. `QA`, `QTEST`, `Quarterly`). This is implemented intentionally per D-12 ("q always quits") but the interaction with the filter overlay is a concrete usability defect.

**Fix:** Consider scoping "q quits" to non-filter states only, or providing a secondary quit binding (`Ctrl-C` / `Ctrl-Q`) that works everywhere, and letting `q` be a regular character in filter mode:

```rust
AppState::Filter { query } => {
    let mut q = query.clone();
    match key {
        // Remove the KeyCode::Char('q') => Quit arm here
        KeyCode::Esc => { ... }
        KeyCode::Char(c) => { q.push(c); ... }  // 'q' falls through to here
        _ => {}
    }
    KeyAction::None
}
```

---

### IN-03: `apply_filter` index recovery uses `Vec::position` (O(n²)) and silently drops duplicate haystacks

**File:** `src/tui/app.rs:206-209`

**Issue:** After `match_list` returns scored matches, original indices are recovered by calling `haystacks.iter().position(|x| x == h)` for each matched haystack string. This is O(n²) across all spaces. More subtly, `position` returns the index of the **first** occurrence — if two spaces ever produce the same `"{key} {name}"` haystack string (theoretically impossible since Confluence space keys are unique, but not enforced by the type system here), one space would be silently dropped from the filtered results. Both issues would be eliminated by building a `HashMap<&str, usize>` from haystack to index before calling `match_list`.

**Fix:**
```rust
// Build a lookup map once instead of scanning O(n) per result
let haystack_to_idx: std::collections::HashMap<&str, usize> = haystacks
    .iter()
    .enumerate()
    .map(|(i, s)| (s.as_str(), i))
    .collect();

self.filtered_indices = matched
    .iter()
    .filter_map(|(h, _)| haystack_to_idx.get(h.as_str()).copied())
    .collect();
```

---

_Reviewed: 2026-04-28T11:52:12Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
