# Phase 4: Comments, Attachments & Search - Context

**Gathered:** 2026-05-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Complete the v1 surface: CQL-powered search across Confluence pages, page-level comment browsing and creation, and attachment management (list, download, upload).

**CLI surface:**
- `ccli page search "<CQL>"` — table output of matching pages (always, regardless of TTY)
- `ccli comment add <PAGE-ID>` — opens `$EDITOR` for plain-text comment creation
- `ccli attachment list <PAGE-ID>` — table of attachments with filename, size, type, date
- `ccli attachment get <PAGE-ID> <FILENAME>` — download attachment to CWD
- `ccli attachment add <PAGE-ID> <FILE-PATH>` — upload file as attachment

**TUI surface:**
- `c` key on page list pushes a `CommentsBrowse { page_id }` screen (same screen-stack pattern as SpacesBrowse → PagesBrowse)
- CommentsBrowse: list pane shows author + date + first line; preview pane shows full stripped comment body

Requirements covered: PAGE-02, CMNT-01, CMNT-02, ATTCH-01, ATTCH-02, ATTCH-03.

Out of phase: Page version history, inline (body) comments vs page-level (footer) comments distinction, attachment version management, batch attachment download, saved CQL aliases (v2 requirement SRCH-01).

</domain>

<decisions>
## Implementation Decisions

### CQL Search

- **D-45:** **`ccli page search "<CQL>"` always outputs a table — no TUI, regardless of TTY.** Search is a one-shot query result, not a live browseable list. Always stdout table. Composable with pipes and grep. Does NOT follow D-35 (TUI when TTY) — search is an explicit exception.
- **D-46:** **Default limit: 25 results. Columns: title, ID, space, URL.** The URL column makes results immediately actionable (copy-paste to browser). `--limit N` flag available for users who want more or fewer results. No pagination walk — respect the limit.
- **D-47:** **CQL expression is a positional argument.** `ccli page search "type=page AND space=DEV AND text~deploy"` — concise, shell-friendly, matches the requirement spec (PAGE-02). No `--query` flag needed.

### Comments TUI

- **D-48:** **Full `CommentsBrowse { page_id }` screen pushed onto the screen stack.** Accessed via `c` key on a selected page in the page list (PagesBrowse screen). Same push/pop pattern as D-30 (Enter on space → PagesBrowse). Esc pops back to PagesBrowse.
- **D-49:** **`c` keybinding on page list opens CommentsBrowse.** Mnemonic, available in the existing key-binding space (q=quit, Esc=back, e=edit, o=open, /=filter). Shown in the key-binding help bar at the bottom of the page list.
- **D-50:** **List pane shows: author · date · first line of comment (truncated to fit).** Example: `Alice · 2026-05-01 · This needs updating before...`. Preview pane shows full stripped comment body (same `strip_storage_xml()` logic from Phase 3). D-19 debounce + session cache applies to comment body fetches.

### Comment Creation Format

- **D-51:** **Plain text input in `$EDITOR` for `ccli comment add <PAGE-ID>`.** Intentional divergence from D-40 (raw storage XML for page create/edit). Comments are prose; raw XML creates unnecessary friction. The CLI wraps the content before posting.
- **D-52:** **Multi-paragraph wrapping: blank lines split into separate `<p>` blocks.** Each paragraph separated by a blank line becomes its own `<p>...</p>` element in storage XML. Single-line input → single `<p>`. The CLI handles wrapping automatically before `POST /rest/api/content/{id}/child/comment`.
- **D-53:** **`$EDITOR` template for comments is empty** (or a minimal comment: no placeholder text). Simpler than pages — user starts typing directly.

### Attachments

- **D-54:** **`ccli attachment get <PAGE-ID> <FILENAME>` — `<FILENAME>` is the attachment name; file saves to `./FILENAME` (CWD).** `--output <path>` flag available to override the destination path. Predictable default: `ccli attachment get 12345 design.png` → `./design.png`.
- **D-55:** **`ccli attachment add <PAGE-ID> <FILE-PATH>` auto-detects MIME type from file extension.** Use `mime_guess` crate (or equivalent) keyed on file extension. Unknown extensions fall back to `application/octet-stream`. No `--content-type` flag needed for v1.
- **D-56:** **`ccli attachment list <PAGE-ID>` columns: filename, size (human-readable, e.g. 142 KB), media-type, date.** Follows OUT-01 aligned table format. Same `--plain` / `--no-headers` / `--columns` flags apply (OutputFormatter reuse).

### Claude's Discretion

