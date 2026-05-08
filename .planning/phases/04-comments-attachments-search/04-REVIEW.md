---
phase: 04-comments-attachments-search
reviewed: 2026-05-08T00:00:00Z
depth: standard
files_reviewed: 14
files_reviewed_list:
  - Cargo.toml
  - src/api/attachment.rs
  - src/api/comment.rs
  - src/api/mod.rs
  - src/cli/attachment.rs
  - src/cli/comment.rs
  - src/cli/mod.rs
  - src/cli/page.rs
  - src/main.rs
  - src/tui/app.rs
  - src/tui/mod.rs
  - src/tui/screens/comments.rs
  - src/tui/screens/mod.rs
  - src/tui/screens/pages.rs
findings:
  critical: 3
  warning: 7
  info: 4
  total: 14
status: issues_found
---

# Phase 04: Code Review Report

**Reviewed:** 2026-05-08T00:00:00Z
**Depth:** standard
**Files Reviewed:** 14
**Status:** issues_found

## Summary

This phase adds comments listing (TUI + CLI), attachment list/get/add, and page search. The overall structure is consistent and the security posture for path traversal (digits-only IDs) and editor injection (no shell) is solid.

Three blockers were identified: a predictable temp-file name that allows a symbolic-link attack; a missing pagination guard that silently truncates results for the attachment list; and a race condition in the event loop where per-tick `tick()` calls are skipped for the active screen when a non-PagesBrowse screen is on top.

Seven warnings cover logic gaps: the `attachment get` command unconditionally overwrites existing files; `comment add` leaves a temp file on disk when the editor exits non-zero; the `handle_open_browser` (spaces) fallback ignores `space_key` entirely and relies only on the current list selection; the search `400` error body is included raw in the error message without sanitisation; `CommentsBrowseState::select_next` does not wrap (inconsistent with spaces/pages navigation); the `help bar` in the comments view advertises an `o` (open) binding that has no keyboard handler; and the `attachment list` command silently truncates at 50 results with no indication to the user.

---

## Critical Issues

### CR-01: Predictable temp-file path allows symlink attack in `comment add`

**File:** `src/cli/comment.rs:39`
**Issue:** The temp file path is `$TMPDIR/ccli-comment-{page_id}.txt` where `page_id` is a validated digit string. A local attacker can pre-create a symlink at that path (e.g. `ccli-comment-12345.txt -> /home/victim/.ssh/authorized_keys`) and `std::fs::write` will follow it, overwriting the target with the user's comment text. The page-edit path in `src/cli/page.rs:220` and `src/tui/mod.rs:496` shares the same pattern (`ccli-edit-{id}.xml`), and the page-create path in `src/cli/page.rs:150` uses the process PID but is otherwise still predictable for the window between process start and the write.

**Fix:** Use a randomly-named temp file via `tempfile::NamedTempFile` (already in `[dev-dependencies]` — move it to `[dependencies]`), or at minimum use `O_CREAT | O_EXCL` semantics:
```rust
// Replace the manual temp-path construction with:
let tmp = tempfile::Builder::new()
    .prefix("ccli-comment-")
    .suffix(".txt")
    .tempfile()
    .context("Failed to create temp file for comment")?;
let temp_path = tmp.path().to_path_buf();
// Keep the NamedTempFile alive until after the editor returns.
// The file is automatically deleted on drop if no explicit remove_file is called.
```
Apply the same fix to the edit workflows in `src/cli/page.rs` and `src/tui/mod.rs`.

---

### CR-02: `attachment list` silently truncates at 50 results; `list_attachments` has no pagination

**File:** `src/api/attachment.rs:63-97`
**Issue:** `GET /rest/api/content/{id}/child/attachment?limit=50` fetches exactly 50 records and stops. Confluence paginates attachment lists with `_links.next`; pages with more than 50 attachments are silently truncated. The user sees a partial list with no warning. The companion `get_attachment` function performs an exact-match lookup in this truncated list, meaning files beyond the 50th position become unreachable via `ccli attachment get`.

The comment API (`list_comments`) has the same issue (hard-coded `limit=50`, no pagination), but the design doc explicitly defers pagination for comments. No such deferral is documented for attachments.

