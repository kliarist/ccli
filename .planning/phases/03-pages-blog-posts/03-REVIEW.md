---
phase: 03-pages-blog-posts
reviewed: 2026-05-07T00:00:00Z
depth: standard
files_reviewed: 4
files_reviewed_list:
  - src/tui/app.rs
  - src/tui/mod.rs
  - src/tui/screens/pages.rs
  - src/tui/screens/mod.rs
findings:
  critical: 0
  warning: 6
  info: 4
  total: 10
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-05-07T00:00:00Z
**Depth:** standard
**Files Reviewed:** 4
**Status:** issues_found

## Summary

Four files reviewed: the TUI application state machine (`app.rs`), the event-loop shell (`mod.rs`), the pages/blog-posts renderer (`screens/pages.rs`), and the screens module declaration (`screens/mod.rs`). The implementation is largely correct and well-structured. No security vulnerabilities or data-loss risks were found.

Six warnings were identified, all of which could produce silent incorrect behaviour at runtime. The most impactful is a duplicate-title collision in `apply_filter` that silently drops pages when two pages share a title. The second-most impactful is an incomplete URL-resolution path in the pages screen: `KeyAction::OpenBrowser` carries a page **id** from `PagesBrowseState`, but the caller in `mod.rs` currently no-ops `DrillDown/PopScreen/EditPage/OpenBrowser` from that path — meaning the browser-open action is wired for spaces but not for pages, and when it is eventually wired the `webui` link from the page object will need to be used rather than the id string.

---

## Warnings

### WR-01: `apply_filter` index-recovery by title string is O(n²) and collapses duplicate titles

**File:** `src/tui/app.rs:229-232` (and mirror at `src/tui/app.rs:547-550`)

**Issue:** After `pattern.match_list(...)` returns scored matches the code re-maps each matched haystack string back to its original index using `haystacks.iter().position(|x| x == h)`. `position` returns the **first** index whose string equals `h`. When two pages share the same title (or two spaces share the same `"{key} {name}"` composite) only the first one is ever reachable — the second is silently dropped from `filtered_indices`. The user sees fewer results than actually exist and can never navigate to the duplicated item.

**Fix:** Carry the original index through the scoring pass rather than recovering it by string equality:

```rust
// Build (original_index, haystack) pairs
let indexed_haystacks: Vec<(usize, String)> = self.pages
    .iter()
    .enumerate()
    .map(|(i, p)| (i, p.title.clone()))
    .collect();

let matched: Vec<(usize, u32)> = pattern
    .match_list(
        indexed_haystacks.iter().map(|(_, h)| h.as_str()),
        &mut matcher,
    )
    .into_iter()
    .filter_map(|(matched_str, score)| {
        indexed_haystacks
            .iter()
            .find(|(_, h)| h == matched_str)
            .map(|(i, _)| (*i, score))
    })
    .collect();

self.filtered_indices = matched.into_iter().map(|(i, _)| i).collect();
```

A cleaner approach is to implement `Utf32Str` / use nucleo's `Item` index directly, but the above avoids the duplicate-collapse bug.

---

### WR-02: `KeyAction::OpenBrowser` carries page id instead of URL — will mismatch caller contract

**File:** `src/tui/app.rs:616-619`

**Issue:** In `PagesBrowseState::handle_key`, `KeyAction::OpenBrowser(id)` is returned where `id` is a Confluence content **id** (e.g. `"99"`). The `KeyAction::OpenBrowser` variant's documented contract (line 673) says it carries the *space key* and that the caller must resolve the URL via `resolve_webui_url()`. When the pages event-loop is eventually wired in `mod.rs`, the caller will try to pass a raw content id as a space key to `resolve_webui_url()`, which will produce a wrong URL.

The browser-open URL for a page should come from `selected_page()._links.webui`, not from `selected_id()`.

**Fix:** Return the resolved webui path directly or add a new `KeyAction` variant. The simplest correct fix inside `PagesBrowseState::handle_key`:

```rust
KeyCode::Enter | KeyCode::Char('o') => {
    if let Some(page) = self.selected_page() {
        if let Some(webui) = page.links.webui.clone() {
            KeyAction::OpenBrowser(webui) // webui path, not id
        } else {
            KeyAction::None
        }
    } else {
        KeyAction::None
    }
}
```

The caller then calls `resolve_webui_url(base_url, &webui)` as for spaces. Alternatively, introduce a distinct `KeyAction::OpenBrowserPage(String)` variant that takes a webui path to avoid confusion with the space-key variant.

---

### WR-03: `DrillDown`, `PopScreen`, and `EditPage` actions are silently discarded in the event loop

**File:** `src/tui/mod.rs:134`

**Issue:** The event loop contains `KeyAction::DrillDown(_) | KeyAction::PopScreen | KeyAction::EditPage(_) => {}`, which is explicitly a no-op placeholder. This means pressing Enter on a space in the spaces browse view does nothing visually or functionally — the screen stack is never pushed, no page list is ever shown. The screen-stack dispatch, page-list fetch, and editor workflow are completely absent from `run_app`. This is a logic gap rather than a compile error; the code compiles and runs but the primary new feature of Phase 3 (drilling into pages) is inoperative.

**Fix:** Wire the dispatch before shipping. At minimum:

```rust
KeyAction::DrillDown(space_key) => {
    // Push PagesBrowse, spawn list_all_pages fetch
    let pages_state = Box::new(PagesBrowseState::new(
        space_key.clone(), ContentType::Page,
    ));
    app.screen_stack.push(Screen::PagesBrowse {
        space_key,
        content_type: ContentType::Page,
        state: pages_state,
    });
}
KeyAction::PopScreen => {
    app.screen_stack.pop();
}
// EditPage wired in a later plan
KeyAction::EditPage(_) => {}
```

---

### WR-04: Status-bar match in `render_status_bar` uses "match" plural incorrectly for count != 1

**File:** `src/tui/screens/pages.rs:328`

**Issue:** The filter status bar renders `"{N}/{total} {kind} match"` for all values of `N`, including `N > 1`. Showing "2/10 pages match" is grammatically wrong. This is a UI defect visible to users, not a pure style issue.

**Fix:**

```rust
AppState::Filter { .. } => {
    let count = state.visible_count();
    let plural = if count == 1 { "match" } else { "matches" };
    Line::from(vec![
        Span::raw(format!(" {}", count)),
        Span::styled(
            format!("/{} {} {}", state.pages.len(), kind_plural, plural),
            Style::default().fg(Color::DarkGray),
        ),
    ])
}
```

---

### WR-05: `handle_key` clones `self.state` / `self.browse_state` on every keypress to satisfy the borrow checker — mutation through clone is error-prone

**File:** `src/tui/app.rs:274` and `src/tui/app.rs:597`

**Issue:** Both `App::handle_key` and `PagesBrowseState::handle_key` clone the state enum at the top of the match (`match &self.state.clone()` / `match &self.browse_state.clone()`), then mutate `self.state` inside the arm. If a new branch is added that reads `query` from the cloned arm *and* calls `self.state = AppState::Browse` partway through, the clone and the live field diverge in a non-obvious way. The current code is correct, but the pattern is a latent maintainability trap — the `Filter` arm in `PagesBrowseState::handle_key` (lines 629-654) already extracts `let mut q = query.clone()` from the cloned value and then calls `self.browse_state = AppState::Filter { query: q.clone() }`. A future refactor that forgets the `.clone()` on the outer match will cause a borrow-checker error that tempts the developer to repeat the same clone-then-mutate pattern.

**Fix:** Use a helper method that returns the current query string, or restructure to extract the `query` before the match:

```rust
// At top of handle_key:
let current_query = if let AppState::Filter { query } = &self.browse_state {
    Some(query.clone())
} else {
    None
};
match self.browse_state {  // move, not reference — avoids clone + borrow conflict
    ...
}
```

---

### WR-06: `render_filter_overlay` is called for both the spaces screen (implicitly) and the pages screen but the pages overlay does not restore the list scroll position after Esc

**File:** `src/tui/app.rs:337-339` and mirror `src/tui/app.rs:633-636`

