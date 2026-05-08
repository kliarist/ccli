---
phase: 04-comments-attachments-search
verified: 2026-05-08T00:00:00Z
status: human_needed
score: 16/16
overrides_applied: 0
human_verification:
  - test: "Run TUI end-to-end: press 'c' on a selected page, observe CommentsBrowse screen, verify spinner transitions to list, verify Esc pops back to PagesBrowse"
    expected: "CommentsBrowse screen renders with spinner, then shows author · date · first-line list rows and preview pane; Esc returns to PagesBrowse with c-comments in help bar"
    why_human: "TUI real-time behavior, async spinner animation, and full Confluence API integration cannot be verified without a running instance and terminal"
  - test: "Run `ccli comment add <PAGE-ID>`: verify $EDITOR opens empty, on save comment appears in Confluence with correct paragraph wrapping; run again with empty save to verify rejection message"
    expected: "stdout: 'Comment added to page {ID}.'; empty save: stderr 'comment add: editor returned empty content — comment not posted'"
    why_human: "Requires $EDITOR interaction and a real Confluence instance; cannot verify editor launch or Confluence write without live integration"
  - test: "Run `ccli attachment list <PAGE-ID>`, `ccli attachment get <PAGE-ID> <FILENAME>`, `ccli attachment add <PAGE-ID> ./file.txt` against a real page with attachments"
    expected: "List: 4-column table (Filename, Size, Media-Type, Date) with human-readable sizes; Get: file written to ./FILENAME with 'Downloaded: ...' stdout; Add: file uploaded with 'Uploaded: ...' stdout"
    why_human: "Requires a real Confluence instance with attachments; file I/O and multipart upload cannot be exercised without live integration"
  - test: "Run `ccli page search \"type=page\" --limit 3` and verify 4-column table (Title, ID, Space, URL) with full https URL in URL column"
    expected: "Table with up to 3 rows; URL column contains https://... full URL; '--limit' overrides default 25"
    why_human: "Requires a real Confluence instance to return actual search results; URL resolution from relative _links.webui cannot be verified without live data"
---

# Phase 4: Comments, Attachments & Search — Verification Report

**Phase Goal:** Implement comments, attachments, and CQL search — users can add/list comments, manage attachments, search pages via CQL, and browse comments interactively in the TUI.
**Verified:** 2026-05-08T00:00:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Roadmap Success Criteria (Must-Haves)

The ROADMAP.md defines 3 success criteria for Phase 4:

