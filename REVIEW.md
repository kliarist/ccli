---
phase: code-review
reviewed: 2026-05-14T00:00:00Z
depth: deep
files_reviewed: 8
files_reviewed_list:
  - src/cli/comment.rs
  - src/cli/init.rs
  - src/cli/mod.rs
  - src/main.rs
  - src/tui/app.rs
  - src/tui/screens/comments.rs
  - src/tui/screens/pages.rs
  - src/tui/screens/spaces.rs
findings:
  critical: 1
  warning: 5
  info: 4
  total: 10
status: issues_found
---

# Code Review Report

**Reviewed:** 2026-05-14
**Depth:** deep
**Files Reviewed:** 8
**Status:** issues_found

## Summary

Eight files were reviewed covering the `ccli comment list` addition, ANSI color gating for init/error output, clap help styling, the Filtering->Browse UX refactor (`active_filter` persistence and Enter-commit/Esc-revert), and visual refresh of all three TUI screens.

The core filter logic is sound and the API layer is clean. One panic-inducing byte-slice in `mask_pat_hint` is a blocker. There are five warnings covering temp-file leaks, inconsistent `set_spaces` behaviour versus `set_pages`, two production `unwrap()` calls that survive only by convention, a test that misvalidates the new Esc-revert behaviour, and a temp-file symlink risk. Four info-level items cover copy-pasted help modal content in the comments screen, a missing `<q> Quit` shortcut on the pages title bar, and minor structural issues.

---

## Critical Issues

### CR-01: `mask_pat_hint` panics on non-ASCII tokens via invalid byte-slice

**File:** `src/cli/init.rs:141-142`

**Issue:** The function counts characters with `.chars().count()` to determine whether to show prefix/suffix, then slices the string with raw byte indices `token[..2]` and `token[token.len() - 3..]`. These are byte slices, not character slices. If the stored token contains any multi-byte UTF-8 character whose byte boundary does not align with positions 2 or `len-3`, Rust will panic at runtime with "byte index N is not a char boundary".

The comment says "PATs are ASCII, so byte indexing aligns with char indexing," but there is no enforcement of this invariant. A user who (inadvertently or by copy-paste) stores a token beginning with a 3-byte character such as `€` (U+20AC, 3 UTF-8 bytes) would cause `mask_pat_hint` to panic when `ccli init` is re-run to display the current-token hint in the prompt label. This is reachable production code invoked on every re-run of `ccli init`.

**Fix:**

```rust
pub(crate) fn mask_pat_hint(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    let len = chars.len();
    if len == 0 {
        return String::new();
    }
    if len <= 5 {
        return "*".repeat(len);
    }
    let prefix: String = chars[..2].iter().collect();
    let suffix: String = chars[chars.len() - 3..].iter().collect();
    format!("{}*****{}", prefix, suffix)
}
```

This is a one-line conceptual change: collect chars first, then slice the `Vec<char>` by index (always valid), then reassemble. Also add a test covering a token that starts with a multibyte character.

---

## Warnings

### WR-01: Temp file is leaked when `launch_editor` or `add_comment` fails in `handle_add`

**File:** `src/cli/comment.rs:98-118`

**Issue:** The function writes an empty template to `temp_path` at line 98, then calls `launch_editor(&temp_path)?` at line 100. If the editor exits non-zero or cannot be launched, the `?` propagates the error out of `handle_add`, skipping both cleanup paths (line 108 for the empty-content branch, line 118 for the success branch). The temp file remains on disk indefinitely.

The same leak occurs at line 101: if `read_to_string` fails (e.g., the user deleted the file while the editor was open), the error propagates and the file is not removed.

Finally, if `add_comment` at line 113 fails with a network or API error, the `?` propagates past the cleanup at line 118, leaving the user's draft comment in `/tmp/ccli-comment-{id}.txt` with no indication that it can be recovered.