- **Comment REST endpoint:** `POST /rest/api/content/{page_id}/child/comment` with body `{ "type": "comment", "body": { "storage": { "value": "<p>...</p>", "representation": "storage" } } }`. Use existing `Client.inner()` pattern, same auth header.
- **Attachment REST endpoints:** `GET /rest/api/content/{id}/child/attachment` for list; `GET /rest/api/content/{id}/child/attachment?filename={name}` + download URL for get; `POST /rest/api/content/{id}/child/attachment` multipart for add.
- **Multipart upload:** `reqwest` multipart (`client.inner().post(url).multipart(form)`) — `reqwest` already in Cargo.toml. Form part: file bytes + filename + content-type (from D-55).
- **CommentsBrowse state:** Follows the same `AppState` enum extension pattern as PagesBrowse (D-31). Add `CommentsBrowse { page_id: String }` to the `Screen` enum in `src/tui/app.rs`.
- **Comment list API:** `GET /rest/api/content/{id}/child/comment?expand=body.storage,version&limit=50` — fetch all comments with body. Comments don't paginate as aggressively as pages; 50 is sufficient for most pages.
- **Decision ID continuation:** Phase 3 ended at D-44. Phase 4 starts at D-45.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project & Phase Inputs
- `.planning/REQUIREMENTS.md` — PAGE-02, CMNT-01, CMNT-02, ATTCH-01, ATTCH-02, ATTCH-03 (Phase 4 requirements with acceptance criteria)
- `.planning/PROJECT.md` — Key decisions (PAT-only auth, jira-cli UX model, raw XML editing scope, out-of-scope items)
- `.planning/ROADMAP.md` §"Phase 4: Comments, Attachments & Search" — goal, success criteria, requirement mapping
- `.planning/phases/03-pages-blog-posts/03-CONTEXT.md` — Phase 3 locked decisions (D-30..D-44); screen stack pattern (D-31), `c`/`e`/`o` keybindings (D-32, D-41), debounce+cache (D-19/D-43), ContentType enum (D-37), plain-text stripping (D-42), CLI flags (D-34, D-35)
- `.planning/phases/01-foundation/01-CONTEXT.md` — D-06 (TTY auto-detect) — note: D-45 is an explicit exception for `page search`

### Target API (Confluence DC 9.2.19)
- Confluence DC REST API v1:
  - `GET /rest/api/content/search?cql={CQL}&limit=25&expand=space` — CQL search endpoint (D-47)
  - `GET /rest/api/content/{id}/child/comment?expand=body.storage,version&limit=50` — page comments list (CMNT-01, D-50)
  - `POST /rest/api/content/{id}/child/comment` — add comment (CMNT-02, D-51, D-52)
  - `GET /rest/api/content/{id}/child/attachment?limit=50` — list attachments (ATTCH-01, D-56)
  - `GET /rest/api/content/{id}/child/attachment?filename={name}` — find attachment by name for download (ATTCH-02, D-54)
  - `POST /rest/api/content/{id}/child/attachment` multipart — upload attachment (ATTCH-03, D-55)
  - Download URL from attachment `_links.download` — direct binary GET

### Phase 3 Code (already implemented — reuse, don't re-build)
- `src/api/page.rs` — `list_all_pages` pagination walk pattern; `ContentType` enum; `AppError` taxonomy (Auth/Network/Api) — replicate for `src/api/comment.rs` and `src/api/attachment.rs`
- `src/api/client.rs` — `Client` struct (`inner()`, `base_url()`) — same client for all new API calls
- `src/tui/app.rs` — `Screen` enum, `Vec<Screen>` stack, push/pop navigation (D-31) — extend with `CommentsBrowse { page_id }`
- `src/tui/screens/pages.rs` — pure render function pattern — replicate for `src/tui/screens/comments.rs`
- `src/cli/page.rs` — `$EDITOR` suspend/restore pattern (D-41), CLI handler structure — replicate for `src/cli/comment.rs` and `src/cli/attachment.rs`
- `src/output/mod.rs` — `OutputFormatter` — reuse for `ccli attachment list` and `ccli page search` table output
- `src/output/xml.rs` — `strip_storage_xml()` — reuse for comment body preview in TUI (D-50)

### Key Crates (already in Cargo.toml)
- `reqwest` — multipart upload (`client.inner().post(url).multipart(form)`) for ATTCH-03
- `ratatui` / `crossterm` — CommentsBrowse TUI screen
- `nucleo` / `nucleo-matcher` — fuzzy filter on comment list (optional for v1; defer if complex)
- `open` — `o` keybind in CommentsBrowse if comment has a `_links.webui`