1. User can run `ccli attachment list <PAGE-ID>` to list attachments, download one with `ccli attachment get`, and upload a file with `ccli attachment add`
2. User can view all comments on a page in the TUI and add a new comment via `ccli comment add <PAGE-ID>` using `$EDITOR`
3. User can run `ccli page search "<CQL>"` and receive a formatted list of matching pages

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `list_comments(client, page_id)` returns `Vec<Comment>` with `body.storage` and `version` expanded | VERIFIED | `src/api/comment.rs`: `list_comments` uses `.query(&[("expand", "body.storage,version"), ("limit", "50")])` (line 84); `CommentListResponse` deserializes into `Vec<Comment>`; test `list_comments_returns_vec_on_200` passes |
| 2 | `add_comment(client, page_id, xml_body)` POSTs storage XML and returns `Result<()>` | VERIFIED | `src/api/comment.rs`: POSTs JSON with `"type":"comment"` and `body.storage.value = xml_body` (lines 111–131); test `add_comment_posts_storage_xml_body` passes |
| 3 | `list_attachments(client, page_id)` returns `Vec<Attachment>` with extensions and version | VERIFIED | `src/api/attachment.rs`: `list_attachments` with `?limit=50`, deserializes `AttachmentListResponse`; test `list_attachments_returns_vec_on_200` passes |
| 4 | `get_attachment(client, page_id, filename)` returns `Bytes` for the named attachment | VERIFIED | `src/api/attachment.rs`: lists attachments, finds by title match, resolves `_links.download`, returns `resp.bytes()`; test `get_attachment_downloads_bytes` passes |
| 5 | `add_attachment(client, page_id, file_path, mime_type)` uploads via multipart/form-data with `X-Atlassian-Token: no-check` | VERIFIED | `src/api/attachment.rs` line 89: `.header("X-Atlassian-Token", "no-check")` + `reqwest::multipart::Form::new()`; `mime_guess::from_path` wired; test `add_attachment_sends_multipart_with_x_atlassian_token_header` passes |
| 6 | All async API fns carry `#[instrument(skip(client))]` — PAT never logged | VERIFIED | `grep -c '#[instrument(skip(client))]' src/api/comment.rs` = 3 (incl. test infra); `grep` on attachment.rs = 4; both files confirm #[instrument(skip(client))] on every public async fn |
| 7 | All API errors map to existing `AppError` variants (Auth/Network/Api) — no new variants | VERIFIED | `src/api/comment.rs` and `src/api/attachment.rs` use only `AppError::Auth`, `AppError::Network`, `AppError::Api`; no new enum variants added |
| 8 | User runs `ccli page search "<CQL>"` and receives a formatted table of matching pages | VERIFIED | `src/cli/page.rs`: `handle_search` hits `/rest/api/content/search`, outputs `["Title", "ID", "Space", "URL"]` via `OutputFormatter::print`; 5 tests pass including `page_search_outputs_table_with_4_columns` |
| 9 | Default result limit is 25; `--limit N` overrides (D-46) | VERIFIED | `src/cli/mod.rs`: `Search { cql: String, #[arg(long, default_value_t = 25)] limit: u32 }` confirmed; test `page_search_default_limit_is_25` passes |
| 10 | Search output columns are Title, ID, Space, URL (D-46) | VERIFIED | `src/cli/page.rs` line where `headers = &["Title", "ID", "Space", "URL"]` confirmed; space resolved from `SearchResultSpace.key`; URL resolved from base_url + `_links.webui` |
| 11 | `Commands::Comment(CommentArgs)` and `Commands::Attachment(AttachmentArgs)` variants exist; main.rs dispatches them | VERIFIED | `src/cli/mod.rs`: both variants confirmed; `src/main.rs`: `Some(Commands::Comment(ref args)) => cli::comment::run(...)` and `Some(Commands::Attachment(ref args)) => cli::attachment::run(...)` both present |
| 12 | `Screen::CommentsBrowse { page_id, state }` pushed and `CommentsBrowseState::handle_key` returns `KeyAction` per v1 keymap | VERIFIED | `src/tui/app.rs`: `Screen::CommentsBrowse` variant confirmed; `handle_key` implemented with Browse/Loading state machine; all 11 new tests in `tui::app::tests` pass |
| 13 | `PagesBrowseState::handle_key` emits `KeyAction::DrillDownComments(page_id)` when 'c' pressed in Browse state with selection | VERIFIED | `src/tui/app.rs`: `KeyCode::Char('c')` branch in Browse arm emits `DrillDownComments(selected_id)` or `KeyAction::None` when no selection; test `pages_browse_state_handle_key_c_returns_drill_down_comments_when_selection_present` passes |
| 14 | User runs `ccli comment add <PAGE-ID>` and `$EDITOR` opens with plain-text input; on save, text is wrapped and POSTed | VERIFIED | `src/cli/comment.rs`: `handle_add` validates id, writes empty temp file (D-53), calls `launch_editor`, calls `wrap_plain_text_to_storage_xml`, calls `add_comment`; 7 unit tests all pass; wiring confirmed: `wrap_plain_text_to_storage_xml(plain) → add_comment(client, &id, &xml)` |
| 15 | `ccli attachment list/get/add` handlers fully implemented with correct output strings | VERIFIED | `src/cli/attachment.rs`: `handle_list` (4-column table), `handle_get` (bytes → disk, "Downloaded: ..."), `handle_add` (upload, "Uploaded: ..."); `human_size` and `build_list_rows` present; 6 unit tests pass; no "not yet implemented" strings remain |
| 16 | `render_comments` renders the CommentsBrowse TUI screen and is wired into the event loop | VERIFIED | `src/tui/screens/comments.rs`: `render_comments` implemented with 4-pane layout, middle-dot separator (U+00B7), strip_storage_xml, Loading/Browse/Error status bar; `src/tui/mod.rs`: `render_comments(f, state.as_mut(), f.area())` in dispatch arm; 7 render tests pass |

**Score:** 16/16 truths verified (automated)

### Deferred Items