**Fix:** Either implement cursor-based pagination (follow `_links.next` in the response), or at minimum surface a warning when `size >= 50`:
```rust
// In list_attachments, parse the full response:
#[derive(Debug, Clone, Deserialize)]
struct AttachmentListResponse {
    pub results: Vec<Attachment>,
    pub size: Option<u64>,
}

// After parsing:
if parsed.results.len() >= 50 {
    tracing::warn!(
        "Attachment list may be truncated: received {} results (limit=50). \
         Use the Confluence web UI to see all attachments.",
        parsed.results.len()
    );
}
```
For `get_attachment`, propagate a clearer error when the file is not found on a page that may have more than 50 attachments.

---

### CR-03: `tick()` is not called for the active screen when a `CommentsBrowse` screen is on top, breaking the spinner for nested `PagesBrowse` states

**File:** `src/tui/mod.rs:213-230`
**Issue:** The event loop only calls `tick()` on the *top* screen:
```rust
// Phase 3: tick pages screen
if let Some(Screen::PagesBrowse { state, .. }) = app.screen_stack.last_mut() {
    state.tick();
    ...
}
// Phase 4: tick comments screen
if let Some(Screen::CommentsBrowse { state, .. }) = app.screen_stack.last_mut() {
    state.tick();
}
```
Both branches match `last_mut()`. When `CommentsBrowse` is on top, the first `if let` does not match (correct), but neither does the second for any `PagesBrowse` screen that may be below it on the stack. More importantly, both branches are **mutually exclusive if-lets on the same last element**: if a `PagesBrowse` is on top, the `CommentsBrowse` branch never fires; and vice-versa. This is correct for spinner advancement because only the top screen is rendered. However, `pages_preview_tx` pending-fetch dispatch is also inside the `PagesBrowse` tick block and will **never fire** while `CommentsBrowse` is on top, leaving a pending preview fetch permanently stuck. When the user presses `Esc` to pop back to `PagesBrowse`, the preview that was supposed to load will never be re-triggered because `pending_preview_id` was already cleared and `last_selection_change` is `None`.

**Fix:** Separate the preview-dispatch logic from the tick and ensure it runs even when it is not the topmost screen, or accept that previews do not load while a comment screen is open and reset `pending_preview_id` on pop:
```rust
// When popping CommentsBrowse, re-trigger the preview debounce:
KeyAction::PopScreen => {
    app.screen_stack.pop();
    // Re-arm preview debounce for the now-visible PagesBrowse
    if let Some(Screen::PagesBrowse { state, .. }) = app.screen_stack.last_mut() {
        if let Some(id) = state.selected_id() {
            if !state.preview_cache.contains_key(&id) {
                state.last_selection_change = Some(std::time::Instant::now());
            }
        }
    }
}
```

---

## Warnings

### WR-01: `attachment get` silently overwrites existing files without confirmation

**File:** `src/cli/attachment.rs:99-100`
**Issue:** `std::fs::write(&dest, &bytes)` unconditionally overwrites the destination file if it already exists. A user running `ccli attachment get 12345 report.pdf` when `report.pdf` exists in the working directory will lose their existing file silently.

**Fix:** Check for existence before writing and bail or prompt:
```rust
if dest.exists() {
    anyhow::bail!(
        "attachment get: '{}' already exists. Use --output to specify a different path.",
        dest.display()
    );
}
```

---

### WR-02: `comment add` leaves temp file on disk when editor exits non-zero

**File:** `src/cli/comment.rs:42-52`
**Issue:** `launch_editor` returns `Err` when the editor exits non-zero. The error is propagated upward via `?` at line 42, so `std::fs::remove_file(&temp_path)` at line 50 is only reached inside the `Err(_)` branch of the `wrap_plain_text_to_storage_xml` match — not on editor failure. The temp file at `$TMPDIR/ccli-comment-{id}.txt` is left on disk after each editor crash, accumulating over repeated runs.

**Fix:**
```rust
launch_editor(&temp_path)?;
let plain_text = std::fs::read_to_string(&temp_path)
    .with_context(|| format!("Failed to read comment from {:?}", temp_path))?;

// Wrap — always attempt cleanup regardless of outcome
let wrap_result = wrap_plain_text_to_storage_xml(&plain_text);
let _ = std::fs::remove_file(&temp_path); // best-effort cleanup here, not only in the Err branch

let xml = match wrap_result {
    Ok(x) => x,
    Err(_) => anyhow::bail!("comment add: editor returned empty content — comment not posted"),
};
```

---