**Fix:** Use a cleanup guard. The simplest approach is to wrap the logic in a closure or use `scopeguard`:

```rust
async fn handle_add(page_id: &str) -> anyhow::Result<()> {
    let id = sanitize_id(page_id)
        .ok_or_else(|| AppError::Api(format!("Invalid id: must be numeric (got {:?})", page_id)))?;
    let cfg = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&cfg)?;

    let temp_path = std::env::temp_dir().join(format!("ccli-comment-{}.txt", id));
    std::fs::write(&temp_path, "")
        .with_context(|| format!("Failed to write template to {:?}", temp_path))?;

    // Ensure cleanup happens on every exit path.
    let result = run_add_inner(&client, &id, &temp_path).await;
    let _ = std::fs::remove_file(&temp_path);
    result
}
```

Where `run_add_inner` contains the editor/wrap/post logic and returns `Result`. If adding `scopeguard` is undesirable, an explicit `let result = ...; cleanup; result` pattern achieves the same effect.

---

### WR-02: `App::set_spaces` does not re-apply `active_filter`, inconsistent with `PagesBrowseState::set_pages`

**File:** `src/tui/app.rs:124-135`

**Issue:** `PagesBrowseState::set_pages` (line 491-496) explicitly re-applies `active_filter` after loading pages:

```rust
pub fn set_pages(&mut self, pages: Vec<Page>) {
    self.pages = pages;
    self.browse_state = AppState::Browse;
    let q = self.active_filter.clone();
    self.apply_filter(&q);   // re-applies committed filter
}
```

`App::set_spaces` (lines 124-135) does the opposite — it unconditionally rebuilds `filtered_indices` as the full range and ignores `active_filter`:

```rust
pub fn set_spaces(&mut self, spaces: Vec<Space>) {
    self.spaces = spaces;
    self.filtered_indices = (0..self.spaces.len()).collect();
    // active_filter is ignored
    ...
}
```

In practice this is not observable because spaces are only fetched once at startup and `active_filter` starts as `""`. However the asymmetry means any future code path that calls `set_spaces` a second time (e.g., a refresh action) would silently clear an active filter, while pages would correctly preserve it. The inconsistency is a latent bug and violates the documented contract for `active_filter` persistence.

**Fix:**

```rust
pub fn set_spaces(&mut self, spaces: Vec<Space>) {
    self.spaces = spaces;
    self.state = AppState::Browse;
    let q = self.active_filter.clone();
    self.apply_filter(&q);   // mirrors PagesBrowseState::set_pages
    if !self.filtered_indices.is_empty() {
        self.list_state.select(Some(0));
        self.last_selection_change = Some(Instant::now());
    } else {
        self.list_state.select(None);
    }
}
```

---

### WR-03: Existing Esc-in-filter test validates OLD behaviour, not the new commit/revert contract

**File:** `src/tui/app.rs:1110-1125`

**Issue:** The test `handle_key_esc_in_filter_closes_overlay_and_restores_list` was written for the previous behaviour where Esc always cleared the filter. After the refactor, Esc now reverts to `active_filter` (the last committed value). The test happens to pass because it never commits a filter — `app.active_filter` stays `""` — so `apply_filter("")` restores the full list. But the test comment asserts `"full list must be restored"`, which is correct only when `active_filter` is empty. If `active_filter` had been committed to `"DEV"`, Esc should show the DEV-filtered list, not the full list. The test does not cover this case and gives false confidence in the revert logic.

**Fix:** Update the existing test to set `app.active_filter` before entering Filtering state, and add a new test for the revert-to-committed path:

```rust
#[test]
fn handle_key_esc_in_filter_reverts_to_committed_filter_not_full_list() {
    let mut app = App::with_spaces(vec![
        make_space("DEV", "Development"),
        make_space("HR", "Human Resources"),
    ]);
    // Commit a filter
    app.handle_key(KeyCode::Char('/'));
    app.handle_key(KeyCode::Char('D'));
    app.handle_key(KeyCode::Char('E'));
    app.handle_key(KeyCode::Char('V'));
    app.handle_key(KeyCode::Enter); // commit -> active_filter = "DEV"
    assert_eq!(app.active_filter, "DEV");
    assert_eq!(app.filtered_indices.len(), 1);

    // Open filter again and refine further
    app.handle_key(KeyCode::Char('/'));
    app.handle_key(KeyCode::Char('x')); // no results for "DEVx"
    assert_eq!(app.filtered_indices.len(), 0);

    // Esc should revert to committed filter "DEV", NOT clear to full list
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.state, AppState::Browse, "Esc should close filter");
    assert_eq!(app.filtered_indices.len(), 1, "must revert to committed DEV filter, not full list");
    assert_eq!(app.active_filter, "DEV", "active_filter must remain committed value");
}
```

---

### WR-04: Two `.unwrap()` calls in production render paths survive only by convention

**Files:** `src/tui/screens/pages.rs:267`, `src/tui/screens/spaces.rs:207`

**Issue:** Both render functions guard against `None` with an early return, then call `.unwrap()` on the same value:

`pages.rs:246-267`:
```rust
let selected = state.selected_page();
if selected.is_none() {
    // ... early return
    return;
}
let selected_page = selected.unwrap(); // panics if refactored
```

`spaces.rs:197-207`:
```rust
if selected_space.is_none() {
    // ... early return
    return;
}
let space = selected_space.unwrap(); // panics if refactored
```

The guard and the unwrap are separated by multiple lines. A future refactor that adds another early-return path between the guard and the unwrap would introduce a panic. The `if let Some` / `else` pattern eliminates the risk entirely.

**Fix:**

```rust
// pages.rs
let Some(selected_page) = state.selected_page() else {
    // render placeholder and return
    return;
};
let detail = state.preview_cache.get(&selected_page.id);
```

```rust
// spaces.rs
let Some(space) = selected_space else {
    // render placeholder and return
    return;
};
let detail = app.preview_cache.get(&space.key);
```

---

### WR-05: Predictable temp file path is vulnerable to symlink attack

**File:** `src/cli/comment.rs:97-98`

**Issue:** The temp file path is `std::env::temp_dir()/ccli-comment-{id}.txt` where `id` is a numeric page ID known to the commenter. `std::fs::write` follows symlinks. On a shared system, a local attacker who knows the page ID can pre-create `/tmp/ccli-comment-{id}.txt` as a symlink to any file writable by the victim user (e.g., `~/.ssh/authorized_keys`, a crontab, or a config file). When the victim runs `ccli comment add {id}`, `fs::write(..., "")` silently truncates the target of the symlink to an empty file.

The same pattern exists in `handle_edit_page` in `tui/mod.rs` (out of scope but worth noting).

**Fix:** Use `tempfile::NamedTempFile` (already in dev-dependencies; promote to dependencies) which creates a file with a random suffix and `O_EXCL` atomically, preventing symlink substitution. If `tempfile` cannot be added, use `std::fs::OpenOptions::new().write(true).create_new(true).open(...)` which fails if the path already exists, and append a random suffix to the name:

```rust
// Add to Cargo.toml [dependencies]: tempfile = "3"
use tempfile::NamedTempFile;

let mut tmp = NamedTempFile::new_in(std::env::temp_dir())
    .context("Failed to create temp file")?;
let temp_path = tmp.path().to_path_buf();
tmp.write_all(b"").context("Failed to write template")?;
tmp.flush()?;
// pass temp_path to launch_editor; persist() to keep file across editor call
let persistent = tmp.keep().map_err(|e| anyhow::anyhow!("{}", e))?;
launch_editor(&persistent.1)?;
```

---

## Info

### IN-01: Comments help modal displays key bindings that are not active in the comments view

**File:** `src/tui/screens/comments.rs:341-365`

