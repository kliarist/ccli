---
phase: 03-pages-blog-posts
reviewed: 2026-05-07T00:00:00Z
depth: standard
files_reviewed: 12
files_reviewed_list:
  - src/api/mod.rs
  - src/api/page.rs
  - src/cli/blog.rs
  - src/cli/mod.rs
  - src/cli/page.rs
  - src/main.rs
  - src/output/mod.rs
  - src/output/xml.rs
  - src/tui/app.rs
  - src/tui/mod.rs
  - src/tui/screens/mod.rs
  - src/tui/screens/pages.rs
findings:
  critical: 3
  warning: 5
  info: 3
  total: 11
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-05-07
**Depth:** standard
**Files Reviewed:** 12
**Status:** issues_found

## Summary

This review covers the full Phase 03 implementation: Confluence content API (`api/page.rs`), CLI subcommands (`cli/page.rs`, `cli/blog.rs`, `cli/mod.rs`), the output subsystem (`output/mod.rs`, `output/xml.rs`), main dispatch (`main.rs`), and the TUI pages-browse screen plus event loop (`tui/app.rs`, `tui/mod.rs`, `tui/screens/pages.rs`, `tui/screens/mod.rs`).

The security baseline is solid: the editor is launched via `Command::new(editor).arg(path)` with no shell, page IDs are digit-validated before being used in temp-file paths, and the PAT is skipped from tracing. Three blockers were found: `update_page` treats HTTP 204 as an error (Confluence DC sometimes responds 204 to a successful PUT), `handle_edit_page` in the TUI hardcodes `ContentType::Page` so blog-post editing will corrupt the Confluence content type, and the `apply_filter` index-recovery logic produces wrong results (wrong item selected or items dropped) whenever two items share an identical haystack string. Five warnings cover a hardcoded `/tmp` path, a version-1 fallback that silently triggers 409s, a never-incremented progress counter shown in the loading UI, continuous polling of a closed oneshot channel, and unencoded space-key values interpolated directly into URL query strings.

---

## Critical Issues

### CR-01: `update_page` treats HTTP 204 as an error — saves always fail on Confluence instances that return 204

**File:** `src/api/page.rs:377-390`

**Issue:** Confluence Data Center (and Confluence Cloud) may return `HTTP 204 No Content` on a successful `PUT /rest/api/content/{id}`. The `update_page` match only accepts `200 => Ok(())` (line 378); a 204 response falls through to the catch-all `status => Err(AppError::Api("Unexpected HTTP 204 updating page …"))`. Every edit on an affected instance appears to the user as a failure despite the page having been saved. In the TUI this shows "Error saving: API error: Unexpected HTTP 204" while the page is actually updated, leading users to retry and potentially cause 409 conflicts.

**Fix:**
```rust
match resp.status().as_u16() {
    200 | 204 => Ok(()),
    409 => Err(AppError::Api(format!(
        "Conflict: page was updated by someone else (version {}).",
        current_version
    ))),
    401 | 403 => Err(AppError::Auth( ... )),
    status => Err(AppError::Api(format!(
        "Unexpected HTTP {} updating page {}",
        status, page_id
    ))),
}
```

---

### CR-02: `handle_edit_page` in TUI hardcodes `ContentType::Page` — editing a blog post sends wrong type to Confluence

**File:** `src/tui/mod.rs:442-450`

**Issue:** `handle_edit_page` always calls `update_page` with `ContentType::Page`:

```rust
let put_result = update_page(
    client, page_id, &title, &space_key, &new_xml,
    ContentType::Page,   // ← always Page
    current_version,
).await;
```

The `Screen::PagesBrowse` struct carries a `content_type: ContentType` field for exactly this reason (line 229 pushes it with `ContentType::Page` today, but the field exists to support `ContentType::BlogPost`). When blog-browse is wired — or even now if someone manually constructs a `BlogPost` browse screen — pressing `e` on a blog post sends `"type":"page"` in the PUT body. Confluence will either reject the request or silently re-classify the blog post as a page. Data loss risk.

**Fix:** Thread `content_type` through from the active screen:

```rust
// In the KeyAction::EditPage arm (tui/mod.rs ~249):
KeyAction::EditPage(page_id) => {
    let ct = app.screen_stack.last()
        .and_then(|s| if let Screen::PagesBrowse { content_type, .. } = s {
            Some(*content_type)
        } else { None })
        .unwrap_or(ContentType::Page);
    handle_edit_page(&mut app, &client, &page_id, ct, terminal).await;
}

// Update handle_edit_page signature:
async fn handle_edit_page(
    app: &mut App,
    client: &Client,
    page_id: &str,
    content_type: ContentType,  // ← new
    terminal: &mut ratatui::DefaultTerminal,
)
// Pass `content_type` to update_page(…).
```

---