### WR-03: `handle_open_browser` ignores its `space_key` argument and relies solely on the current list selection

**File:** `src/tui/mod.rs:338-391`
**Issue:** The function signature takes `space_key: &str` but never uses it for the actual URL resolution. Instead it re-derives the space from `app.list_state.selected()`. If the selection changes between the key event and the browser-open handler (race), or if `filtered_indices` has drifted, the wrong space could be opened. The `space_key` argument is effectively dead code. The parameter should either be removed (and the URL resolved before dispatch) or be used authoritatively.

**Fix:** Resolve the URL directly from `space_key` to avoid index aliasing:
```rust
fn handle_open_browser(
    app: &mut App,
    space_key: &str,
    base_url: &str,
    terminal: &mut ratatui::DefaultTerminal,
) -> anyhow::Result<()> {
    let space = app.spaces.iter().find(|s| s.key == space_key);
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
    // ... rest unchanged
}
```

---

### WR-04: `page search` includes raw Confluence API error body in the `400` error message without sanitisation

**File:** `src/cli/page.rs:369-371`
**Issue:** When Confluence returns HTTP 400 for an invalid CQL expression, the entire raw response body is included verbatim in the error returned to the user:
```rust
let body = resp.text().await.unwrap_or_default();
return Err(AppError::Api(format!("invalid CQL — {}", body)).into());
```
Confluence's 400 body can contain large HTML error pages, stack traces, or multi-kilobyte JSON blobs — all of which are printed to the user's terminal. At worst, if the server is under attacker control or returns ANSI escape sequences, this creates a terminal injection vector.

**Fix:** Extract only the relevant portion (e.g. the `message` field from JSON, or truncate):
```rust
let body = resp.text().await.unwrap_or_default();
// Try to extract a short message from JSON; fallback to first 200 chars
let short = serde_json::from_str::<serde_json::Value>(&body)
    .ok()
    .and_then(|v| v["message"].as_str().map(|s| s.to_string()))
    .unwrap_or_else(|| body.chars().take(200).collect());
return Err(AppError::Api(format!("invalid CQL — {}", short)).into());
```

---

### WR-05: `CommentsBrowseState::select_next` does not wrap; inconsistent with spaces/pages navigation

**File:** `src/tui/app.rs:811-819`
**Issue:** `CommentsBrowseState::select_next` clamps at the last item (`.min(self.comments.len() - 1)`) instead of wrapping to index 0, while `App::select_next` and `PagesBrowseState::select_next` both wrap. This breaks the muscle memory of users who navigate with `j` and expect the list to wrap. `select_prev` correctly uses `saturating_sub(1)` which clamps at 0, consistent with the non-wrapping choice, so the clamping is at least internally consistent for comments — but it is deliberately inconsistent with the other two screens.

**Fix:** Either wrap on all screens (preferred for consistency):
```rust
pub fn select_next(&mut self) {
    if self.comments.is_empty() {
        return;
    }
    let current = self.list_state.selected().unwrap_or(0);
    let next = if current + 1 >= self.comments.len() { 0 } else { current + 1 };
    self.list_state.select(Some(next));
}

pub fn select_prev(&mut self) {
    if self.comments.is_empty() {
        return;
    }
    let current = self.list_state.selected().unwrap_or(0);
    let prev = if current == 0 { self.comments.len() - 1 } else { current - 1 };
    self.list_state.select(Some(prev));
}
```
Or document the deliberate difference in a code comment.

---

### WR-06: Comments help bar advertises `o` (open in browser) with no keyboard handler

**File:** `src/tui/screens/comments.rs:248-256`
**Issue:** The help bar rendered by `render_help_bar` includes:
```
o  open   ?  help   Esc  back
```
But `CommentsBrowseState::handle_key` has no handler for `KeyCode::Char('o')` — it falls through to `_ => KeyAction::None`. Pressing `o` silently does nothing. This is misleading to users who expect the key to open the current comment's page in the browser.