### Key Crates to Add in Phase 4
- `mime_guess` (or `infer`) — MIME type detection from file extension for attachment upload (D-55). Check if already present; if not, add minimal version.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/api/page.rs` `list_all_pages()` — pagination walk (while next_start exists). Copy and adapt for `list_all_comments(page_id)` in `src/api/comment.rs` and `list_all_attachments(page_id)` in `src/api/attachment.rs`. Same `Client.inner()` + `AppError` taxonomy.
- `src/cli/page.rs` `$EDITOR` workflow — suspend TUI → write temp file → launch editor → read file → POST → restore. Replicate in `src/cli/comment.rs` for `ccli comment add`. Use the plain-text → XML wrapping step (D-52) instead of reading raw XML.
- `src/output/mod.rs` `OutputFormatter::print()` — TTY-aware table/TSV dispatch. Reuse directly for `ccli attachment list` and `ccli page search` output (both are table-only).
- `src/tui/app.rs` `Screen` enum, `Vec<Screen>` stack — extend with `CommentsBrowse { page_id: String }`. Same `push(screen)` on `c` key, `pop()` on Esc. One new arm in the `match screen` dispatch.
- `src/tui/screens/pages.rs` pure render function — replicate layout for `src/tui/screens/comments.rs`: left list pane (author + date + first line), right preview pane (full text via `strip_storage_xml()`).
- `src/output/xml.rs` `strip_storage_xml()` — reuse unchanged for comment preview rendering.

### Established Patterns
- **`#[instrument(skip(client))]` on every async API fn** — PAT never logged (security invariant from all prior phases).
- **`AppError` taxonomy (Auth / Network / Api / Config)** — no new variants needed; map all new API errors to existing variants.
- **`reqwest` `.query(&[...])` builder** — use for all new GET endpoints (WR-05 space_key injection prevention generalizes to all query params).
- **`ContentType` enum in `page.rs`** — not applicable to comments/attachments but the pattern (enum + `as_api_str()`) is the model for any new discriminators.
- **`sanitize_id()` digits-only validation** — apply to `page_id` in all new CLI handlers (T-03-14 path traversal mitigation).

### Integration Points
- `src/cli/mod.rs` `Commands` enum — add `Comment(CommentArgs)` and `Attachment(AttachmentArgs)` alongside existing `Space`, `Page`, `Blog`, `Init`.
- `src/main.rs` — dispatch `Some(Commands::Comment(args))` and `Some(Commands::Attachment(args))` to their handlers.
- `src/tui/app.rs` `Screen` enum + event loop in `src/tui/mod.rs` — add `CommentsBrowse` arm to screen match; add `c` key handler in `PagesBrowse` key dispatch.
- Key-binding help bar in `src/tui/screens/pages.rs` — add `c: comments` to the help row shown at the bottom of the page list screen.

</code_context>

<specifics>
## Specific Ideas

- **CQL search is explicitly NOT TUI** — even on an interactive TTY. Search results are a snapshot, not a browseable live list. The `ccli page search` command is the composable, pipe-friendly entry point. This is an intentional deviation from D-35.
- **`c` key as the comments entry point in TUI** — user said `c` is the right mnemonic. Must be added to the key-binding help bar so it's discoverable.
- **Plain text for comments = a deliberate UX split** — pages use raw XML (D-40) because they need structure; comments are prose. This divergence is intentional and should be noted in code comments when implementing `ccli comment add`.
- **Attachment download defaults to CWD** — `ccli attachment get 12345 design.png` saves to `./design.png`. The `--output` flag is available but optional. This mirrors how common CLI tools (`curl -O`, `wget`) work.
- **MIME auto-detection for uploads** — use `mime_guess` (or `infer`) to avoid requiring users to know MIME strings. This is important for Confluence to render image attachments inline correctly.

</specifics>

<deferred>
## Deferred Ideas

- **Browseable search results in TUI** — opening `ccli page search` in a TUI screen with preview pane. Could be a v2 feature; decided against for v1 (table output is sufficient and simpler).
- **Inline/body comments vs page-level comments** — Confluence distinguishes footer comments (page-level) from inline comments (anchored to specific text). Phase 4 targets page-level comments only. Inline comment API is more complex; defer post-v1.
- **Attachment version history** — Confluence supports versioned attachments. Out of scope for v1; listing shows latest version only.
- **Batch attachment download** — downloading all attachments from a page at once. Post-v1 convenience feature.
- **Saved CQL aliases (SRCH-01)** — v2 requirement already in REQUIREMENTS.md; not part of Phase 4.
- **Fuzzy filter on comment list in TUI** — the CommentsBrowse screen could support `/` to filter comments. Defer: pages often have few comments; filter adds complexity with minimal payoff for v1.

</deferred>

---

*Phase: 04-comments-attachments-search*
*Context gathered: 2026-05-07*