### CR-03: `apply_filter` index-recovery via `Vec::position` collapses duplicate titles/haystacks — wrong item selected or items dropped

**File:** `src/tui/app.rs:228-231` (spaces) and `src/tui/app.rs:546-549` (pages)

**Issue:** After `pattern.match_list(...)` returns scored matches, both `App::apply_filter` and `PagesBrowseState::apply_filter` map each matched string back to its original index using:

```rust
.filter_map(|(h, _)| haystacks.iter().position(|x| x == h))
```

`position` returns the index of the **first** element equal to `h`. When two pages share the same title (or two spaces share identical `"{key} {name}"` composites), only the first match's index is ever returned. The second item is mapped to the first item's index, producing a duplicate index in `filtered_indices` (same item shown twice) or a missing item. Additionally, if nucleo deduplicates identical-string inputs before scoring, items may be silently dropped from the match results.

**Fix:** Recover indices by carrying them through the match rather than recovering by string equality:

```rust
// PagesBrowseState::apply_filter (same pattern for App::apply_filter):
let haystacks: Vec<String> = self.pages.iter().map(|p| p.title.clone()).collect();
let matched: Vec<(String, u32)> = pattern
    .match_list(haystacks.iter().map(|s| s.as_str()), &mut matcher)
    .into_iter()
    .map(|(s, score)| (s.to_owned(), score))
    .collect();

// Use a mutable "remaining" list so duplicates are consumed one-for-one:
let mut remaining: Vec<(usize, &str)> = haystacks
    .iter()
    .enumerate()
    .map(|(i, s)| (i, s.as_str()))
    .collect();
self.filtered_indices = matched
    .iter()
    .filter_map(|(h, _)| {
        let pos = remaining.iter().position(|(_, s)| s == h)?;
        let (orig_idx, _) = remaining.remove(pos);
        Some(orig_idx)
    })
    .collect();
```

---

## Warnings

### WR-01: `handle_edit_typed` uses hardcoded `/tmp` path; inconsistent with `handle_create_typed` which uses `std::env::temp_dir()`

**File:** `src/cli/page.rs:206` and `src/tui/mod.rs:422`

**Issue:** `handle_create_typed` (line 142) correctly uses `std::env::temp_dir()` to find the system temp directory. `handle_edit_typed` (line 206) and the TUI's `handle_edit_page` (line 422) hard-code `/tmp/ccli-edit-{id}.xml`. On non-Unix systems (`TMPDIR`-redirected environments, Windows if ever targeted) the write will fail with a confusing error rather than a graceful message about a missing temp directory.

**Fix:**
```rust
// cli/page.rs:206
let temp_path = std::env::temp_dir().join(format!("ccli-edit-{}.xml", id));

// tui/mod.rs:422
let temp_path = std::env::temp_dir().join(format!("ccli-edit-{}.xml", page_id));
```

---

### WR-02: `unwrap_or(1)` version fallback silently triggers 409 Conflict on pages where version is absent from the API response

**File:** `src/cli/page.rs:191` and `src/tui/mod.rs:403`

**Issue:** When `detail.version` is `None` (possible if the Confluence instance omits version metadata), both handlers default to version number `1`:

```rust
let current_version = detail.version.as_ref().map(|v| v.number).unwrap_or(1);
```

`update_page` then sends `version.number = 2`. Any page on version > 1 will receive a 409 Conflict, and the user's edit is rejected with no indication that the real problem is a missing version field. The fallback of `1` cannot be correct for any page that has ever been edited.

**Fix:** Treat a missing version as a fatal fetch-path error:
```rust
// cli/page.rs
let current_version = detail.version.as_ref().map(|v| v.number)
    .ok_or_else(|| anyhow::anyhow!(
        "Page version was not returned by the API — cannot safely update."
    ))?;

// tui/mod.rs (inside handle_edit_page, before ratatui::restore())
let current_version = match detail.version.as_ref().map(|v| v.number) {
    Some(v) => v,
    None => {
        set_pages_status(app,
            "Page version missing from API — cannot safely update.".to_string(),
            StatusStyle::Error);
        return;
    }
};
```

---

### WR-03: `spaces_rx` oneshot receiver is polled on every event-loop iteration after the fetch completes

**File:** `src/tui/mod.rs:126-130`

**Issue:** `spaces_rx` is a `oneshot::Receiver`. Once `try_recv()` returns `Ok(_)`, the channel is closed and every subsequent `try_recv()` returns `Err(TryRecvError::Closed)`. The loop polls it unconditionally on every iteration (every 100 ms) for the entire session. For a 10-minute TUI session this is ~6 000 no-op `try_recv` calls on a dead channel. More importantly, if this path ever grows (e.g. supporting re-fetch), the forgotten state will cause logical errors.