**Issue:** The `render_help_modal` in `comments.rs` is a verbatim copy of the modal from `spaces.rs` and `pages.rs`. It lists:

- `<Enter>  Browse pages in space` — Enter does nothing in `CommentsBrowseState`
- `<o>  Open space in browser` — `o` does nothing in comments
- `<Enter/o>  Open page in browser` — neither key does anything in comments
- `<e>  Edit in $EDITOR` — `e` does nothing in comments
- `<c>  View comments` — `c` does nothing in comments
- `</>  Filter (Enter commits · Esc cancels)` — `/` does nothing in comments (no filter in v1)
- `<PgDn/PgUp>  Scroll preview pane` — PageDown/PageUp do nothing in comments

A user who presses `?` in the comments view is shown a help screen that describes actions which are all no-ops. Only the Navigation, Comments (Esc), and General (`?`, `q`) sections are accurate.

**Fix:** Provide a comments-specific help modal listing only the bindings that are active: navigation (j/k/arrows/g/G), `?` to close, `Esc` to go back, `q` to quit.

---

### IN-02: Pages title-bar shortcuts row omits `<q> Quit`

**File:** `src/tui/screens/pages.rs:102-118`

**Issue:** The pages shortcuts row shows: `</>`, `<Enter/o>`, `<e>`, `<c>`, `<g/G>`, `<?>`, `<Esc>`. It omits `<q> Quit`. The spaces title bar (line 80-93 of `spaces.rs`) and comments title bar (line 80-90 of `comments.rs`) both include `<q> Quit`. A user on the pages screen who has not read the comments help modal has no visible reminder that `q` quits from this screen.

**Fix:** Append `kb("q"), desc("Quit")` to the pages shortcuts `Line::from(vec![...])` at line 102-118 of `pages.rs`.

---

### IN-03: Esc-in-filter Esc cancels/reverts the selection to `Some(0)`, losing the previously selected row

**File:** `src/tui/app.rs:354-358` and `src/tui/app.rs:726-729`

**Issue:** When the user presses `/` to open the filter, the current selection (e.g., row 5 in the committed-filter list) is preserved. But pressing `Esc` to cancel calls `apply_filter(&prev)` which always resets `list_state` to `Some(0)` (the selection-reset invariant). This means cancelling a filter refinement drops the user from row 5 back to row 0. The user loses their place in the list even though they did not intend to navigate.

This applies to both `App::handle_key` (spaces) and `PagesBrowseState::handle_key` (pages).

**Fix:** Save the selection before entering Filtering state and restore it on Esc. Store `saved_selection: Option<usize>` in the Filtering variant or as a separate field, then restore with `list_state.select(saved_selection)` on Esc before calling `apply_filter`.

---

### IN-04: `display_name` from Confluence API is interpolated raw into ANSI-coloured `println!`

**File:** `src/cli/init.rs:103`

**Issue:**

```rust
println!("\x1b[1;32mConnected as {}\x1b[0m — configuration saved.", display_name);
```

`display_name` is taken directly from the Confluence API response `body["displayName"]`. A Confluence administrator who has set their display name to contain ANSI escape sequences (e.g., `\x1b[2J` to clear the screen, or `\x1b[?1049h` to switch screen buffers) would cause unexpected terminal behaviour when `ccli init` prints this line. The risk is limited to self-inflicted damage (the user is authenticating as the account whose name is printed), but it is worth sanitising.

**Fix:** Strip or escape ANSI sequences from `display_name` before printing:

```rust
// Simple approach: strip non-printable/non-ASCII control chars
let safe_name: String = display_name.chars()
    .filter(|c| !c.is_control())
    .collect();
if std::io::stdout().is_terminal() {
    println!("\x1b[1;32mConnected as {}\x1b[0m — configuration saved.", safe_name);
} else {
    println!("Connected as {} — configuration saved.", safe_name);
}
```

---

_Reviewed: 2026-05-14_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