**Issue:** When Esc closes the filter overlay, `apply_filter("")` is called which always resets `list_state.select(Some(0))`. A user who had scrolled to item 50, opened the filter, typed nothing, and pressed Esc is silently jumped back to the top of the list. The Pitfall 5 comment justifies resetting selection when the filtered set changes, but resetting it when the user exits without changing the query discards their scroll position unnecessarily.

**Fix:** Only call `apply_filter("")` when the query was non-empty:

```rust
KeyCode::Esc => {
    let was_filtering = matches!(&self.browse_state, AppState::Filter { query } if !query.is_empty());
    self.browse_state = AppState::Browse;
    if was_filtering {
        self.apply_filter("");
    }
    return KeyAction::None;
}
```

---

## Info

### IN-01: Pervasive `#[allow(dead_code)]` suppression hides real dead code

**File:** `src/tui/app.rs:32,39,87,395,415,433,666` and `src/tui/screens/pages.rs:12`

**Issue:** Large blocks of the public API are annotated `#[allow(dead_code)]` or suppressed at the file level (`#![allow(dead_code)]`). This is noted as intentional pending Phase 6 wiring, but it means the compiler will not warn when genuinely dead items accumulate. At least `StatusMessage`, `StatusStyle`, `Screen`, `PagesBrowseState`, and most `KeyAction` variants are suppressed. Once wiring is complete, these annotations should be removed.

**Fix:** Track removal of these annotations as a Phase 6 task. Consider using `#[cfg_attr(not(test), allow(dead_code))]` as an intermediate step to retain compile-time visibility in production builds.

---

### IN-02: `body_line.to_string()` allocates a new String for every line of stripped XML body

**File:** `src/tui/screens/pages.rs:280`

**Issue:** `stripped.lines()` yields `&str` slices, but `Line::from(body_line.to_string())` allocates an owned `String` per line. On pages with large bodies (capped at 200 lines) this is 200 allocations per render frame. `Line::from` accepts `&str` but the borrow lifetime of `stripped` must outlive `lines`. The fix below avoids allocations.

**Fix:**

```rust
let stripped = strip_storage_xml(body);
let lines_to_render: Vec<Line> = stripped
    .lines()
    .take(200)
    .map(|l| Line::from(l.to_owned()))  // same allocation count but explicit
    .collect();
let truncated = stripped.lines().count() > 200;
lines.extend(lines_to_render);
if truncated {
    lines.push(Line::from(Span::styled("…", Style::default().fg(Color::DarkGray))));
}
```

Note: this is flagged as Info (not a correctness issue) but the current logic also has a subtle off-by-one: the loop breaks **after** pushing the "…" line at `count == 200`, meaning 200 content lines + 1 ellipsis = 201 lines are rendered, not exactly 200 as the comment claims.

---

### IN-03: `unwrap()` on already-checked `Option` in `render_preview_pane`

**File:** `src/tui/screens/pages.rs:202`

**Issue:** Line 187 checks `if selected.is_none()` and returns early; line 202 calls `selected.unwrap()`. This is safe as written but introduces an implicit invariant. Prefer `if let Some(selected_page) = selected` at line 187's guard inversion, or call `selected_page()` once and bind to a `let`:

```rust
let Some(selected_page) = state.selected_page() else {
    // render "Select a page…" and return
    return;
};
let detail = state.preview_cache.get(&selected_page.id);
```

---

### IN-04: Magic number `4` in `inner_width` calculation is underdocumented

**File:** `src/tui/screens/pages.rs:151`

**Issue:** The comment `// 4 = 1 (RIGHT border) + 2 (HighlightSpacing gutter "> ") + 1 (padding)` is correct but the arithmetic is spread across a comment. If the `Padding::horizontal(1)` or the `highlight_symbol` are ever changed the magic number `4` will silently become wrong. Extract as a named constant or derive it from the configuration above.

```rust
const LIST_PANE_GUTTER: u16 = 1  // right border
    + 2   // "> " highlight symbol
    + 1;  // horizontal padding (one side)
let inner_width = area.width.saturating_sub(LIST_PANE_GUTTER) as usize;
```

---

_Reviewed: 2026-05-07T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