**Fix:**
```rust
// Wrap in Option; take it once:
let mut spaces_rx: Option<oneshot::Receiver<Result<Vec<Space>, AppError>>> = Some(spaces_rx);

// In the loop:
if let Some(rx) = spaces_rx.as_mut() {
    if let Ok(fetch_result) = rx.try_recv() {
        spaces_rx = None;   // stop polling
        match fetch_result {
            Ok(spaces) => app.set_spaces(spaces),
            Err(e) => app.set_fetch_error(&e),
        }
    }
}
```

---

### WR-04: `pages_fetched_count` and `spaces_fetched_count` are never incremented — the loading spinner always shows "0 fetched"

**File:** `src/tui/app.rs:82,104,427,449` and `src/tui/screens/pages.rs:315`

**Issue:** Both `App::spaces_fetched_count` and `PagesBrowseState::pages_fetched_count` are initialized to `0` and never updated anywhere in the codebase. The loading status bar at `pages.rs:315` renders:

```
◐  Loading pages… 0 fetched
```

This is always `0` regardless of how many pages have been fetched. The counter is meaningless as shown. Since `list_all_pages` runs in a `tokio::spawn` and only sends the final result, per-page progress requires a progress channel.

**Fix:** Either (a) remove the counter fields and display only the spinner without a count, or (b) introduce a progress mpsc channel. Option (a) is lower risk:

```rust
// pages.rs render_status_bar, Loading arm:
Line::from(vec![
    Span::styled(format!(" {}", frame), Style::default().fg(Color::Cyan)),
    Span::raw(format!("  Loading {}…", kind_plural)),
])
```

---

### WR-05: `space_key` interpolated directly into URL query string without percent-encoding — allows query-string injection

**File:** `src/api/page.rs:150-157`

**Issue:** The URL for `list_all_pages` is built as:

```rust
format!(
    "{}/rest/api/content?spaceKey={}&type={}&start={}&limit={}&expand=version,ancestors",
    client.base_url().trim_end_matches('/'),
    space_key,   // ← no encoding
    content_type.as_api_str(),
    start,
    limit,
)
```

A space key containing `&`, `=`, `+`, or `%` (technically valid in Confluence) would break the query string. A key such as `DEV&type=blogpost` would override the `type` parameter. The `page_id` in `get_page_detail` is path-segment, not a query parameter, and is digit-validated, so that path is safe. But `space_key` is user-supplied (CLI arg or TUI DrillDown) and receives no sanitization.

**Fix:** Use `reqwest`'s `.query()` builder to let `reqwest` percent-encode the values:
```rust
let resp = client
    .inner()
    .get(format!("{}/rest/api/content", client.base_url().trim_end_matches('/')))
    .query(&[
        ("spaceKey", space_key),
        ("type", content_type.as_api_str()),
        ("start", &start.to_string()),
        ("limit", &limit.to_string()),
        ("expand", "version,ancestors"),
    ])
    .send()
    .await
    ...
```

---

## Info

### IN-01: `extract_space_key_from_webui` is duplicated verbatim in `cli/page.rs` and `tui/mod.rs`

**File:** `src/cli/page.rs:275-284` and `src/tui/mod.rs:503-512`

**Issue:** The function body, comments, and test cases are copy-pasted identically. Any future fix (e.g. handling Confluence Cloud's `/wiki/spaces/DEV/pages/...` URL format) must be applied in two places independently.

**Fix:** Move the function to `src/api/page.rs` or a new `src/util.rs` and import it in both call sites.

---

### IN-02: `std::env::set_var` in test `run_editor_workflow_returns_error_when_editor_fails` is unsound under parallel test execution

**File:** `src/tui/mod.rs:570-572`

**Issue:** The test mutates the process-global `EDITOR` environment variable with no mutex:

```rust
std::env::set_var("EDITOR", "this-binary-does-not-exist-xyzzy-12345");
let result = run_editor_workflow(&path, "<p>test</p>");
std::env::remove_var("EDITOR");
```

Rust tests run in parallel threads by default. A concurrent test that reads `EDITOR` (e.g. any future `launch_editor` test) may observe the injected value. The `config` module already uses `ENV_LOCK: Mutex<()>` for this purpose; the same guard should be acquired here.

---

### IN-03: Pervasive `#[allow(dead_code)]` on public API items in `api/page.rs` suppresses genuine compiler warnings

**File:** `src/api/page.rs:25,42,53,60,73,80,88,95,103,117,123,138,208,258,333`

**Issue:** Nearly every public struct, enum, and function in `api/page.rs` carries `#[allow(dead_code)]`. This prevents the compiler from warning when fields like `ContentListLinks::base` or `StorageBody::representation` are never read. If a future refactor removes a use site, the compiler cannot alert to the orphaned code. These suppressions should be removed once the items are connected to their consumers (Phase 4+).

---

_Reviewed: 2026-05-07_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