None — all success criteria are implemented in Phase 4.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/api/comment.rs` | Comment struct, list_comments, add_comment | VERIFIED | 236 lines; all public structs and functions present; 4 unit tests pass |
| `src/api/attachment.rs` | Attachment struct, list_attachments, get_attachment, add_attachment | VERIFIED | All 3 async fns present; 5 unit tests pass |
| `Cargo.toml` | mime_guess dependency | VERIFIED | `mime_guess = "2"` present; `bytes = "1"` added as required; reqwest multipart feature enabled |
| `src/api/mod.rs` | `pub mod comment` + `pub mod attachment` | VERIFIED | Both pub mod declarations present |
| `src/cli/mod.rs` | Commands::Comment, Commands::Attachment, PageCommands::Search | VERIFIED | All three additions confirmed; CommentArgs, CommentCommands, AttachmentArgs, AttachmentCommands structs present |
| `src/cli/page.rs` | `handle_search` with CQL + always-table output | VERIFIED | `handle_search` present; hits `/rest/api/content/search` with cql/limit/expand params; 5 tests pass |
| `src/main.rs` | Comment and Attachment dispatch arms | VERIFIED | Both arms present; dispatch to `cli::comment::run` and `cli::attachment::run` |
| `src/tui/app.rs` | CommentsBrowseState, Screen::CommentsBrowse, KeyAction::DrillDownComments | VERIFIED | All three additions confirmed; 11 new tests pass; 48 total tui::app tests pass |
| `src/cli/comment.rs` | Full handler: handle_add, wrap_plain_text_to_storage_xml, sanitize_id, launch_editor | VERIFIED | All 4 functions present; zero "not yet implemented" stubs; 7 tests pass |
| `src/cli/attachment.rs` | handle_list, handle_get, handle_add, human_size, build_list_rows | VERIFIED | All 5 functions present; zero "not yet implemented" stubs; 6 tests pass |
| `src/tui/screens/comments.rs` | render_comments — pure render fn | VERIFIED | render_comments + render_list_pane + render_preview_pane present; middle-dot separator; 7 tests pass |
| `src/tui/screens/mod.rs` | `pub mod comments;` | VERIFIED | Present |
| `src/tui/mod.rs` | render_comments dispatch, DrillDownComments handler, comments_list channel, drain loop | VERIFIED | All 4 wiring elements confirmed; imports, channel, drain loop, render dispatch, key routing all present |
| `src/tui/screens/pages.rs` | `c  comments` in help bar; `c View comments` in help modal | VERIFIED | `key("c")` + `"  comments   "` + `row("c", "View comments")` all present; tests pass |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/api/comment.rs` | `src/api/client.rs Client::inner()` | `client.inner().get/post()` | VERIFIED | `client.inner()` used at lines 82 and 133 in comment.rs |
| `src/api/attachment.rs` | `reqwest::multipart` | multipart upload + X-Atlassian-Token | VERIFIED | `multipart::Form::new().part("file", part)` + `.header("X-Atlassian-Token", "no-check")` confirmed |
| `src/api/comment.rs` | `src/api/error.rs AppError` | 401/403 → Auth, connect/timeout → Network, other → Api | VERIFIED | All 3 error paths confirmed in comment.rs; no new AppError variants |
| `src/cli/page.rs handle_search` | Confluence `/rest/api/content/search` | `.query(&[("cql", cql), ("limit", N), ("expand", "space")])` | VERIFIED | Lines 347–352 in page.rs; all 3 params present |
| `src/cli/page.rs handle_search` | `OutputFormatter::print` | Always-table output | VERIFIED | `OutputFormatter::new(cli.output_config()).print(headers, &rows)` confirmed |
| `src/main.rs` | `src/cli/comment.rs` and `src/cli/attachment.rs` | `cli::comment::run / cli::attachment::run` dispatch | VERIFIED | Both arms present in main.rs |
| `src/tui/app.rs PagesBrowseState::handle_key` | `KeyAction::DrillDownComments` | 'c' key in Browse state with selection | VERIFIED | `KeyCode::Char('c')` branch emits `DrillDownComments(id)` or `None`; test 10 + 11 verify both paths |
| `src/tui/mod.rs DrillDownComments arm` | `src/api/comment.rs list_comments` | `tokio::spawn(list_comments(client, page_id))` | VERIFIED | `list_comments(&fetch_client, &id_tag)` in spawned task at line 312 in tui/mod.rs |
| `src/tui/mod.rs comments_list_rx drain` | `CommentsBrowseState::set_comments / set_fetch_error` | match on screen_stack for matching page_id | VERIFIED | Lines 185–194 in tui/mod.rs confirm drain loop calls `state.set_comments(comments)` or `state.set_fetch_error(e)` |
| `src/cli/comment.rs` | `src/api/comment.rs add_comment` | `wrap_plain_text_to_storage_xml(plain) → add_comment(client, &id, &xml)` | VERIFIED | Lines 47–55 in cli/comment.rs; both wired correctly |
| `src/cli/attachment.rs handle_get` | `src/api/attachment.rs get_attachment` | `get_attachment` returns Bytes; `std::fs::write(dest, &bytes)` | VERIFIED | Lines 90–99 in cli/attachment.rs |
| `src/cli/attachment.rs handle_add` | `src/api/attachment.rs add_attachment` | `add_attachment(client, page_id, Path::new(&file_path))` | VERIFIED | `add_attachment(&client, &id, path)` present in handle_add |
| `src/tui/screens/comments.rs render_list_pane` | `Comment` fields via `strip_storage_xml` | `display_name`, `version.when`, `body.storage.value` | VERIFIED | `format_list_row` accesses all 3 field paths; `strip_storage_xml(body)` called for first_line |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `src/tui/screens/comments.rs` | `state.comments` (Vec<Comment>) | `list_comments` API call → `comments_list_rx` channel → `set_comments()` | Yes — real HTTP GET with httpmock-tested response deserializing to Vec<Comment> | FLOWING |
| `src/cli/page.rs handle_search` | `parsed.results` (Vec<SearchResult>) | `client.inner().get(url).query(...).send()` → `resp.json()` | Yes — direct HTTP call; `SearchResponse` deserializes from Confluence JSON | FLOWING |
| `src/cli/attachment.rs handle_list` | `attachments` (Vec<Attachment>) | `list_attachments(&client, &id)` → API GET | Yes — wired to real API call; `build_list_rows` converts to table rows | FLOWING |
| `src/cli/attachment.rs handle_get` | `bytes` (Bytes) | `get_attachment(&client, &id, filename)` → download URL | Yes — real bytes returned; `std::fs::write(dest, &bytes)` writes to disk | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| All 235 tests pass | `cargo test` | 235 passed; 0 failed | PASS |
| Build is clean | `cargo build` | exit 0, no errors | PASS |
| Clippy clean | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | PASS |
| 9 API-layer tests | `cargo test -- api::comment api::attachment` | 9 passed | PASS |
| 7 comment CLI tests | `cargo test -- cli::comment` | 7 passed | PASS |
| 6 attachment CLI tests | `cargo test -- cli::attachment` | 6 passed | PASS |
| 48 TUI app tests (incl. 11 new) | `cargo test -- tui::app` | 48 passed | PASS |
| 7 render_comments tests | `cargo test -- tui::screens::comments` | 7 passed | PASS |
| TUI end-to-end against real Confluence | Manual run required | SKIP | ? SKIP — needs live instance |
| ccli comment add workflow | Manual run with $EDITOR | SKIP | ? SKIP — needs interactive terminal |
| ccli page search against live Confluence | Manual run required | SKIP | ? SKIP — needs live instance |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CMNT-01 | 04-01, 04-03, 04-05 | User can list all comments on a page in the TUI | VERIFIED | `CommentsBrowseState` + `render_comments` + event-loop wiring complete; 'c' key pushes screen, async fetch populates state, list/preview render confirmed by 7 tests |
| CMNT-02 | 04-01, 04-04 | User can add a page-level comment via `ccli comment add <PAGE-ID>` using `$EDITOR` | VERIFIED | `handle_add` + `wrap_plain_text_to_storage_xml` + `add_comment` wired; 7 unit tests pass; live path needs human verification |
| ATTCH-01 | 04-01, 04-04 | User can list all attachments on a page via `ccli attachment list <PAGE-ID>` | VERIFIED | `handle_list` → `list_attachments` → 4-column table; `build_list_rows` tested |
| ATTCH-02 | 04-01, 04-04 | User can download an attachment via `ccli attachment get <PAGE-ID> <FILENAME>` | VERIFIED | `handle_get` → `get_attachment` → `std::fs::write`; `--output` override wired; test `get_attachment_downloads_bytes` passes |
| ATTCH-03 | 04-01, 04-04 | User can upload a local file as an attachment via `ccli attachment add <PAGE-ID> <FILE-PATH>` | VERIFIED | `handle_add` → `add_attachment` with `X-Atlassian-Token: no-check` and `mime_guess`; test `add_attachment_sends_multipart_with_x_atlassian_token_header` passes |
| PAGE-02 | 04-02 | User can search pages using a CQL expression | VERIFIED | `handle_search` → `/rest/api/content/search` → 4-column table (Title, ID, Space, URL); 5 tests pass |