**Fix:** Either implement the `o` handler (likely `KeyAction::OpenBrowser` with the parent page's webui URL, which would need to be stored in `CommentsBrowseState`) or remove `o` from the help bar until the feature is implemented:
```rust
// Remove the 'o' entry from render_help_bar:
let line = Line::from(vec![
    Span::raw(" "),
    key("?"),
    Span::raw("  help   "),
    key("Esc"),
    Span::raw("  back"),
]);
```

---

### WR-07: `add_attachment` reads the entire file into memory before upload, with no size guard

**File:** `src/api/attachment.rs:167-168`
**Issue:** `std::fs::read(file_path)` loads the entire file into a `Vec<u8>` before constructing the multipart form. There is no file-size check before the read. Uploading a large file (e.g. a multi-gigabyte video or database backup) will allocate the entire file in RAM, potentially exhausting process memory. The metadata stat check in `src/cli/attachment.rs:115-117` is only used for the upload confirmation message and does not gate the actual read.

**Fix:** Add a size guard in `handle_add` before calling `add_attachment`:
```rust
const MAX_UPLOAD_BYTES: u64 = 512 * 1024 * 1024; // 512 MiB — adjust per requirements
if size > MAX_UPLOAD_BYTES {
    anyhow::bail!(
        "attachment add: '{}' is {} — exceeds the {} limit",
        filename,
        human_size(size),
        human_size(MAX_UPLOAD_BYTES)
    );
}
```
Alternatively, switch `add_attachment` to use `reqwest::Body::wrap_stream` with a `tokio::fs::File` stream to avoid loading the file into memory entirely.

---

## Info

### IN-01: `sanitize_id` is duplicated across three files with identical implementations

**File:** `src/cli/comment.rs:68-77`, `src/cli/attachment.rs:146-155`, `src/cli/page.rs:263-272`
**Issue:** The exact same `sanitize_id` function is copy-pasted in three CLI handler files. A divergence (e.g. a future change to allow hex IDs) would require updating all three copies.

**Fix:** Extract to a shared `src/cli/util.rs` or `src/cli/mod.rs` and re-export:
```rust
// src/cli/mod.rs
pub(crate) fn sanitize_id(s: &str) -> Option<String> {
    if s.is_empty() { return None; }
    if s.chars().all(|c| c.is_ascii_digit()) { Some(s.to_string()) } else { None }
}
```

---

### IN-02: `tempfile` is listed only in `[dev-dependencies]` but is needed in production code

**File:** `Cargo.toml:36`
**Issue:** `tempfile = "3"` is under `[dev-dependencies]`. If CR-01 is fixed by using `tempfile::NamedTempFile` in production code (the correct fix), the crate must be moved to `[dependencies]`. This is currently a latent issue — the fix for CR-01 will cause a compile error until `Cargo.toml` is updated.

**Fix:** Move `tempfile` to `[dependencies]`:
```toml
[dependencies]
# ... existing deps ...
tempfile = "3"
```

---

### IN-03: `#[allow(dead_code)]` on public structs masks missing wiring

**File:** `src/api/attachment.rs:22`, `src/api/comment.rs:17-18`, `src/tui/app.rs:33`, etc.
**Issue:** Many public structs and their fields carry `#[allow(dead_code)]`. In several cases (e.g. `CommentLinks::self_url`, `AttachmentLinks::webui`) the field is genuinely unused. These suppressions hide whether the field is intentionally unused or accidentally unconnected. The `webui` field on `AttachmentLinks` would be needed if a future "open attachment in browser" feature were added; having it silenced makes the gap invisible.

**Fix:** Remove `#[allow(dead_code)]` from individual unused fields and add a `// TODO:` comment explaining the planned use, or remove fields that will never be used. Keep the attribute only on structs that are fully consumed by deserialisation.

---

### IN-04: `run_editor_workflow` in `src/tui/mod.rs` duplicates `launch_editor` from `src/cli/page.rs` and `src/cli/comment.rs`

**File:** `src/tui/mod.rs:553-571`, `src/cli/page.rs:412-425`, `src/cli/comment.rs:80-92`
**Issue:** The pattern "read `$EDITOR`/`$VISUAL`/`vi`, spawn with `.arg(path)`, check `.status()`" appears three times with nearly identical implementations. A security fix (e.g. restricting allowed editor names) would need to be applied in three places.

**Fix:** Extract to a shared helper in a new `src/editor.rs` or `src/cli/util.rs`:
```rust
pub fn launch_editor(path: &std::path::Path) -> anyhow::Result<()> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor)
        .arg(path)
        .status()
        .with_context(|| format!("Failed to launch $EDITOR ({})", editor))?;
    if !status.success() {
        anyhow::bail!("$EDITOR exited non-zero (status {:?})", status.code());
    }
    Ok(())
}
```

---

_Reviewed: 2026-05-08T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
