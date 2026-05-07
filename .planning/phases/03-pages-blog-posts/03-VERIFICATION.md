---
phase: 03-pages-blog-posts
verified: 2026-05-07T14:30:00Z
status: human_needed
score: 7/8 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 3/5
  gaps_closed:
    - "CLI page/blog subcommands — src/cli/page.rs and src/cli/blog.rs now exist and are wired via src/main.rs"
    - "TUI event loop wired — DrillDown, PopScreen, EditPage, render_pages, pages_list/preview channels all implemented in src/tui/mod.rs"
    - "OpenBrowser webui path fix (WR-02) — PagesBrowseState::handle_key now carries p.links.webui not page.id"
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "Confirm whether BLOG-01 TUI requirement is satisfied or intentionally deferred"
    expected: "Either (a) a navigation path in the TUI to view blog posts in PagesBrowse, OR (b) an explicit acceptance that BLOG-01 is satisfied by 'ccli blog list' CLI table output per ROADMAP SC-5 wording"
    why_human: "The TUI DrillDown handler in src/tui/mod.rs hardcodes ContentType::Page. Pressing Enter on a space only shows Pages — there is no TUI navigation path to Blog Posts. BLOG-01 requires 'TUI list + preview pane' for blog posts. ROADMAP SC-5 says 'using ccli blog subcommands' which the CLI satisfies. These two contracts conflict and cannot be resolved programmatically."
---

# Phase 3: Pages & Blog Posts Verification Report

**Phase Goal:** List, view, create, and edit Pages and Blog Posts — the core content workflow
**Verified:** 2026-05-07T14:30:00Z
**Status:** HUMAN_NEEDED
**Re-verification:** Yes — after gap closure (Plans 03-05 and 03-06 added)

## Goal Achievement

### Observable Truths

All three previously-failed gaps have been closed. One new UNCERTAIN finding emerged about BLOG-01 TUI access.

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | User can run `ccli page list <KEY>` or open a space in the TUI to see pages in a list + preview pane, filterable by title | VERIFIED | `ccli page list` dispatches to `handle_list_typed`; TUI DrillDown pushes `Screen::PagesBrowse` and spawns `list_all_pages`; `render_pages` called when stack is non-empty; `apply_filter` resets list_state per Pitfall 3 |
| 2 | User can run `ccli page view <PAGE-ID>` to print page content to stdout | VERIFIED | `handle_view_typed` fetches via `get_page_detail`, calls `extract_body_value`, calls `strip_storage_xml`, prints result |
| 3 | User can run `ccli page create` to open `$EDITOR` and publish a new page | VERIFIED | `handle_create_typed` prompts via dialoguer, writes CREATE_TEMPLATE `<p>Start writing here...</p>`, launches `$EDITOR` via `Command::new(editor).arg(path)`, calls `create_page`, prints new id |
| 4 | User can run `ccli page edit <PAGE-ID>` to open storage XML in `$EDITOR` and save back | VERIFIED | `handle_edit_typed` fetches detail, writes to `/tmp/ccli-edit-<id>.xml` (sanitized digits-only), launches `$EDITOR`, calls `update_page` with version+1; 409 → preserves temp file + stderr message |
| 5 | User can list, create, edit, and open Blog Posts using `ccli blog` subcommands | VERIFIED | `ccli blog list/view/create/edit` all dispatch to page handlers with `ContentType::BlogPost`; `--parent` rejected by clap (Pitfall 5 enforced at type level) |
| 6 | User can press 'o' on a page in the TUI to open it in the system browser (PAGE-07, BLOG-04) | VERIFIED | `PagesBrowseState::handle_key` returns `OpenBrowser(p.links.webui)` (WR-02 fix); event loop calls `handle_open_page_browser(webui_path, base_url)` → `resolve_webui_url` + `open::that` |
| 7 | TUI drill-down works end-to-end: Enter on space → pages screen; Esc → back to spaces | VERIFIED | `KeyAction::DrillDown(space_key)` pushes `Screen::PagesBrowse` + spawns `list_all_pages`; `KeyAction::PopScreen` calls `app.screen_stack.pop()`; render dispatch routes to `render_pages` when stack top is `PagesBrowse` |
| 8 | Blog Posts are listable in the TUI with a preview pane (BLOG-01 "TUI list + preview pane") | UNCERTAIN | `render_pages` fully supports `ContentType::BlogPost` display (tested — "Blog Posts" label verified). `ccli blog list` produces a CLI table. BUT the TUI DrillDown handler in `src/tui/mod.rs` hardcodes `ContentType::Page` — no TUI navigation path to blog posts exists. ROADMAP SC-5 says "using ccli blog subcommands" (satisfied); BLOG-01 says "TUI list + preview pane" (not satisfied via TUI). Requires human decision. |