All 6 requirements declared in PLAN frontmatter confirmed in codebase.

**Orphaned requirements check:** REQUIREMENTS.md maps CMNT-01, CMNT-02, ATTCH-01, ATTCH-02, ATTCH-03, PAGE-02 to Phase 4. All 6 appear in at least one plan's `requirements` field. No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/tui/screens/comments.rs` (help bar) | ~440 | Help bar shows `o  open` but 'o' is intentionally a no-op in v1 | Info | Documented in threat model T-04-31 as accepted; `CommentsBrowseState::handle_key` returns `KeyAction::None` for 'o'; no functional impact |

No blockers. No stubs remain in any key file. No "not yet implemented" strings in comment or attachment CLIs.

### Human Verification Required

The automated checks confirm all code is implemented, wired, and tested at the unit level. The following items require human testing against a live Confluence instance or interactive terminal:

#### 1. TUI Comment Browsing End-to-End

**Test:** Launch `ccli` TUI, navigate to a space and select a page with known comments, press `c`.
**Expected:** CommentsBrowse screen appears, spinner shows "Loading comments…" briefly, then comment list with `author · date · first-line` rows renders. Preview pane shows Author/Date metadata and full stripped body. Press `j`/`k` to navigate; preview updates. Press `Esc` to return to PagesBrowse — help bar shows `c  comments`. Press `c` again on a different page — refetches.
**Why human:** Real-time async fetch, spinner animation, screen-stack navigation, and full Confluence integration cannot be verified without a running Confluence instance and interactive terminal.