**Score:** 7/8 truths verified (1 UNCERTAIN)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/api/page.rs` | 4 async API functions + structs | VERIFIED | 4 functions: `list_all_pages`, `get_page_detail`, `create_page`, `update_page`; 16 httpmock tests |
| `src/api/mod.rs` | page module re-exports | VERIFIED | `pub mod page;` + full re-exports |
| `Cargo.toml` | `quick-xml = "0.39"` | VERIFIED | Present |
| `src/output/xml.rs` | `strip_storage_xml` function | VERIFIED | Exists, 14 tests pass |
| `src/output/mod.rs` | xml module re-export | VERIFIED | `pub mod xml;` + `pub use xml::strip_storage_xml;` |
| `src/tui/app.rs` | Screen, PagesBrowseState, KeyAction extensions, screen_stack | VERIFIED | All types present; 27 unit tests |
| `src/tui/screens/pages.rs` | `render_pages` function + no dead_code | VERIFIED | 9 helper functions; `#![allow(dead_code)]` removed; 7 render tests |
| `src/tui/screens/mod.rs` | pages module declaration | VERIFIED | `pub mod pages;` present |
| `src/cli/page.rs` | page CLI handlers | VERIFIED | `run`, `handle_list_typed`, `handle_view_typed`, `handle_create_typed`, `handle_edit_typed` |
| `src/cli/blog.rs` | blog CLI handlers | VERIFIED | `run` delegates all commands to page handlers with `ContentType::BlogPost` |
| `src/cli/mod.rs` | Page/Blog Commands variants | VERIFIED | `Commands::Page(PageArgs)` and `Commands::Blog(BlogArgs)` with full subcommand trees |
| `src/main.rs` | Page/Blog dispatch arms | VERIFIED | `Commands::Page(ref args) => cli::page::run(&cli, args).await` and Blog analog |
| `src/tui/mod.rs` | Event loop wiring: DrillDown/PopScreen/EditPage/render_pages | VERIFIED | All three action arms wired; `pages_list_tx/rx` and `pages_preview_tx/rx` channels; render dispatch via `screen_stack.last_mut()` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/api/page.rs` | Confluence /rest/api/content | `client.inner().get/post/put` | VERIFIED | `rest/api/content` pattern present in all 4 functions |
| `src/api/mod.rs` | `src/api/page.rs` | `pub mod page` + re-exports | VERIFIED | All 4 functions + ContentType/Page/PageDetail exported |
| `src/output/xml.rs` | `quick_xml::Reader` | `use quick_xml::Reader` | VERIFIED | Present; uses `read_event()` (slice reader API) |
| `src/output/mod.rs` | `src/output/xml.rs` | `pub mod xml; pub use xml::strip_storage_xml` | VERIFIED | Both lines present |
| `src/tui/app.rs` | `src/api/page.rs` | `use crate::api::page::{ContentType, Page, PageDetail}` | VERIFIED | Import at line 21 |
| `src/tui/screens/pages.rs` | `src/output/xml.rs` | `use crate::output::strip_storage_xml` | VERIFIED | Import present; called in `build_preview_text` |
| `src/tui/screens/pages.rs` | `src/tui/app.rs::PagesBrowseState` | `use crate::tui::app::PagesBrowseState` | VERIFIED | Import present |
| `src/tui/mod.rs terminal.draw` | `render_pages` | `match screen_stack.last_mut() { PagesBrowse => render_pages(..) }` | VERIFIED | Render dispatch present at line 116-124 |
| `src/tui/mod.rs DrillDown arm` | `list_all_pages` | `tokio::spawn(async { list_all_pages(..).await })` | VERIFIED | Async fetch spawned with `pages_list_tx` |
| `src/tui/mod.rs preview dispatch` | `get_page_detail` | `tokio::spawn(async { get_page_detail(..).await })` | VERIFIED | Fetch dispatched via `pages_preview_tx` |
| `src/cli/page.rs` | `src/api/page.rs` | `use crate::api::page::{create_page, get_page_detail, list_all_pages, update_page, ContentType, PageDetail}` | VERIFIED | All 4 API functions imported and called |
| `src/cli/page.rs::handle_view_typed` | `src/output/xml.rs::strip_storage_xml` | `use crate::output::strip_storage_xml` | VERIFIED | Called at line 111 after `extract_body_value` |
| `src/cli/page.rs::handle_edit_typed` | `$EDITOR` | `Command::new(&editor).arg(path).status()` | VERIFIED | `launch_editor` helper uses direct exec, no shell |
| `src/main.rs` | `src/cli/page.rs` / `src/cli/blog.rs` | `Commands::Page/Blog` dispatch | VERIFIED | Both dispatch arms at lines 42-43 |
| `src/tui/app.rs::PagesBrowseState` | `page.links.webui` | `selected_page().and_then(\|p\| p.links.webui.clone())` | VERIFIED | WR-02 fix at line 619; webui path not page id |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| `render_pages` | `state.pages` | `list_all_pages()` via `pages_list_rx.try_recv()` | Yes — spawned on DrillDown, result drained each loop tick | FLOWING |
| `build_preview_text` | `state.preview_cache` | `get_page_detail()` via `pages_preview_rx.try_recv()` | Yes — spawned when `pending_preview_id` set by `state.tick()` debounce | FLOWING |
| `handle_view_typed` | `stripped` | `get_page_detail` → `extract_body_value` → `strip_storage_xml` | Yes — fetches real API data and prints | FLOWING |
| `handle_list_typed` | `rows` | `list_all_pages()` results | Yes — fetches all pages, formats as table | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `ccli page` subcommand exists with list/view/create/edit | `./target/debug/ccli page --help` | Lists all 4 subcommands | PASS |
| `ccli blog` subcommand exists with list/view/create/edit | `./target/debug/ccli blog --help` | Lists all 4 subcommands | PASS |
| `ccli page list` requires positional space_key (D-34) | `./target/debug/ccli page list` (no args) | "required arguments were not provided: SPACE_KEY" | PASS |
| `ccli blog create --parent` fails (Pitfall 5) | `./target/debug/ccli blog create --space DEV --title Hi --parent 99` | "unexpected argument '--parent' found" | PASS |
| All 189 tests pass | `cargo test` | 189 passed, 0 failed | PASS |
| Build succeeds | `cargo build` | exits 0, no errors or warnings | PASS |

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| PAGE-01 | 03-01, 03-03, 03-04, 03-06 | List pages in TUI list + preview pane | SATISFIED | DrillDown wired; render_pages called; list_all_pages fetched; preview pane populates after debounce |
| PAGE-02 | Phase 4 | CQL search | DEFERRED | Phase 4 in REQUIREMENTS.md traceability; not in Phase 3 scope |
| PAGE-03 | 03-03, 03-04, 03-06 | Fuzzy-filter pages by title in TUI | SATISFIED | `apply_filter` uses nucleo on titles; Pitfall 3 reset verified; 150ms debounce |
| PAGE-04 | 03-01, 03-02, 03-04, 03-06 | View full page content in TUI preview | SATISFIED | `strip_storage_xml` called in `build_preview_text`; preview cache populated via `get_page_detail` |
| PAGE-05 | 03-05 | `ccli page create` opens `$EDITOR` | SATISFIED | `handle_create_typed` prompts, opens editor with CREATE_TEMPLATE, POSTs, prints id |
| PAGE-06 | 03-05 | `ccli page edit` opens storage XML | SATISFIED | `handle_edit_typed` fetches XML, opens editor, PUTs version+1 |
| PAGE-07 | 03-06 | Press 'o' on page in TUI → browser | SATISFIED | `PagesBrowseState::handle_key` returns `OpenBrowser(webui_path)` (WR-02); `handle_open_page_browser` resolves and opens |
| PAGE-08 | 03-01, 03-02, 03-05 | `ccli page view` prints to stdout | SATISFIED | `handle_view_typed` → `get_page_detail` → `strip_storage_xml` → `println!` |
| BLOG-01 | 03-01, 03-03, 03-04, 03-06 | List blog posts in TUI list + preview pane | UNCERTAIN | `render_pages` supports BlogPost; `ccli blog list` produces CLI table. TUI DrillDown hardcodes `ContentType::Page` — no TUI path to blog posts. ROADMAP SC-5 says "ccli blog subcommands" (satisfied); BLOG-01 says "TUI list" (not satisfied). Human decision required. |
| BLOG-02 | 03-05 | `ccli blog create` opens `$EDITOR` | SATISFIED | `blog.rs::run` delegates to `handle_create_typed(..., ContentType::BlogPost, None)` |
| BLOG-03 | 03-05 | `ccli blog edit` opens storage XML | SATISFIED | `blog.rs::run` delegates to `handle_edit_typed(post_id, ContentType::BlogPost)` |
| BLOG-04 | 03-06 | Press 'o' on blog post in TUI → browser | SATISFIED | Same `handle_open_page_browser` path; `PagesBrowseState` handles both content types |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/tui/mod.rs` | 225 | `ContentType::Page` hardcoded in DrillDown handler | WARNING | Blog Posts unreachable via TUI drill-down (BLOG-01 question) |
| `src/cli/page.rs` | 88 | `// D-35 TUI dispatch is wired in plan 03-06; Plan 05 always prints a table.` | INFO | Intentional deferral comment; CLI list always prints table even on TTY |

No blockers (no TODO stubs, no no-op handlers, no missing file anti-patterns) in the gap-closure plans. The previously-flagged blockers are all resolved.

### Human Verification Required

### 1. BLOG-01 TUI Blog Posts Access

**Test:** Navigate to any plan that addresses blog post TUI browsing. Check if BLOG-01 ("TUI list + preview pane") is satisfied by the CLI alone, or if TUI blog access is required.

**Expected:** One of:
- (A) Decision that ROADMAP SC-5 wording ("using `ccli blog` subcommands") supersedes the BLOG-01 requirement text ("TUI list + preview pane"), and `ccli blog list` satisfies BLOG-01. Mark as ACCEPTED.
- (B) Gap identified: `src/tui/mod.rs` DrillDown handler should also push `ContentType::BlogPost` when the user navigates to a blog-posts view (would require a UI mechanism to select content type — e.g., pressing `b` vs Enter, or a separate blog list page).

**Why human:** The ROADMAP Success Criteria (SC-5) are worded around "ccli blog subcommands" which are satisfied by the CLI. BLOG-01 in REQUIREMENTS.md says "TUI list + preview pane." The DrillDown handler hardcodes `ContentType::Page` — there is no code path that ever creates a `PagesBrowse` screen with `ContentType::BlogPost`. This is a genuine ambiguity between two written contracts (REQUIREMENTS.md vs ROADMAP.md SC). Code analysis cannot determine which is authoritative.