#### 2. ccli comment add Workflow

**Test:** Run `ccli comment add <PAGE-ID>` on a page that exists. Verify `$EDITOR` opens with empty buffer. Type two paragraphs separated by a blank line, save, exit. Then run again and save without typing.
**Expected:** First run: stdout `Comment added to page {PAGE-ID}.`; comment appears in Confluence with two `<p>` blocks. Second run: stderr `comment add: editor returned empty content — comment not posted`; no comment posted.
**Why human:** Requires `$EDITOR` interaction and a writable Confluence page; the editor launch, file lifecycle, and API POST cannot be exercised without live integration.

#### 3. ccli attachment list/get/add Workflow

**Test:** Run `ccli attachment list <PAGE-ID>` on a page with attachments. Note a filename, run `ccli attachment get <PAGE-ID> <FILENAME>`. Create a small file, run `ccli attachment add <PAGE-ID> ./file.txt`.
**Expected:** List: 4-column table (Filename, Size, Media-Type, Date); sizes human-readable. Get: file written to `./FILENAME`, stdout `Downloaded: {FILENAME} → ./{FILENAME}`. Add: stdout `Uploaded: file.txt ({size}) → page {PAGE-ID}`; attachment visible in Confluence UI.
**Why human:** Requires a real Confluence instance with actual attachments; multipart upload and binary download cannot be fully verified without live data.

#### 4. ccli page search

**Test:** Run `ccli page search "type=page" --limit 3` against a real Confluence instance.
**Expected:** 4-column table (Title, ID, Space, URL); URL column contains full `https://...` URL; up to 3 rows. Run without `--limit` to confirm default of 25 rows (or fewer if fewer pages exist).
**Why human:** Requires a real Confluence instance to return actual search results; URL resolution from relative `_links.webui` cannot be confirmed without live data.

### Gaps Summary

No gaps found. All 16 observable truths verified in the codebase. All 6 requirements implemented and connected. Build and clippy are clean. 235 tests pass.

The `human_needed` status reflects 4 live-integration tests that cannot be automated (TUI rendering, $EDITOR interaction, real API calls to a Confluence instance).

---

_Verified: 2026-05-08T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