### Gaps Summary

All three previously-identified gaps have been closed:

1. **CLI page/blog subcommands** — `src/cli/page.rs` and `src/cli/blog.rs` fully implemented; `Commands::Page` and `Commands::Blog` wired in `src/main.rs`. All CLI behaviors (list, view, create, edit) work end-to-end. 409 conflict preserves temp file. Path traversal prevented by `sanitize_id`. 

2. **TUI event loop wiring** — `src/tui/mod.rs` now dispatches `DrillDown` to push `Screen::PagesBrowse` and spawn `list_all_pages`; `PopScreen` pops the stack; `EditPage` suspends TUI, runs editor, restores TUI; `render_pages` called when stack top is `PagesBrowse`; preview debounce working via `pages_preview_tx/rx` channels.

3. **OpenBrowser webui path (WR-02)** — `PagesBrowseState::handle_key` now returns `OpenBrowser(p.links.webui)` not `OpenBrowser(id)`; missing webui returns `KeyAction::None` gracefully; tests updated and passing.

**One outstanding item requires human decision:** Whether BLOG-01 ("TUI list + preview pane" for blog posts) is satisfied by the CLI table output alone, or whether a TUI path to blog posts must be added. All Phase 3 ROADMAP Success Criteria (SC 1-5) are satisfied by the current implementation; the ambiguity is between REQUIREMENTS.md BLOG-01 text and the ROADMAP SC wording.

**Test suite:** 189 tests pass, 0 fail. `cargo build` exits 0.

---

_Verified: 2026-05-07T14:30:00Z_
_Verifier: Claude (gsd-verifier)_
