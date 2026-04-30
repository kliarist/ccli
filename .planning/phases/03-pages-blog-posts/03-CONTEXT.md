# Phase 3: Pages & Blog Posts - Context

**Gathered:** 2026-04-30
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver the core content workflow: users can list, view, create, and edit Confluence Pages and Blog Posts entirely from the terminal.

**TUI surface:** Enter on a space in the Spaces browse screen drills into that space's page list (screen stack). The page list has a list + preview pane (same 40/60 split as Phase 2). From the page list, users can open in browser (`o`), edit in `$EDITOR` (`e`), or fuzzy-filter by title (`/`).

**CLI surface:** `ccli page list <SPACE-KEY>`, `ccli page view <PAGE-ID>`, `ccli page create`, `ccli page edit <PAGE-ID>` — mirrored exactly by `ccli blog` subcommands using `type=blogpost` filter.

Requirements covered: PAGE-01, PAGE-03, PAGE-04, PAGE-05, PAGE-06, PAGE-07, PAGE-08, BLOG-01, BLOG-02, BLOG-03, BLOG-04.

Out of phase: CQL search (`ccli page search`) is Phase 4 (PAGE-02). Comments (Phase 4). Attachments (Phase 4). Sub-page tree navigation. Clipboard support.

</domain>

<decisions>
## Implementation Decisions

### TUI Drill-Down Navigation

- **D-30:** **Enter on a space drills into that space's page list.** The Spaces browse screen gains an Enter handler that pushes a `PagesBrowse { space_key }` screen onto the navigation stack. Esc pops back to `SpacesBrowse`.
- **D-31:** **Enum-based screen stack.** App grows a `Vec<Screen>` where `Screen` is an enum (`SpacesBrowse`, `PagesBrowse { space_key }`). Push on Enter, pop on Esc. Standard Ratatui community pattern — no new crates needed.
- **D-32:** **Enter on a page in the TUI = open in browser (same as `o`).** The TUI is browse/edit-oriented; viewing full content is handled by the preview pane (truncated) or `ccli page view` (stdout). Enter is a power-user shortcut to the browser, consistent with D-12.
- **D-33:** **Flat list of all pages in the space, sorted by title ascending.** No hierarchical tree view. Sub-page relationships visible in the preview pane (parent field). Same simplicity as the spaces list.

### CLI Interface

- **D-34:** **`ccli page list <SPACE-KEY>` — space key is a required positional argument.** Concise and shell-friendly. Error when omitted: `Space key required — usage: ccli page list <SPACE-KEY>`. Same pattern for `ccli blog list <SPACE-KEY>`.
- **D-35:** **TUI when TTY, table when piped (D-06 pattern).** `ccli page list DEV` on an interactive terminal opens the TUI pages screen pre-loaded to that space. Piped or `--plain` → formatted table output (title, ID, author columns). Consistent with the Phase 1 TTY-auto-detect contract.
- **D-36:** **`ccli page view <PAGE-ID>` prints plain text to stdout.** Strip storage XML tags: paragraphs as text blocks, headings as uppercase lines, code blocks as indented text. Human-readable, greppable, pipeable. Same stripping logic used in the TUI preview pane (shared implementation).
- **D-37:** **`ccli blog` is an exact mirror of `ccli page`.** Commands: `ccli blog list <SPACE-KEY>`, `ccli blog create`, `ccli blog edit <POST-ID>`, `ccli blog view <POST-ID>`. Blog posts fetched using `type=blogpost` filter on the same `/rest/api/content` endpoint. No UX divergence between pages and blog posts in Phase 3.

### Editor Workflow (create & edit)

- **D-38:** **`ccli page create` uses CLI flags with interactive fallback for missing values.** Flags: `--space <KEY>` (required), `--title <TITLE>` (required), `--parent <PAGE-ID>` (optional). If flags are omitted, prompt interactively via `dialoguer` (same pattern as `ccli init`). Then `$EDITOR` opens with the XML template. `ccli blog create` uses the same pattern (no `--parent` flag — blog posts don't have parent pages).
- **D-39:** **409 version conflict on `ccli page edit`: show error and preserve temp file.** On HTTP 409: print to stderr: `Conflict: page was updated by someone else (version N). Your edits saved at /tmp/ccli-edit-<ID>.xml — re-open to merge manually.` Temp file is NOT deleted. User merges manually and retries.
- **D-40:** **Minimal storage XML template for `ccli page create`.** `$EDITOR` opens with: `<p>Start writing here...</p>` (just a body element with a placeholder paragraph). No comments, no macro examples. Users writing complex content already know the storage format; the template just makes the file valid.
- **D-41:** **`e` keybinding in TUI pages screen to edit selected page.** Pressing `e` on a highlighted page suspends the TUI (restore terminal), fetches the current storage XML, writes it to `/tmp/ccli-edit-<PAGE-ID>.xml`, launches `$EDITOR`, then on exit reads the file and PUTs to Confluence. Restores the TUI on completion. Logs success or error in the status bar on resume.

### Preview Pane (Pages & Blog Posts)

- **D-42:** **Preview pane shows stripped plain text.** Fetch page body with `?expand=body.storage`, strip XML/HTML tags, render paragraphs as text and headings as uppercase lines. Shared `strip_storage_xml()` function — same logic used by `ccli page view` stdout output.
- **D-43:** **Same D-19 debounce + session cache pattern for preview fetches.** 150ms settle after selection change, then fetch `?expand=body.storage,version,ancestors`. Results cached in-memory keyed by page ID. Re-visiting a page is instant; fast j/k navigation doesn't trigger a request storm.
- **D-44:** **List pane shows title only (left, 40% width).** Author, last modified, version, and parent page shown in the preview pane after selection. Clean list, readable in narrow terminals. Non-interactive `ccli page list` table shows title + ID + author columns (richer — no width constraint).

### Claude's Discretion

- **API call for page list:** Use `/rest/api/content?spaceKey=DEV&type=page&limit=50&expand=version,ancestors` — include version + ancestors so the preview pane has enough data without a second call.
- **Pagination for page list:** Same walk-all-pages approach as D-14 (spaces). Fetch until exhausted; spinner shows progress. Pages are stored in memory.
- **Module layout:** `src/api/page.rs` (page + blog post API calls), `src/cli/page.rs`, `src/cli/blog.rs`, `src/tui/screens/pages.rs`. Screen stack lives in `src/tui/app.rs` as `Vec<Screen>`.
- **`e` keybind editor pattern:** Suspend TUI (`disable_raw_mode` + `LeaveAlternateScreen`), write temp file, `std::process::Command::new($EDITOR).arg(path).status()`, read file, PUT, restore TUI (`enable_raw_mode` + `EnterAlternateScreen`). Same suspend/restore as Phase 2's headless browser fallback (D-23).
- **Blog post list in TUI:** Separate screen entry via `BlogsBrowse { space_key }` in the Screen enum. Accessed from a future "Blog Posts" option on the space preview pane or via `ccli blog list DEV` (which opens TUI when TTY).
- **Plain text stripping implementation:** A small `strip_storage_xml(xml: &str) -> String` helper. Use `quick-xml` (already common in Rust ecosystem) or simple regex for tag removal. Keep it simple — not a full renderer.
- **Decision ID continuation:** Phase 2 ended at D-25. Phase 3 starts at D-30 (gap intentional for small Phase 2 additions if needed).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project & Phase Inputs
- `.planning/REQUIREMENTS.md` — PAGE-01 through PAGE-08, BLOG-01 through BLOG-04 (Phase 3 requirements with acceptance criteria)
- `.planning/PROJECT.md` — Key decisions (raw XML editing locked, jira-cli UX model, PAT-only auth, out-of-scope items)
- `.planning/ROADMAP.md` §"Phase 3: Pages & Blog Posts" — goal, success criteria, requirement mapping
- `.planning/phases/02-tui-shell-spaces/02-CONTEXT.md` — Phase 2 locked decisions (D-10..D-25); D-12 (q/Esc), D-16 (nucleo), D-19 (debounce+cache), D-20 (40/60 split), D-22 (`open` crate), D-24 (`_links.webui`) all apply directly to Phase 3
- `.planning/phases/01-foundation/01-CONTEXT.md` — D-06 (TTY auto-detect) applies to `ccli page list` output behavior

### Reference Implementation
- `https://github.com/ankitpokhrel/jira-cli` — UX model. Reuse for: screen drill-down pattern (project → issue list), `e` to edit, `o` to browser, flat list with preview pane. NOT Rust — patterns only.

### Target API (Confluence DC 9.2.19)
- Confluence DC REST API v1: `https://docs.atlassian.com/ConfluenceServer/rest/latest/`
  - `GET /rest/api/content?spaceKey=DEV&type=page&limit=50&expand=version,ancestors` — page list for a space
  - `GET /rest/api/content?spaceKey=DEV&type=blogpost&limit=50&expand=version,ancestors` — blog post list
  - `GET /rest/api/content/<ID>?expand=body.storage,version,ancestors` — full page for preview/edit
  - `POST /rest/api/content` — create page or blog post
  - `PUT /rest/api/content/<ID>` — update page/blog post (requires `version.number` from GET)
  - `GET /rest/api/content/<ID>?expand=body.storage` — fetch for preview pane (D-43)

### Phase 2 Code (already implemented — reuse, don't re-build)
- `src/api/space.rs` — pagination walk pattern (`list_all_spaces`) — replicate for pages in `src/api/page.rs`
- `src/tui/app.rs` — `App` struct, `AppState` enum, `KeyAction` enum — extend with `Screen` stack concept and `PagesBrowse` state
- `src/tui/screens/spaces.rs` — pure render function pattern — replicate for `src/tui/screens/pages.rs`
- `src/tui/mod.rs` — event loop with `tokio::select!` — extend to handle screen stack routing
- `src/cli/mod.rs` — `Commands` enum and `SpaceArgs` pattern — add `Page(PageArgs)` and `Blog(BlogArgs)` following the same structure
- `src/output/mod.rs` — `OutputFormatter` — reuse for `ccli page list` non-interactive table output
- `src/cli/init.rs` — `dialoguer` prompt pattern — reuse for `ccli page create` interactive flag prompting (D-38)

### Key Crates (already in `Cargo.toml`)
- `ratatui` — TUI framework (screen stack, pages list rendering)
- `crossterm` — terminal backend + raw-mode suspend/restore for editor launch (D-41)
- `nucleo` / `nucleo-matcher` — fuzzy filter on page titles (D-16 pattern)
- `open` — browser launch for `o` keybind (D-22)
- `tokio` — async runtime, spawn fetch tasks for preview (D-43)

### Key Crates to Add in Phase 3
- `quick-xml` (or `roxmltree`) — XML tag stripping for `strip_storage_xml()` helper (D-42, D-36). Check if already present; if not, add minimal version.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/api/space.rs` `list_all_spaces()` — pagination walk (while next_start exists, keep fetching). Copy and adapt for `list_all_pages(space_key, content_type)` — same pattern, different endpoint + params.
- `src/tui/app.rs` `App` struct — owns Vec<Space>, filtered_indices, nucleo filter, debounce/cache. Extend by making `App` hold a `Vec<Screen>` stack; each Screen variant owns its own state (page list, filter, cache, list_state).
- `src/tui/screens/spaces.rs` pure render function — stateless Ratatui rendering takes `&App` returns Frame draw commands. Replicate for `render_pages(app, frame, area)`.
- `src/output/mod.rs` `OutputFormatter::print()` — TTY-aware table/TSV dispatch. Reuse directly for `ccli page list` and `ccli blog list` non-interactive output.
- `src/cli/space.rs` `handle_space_list()` — non-interactive handler pattern. Copy for `handle_page_list()` and `handle_blog_list()`.
- `src/cli/init.rs` dialoguer usage — interactive prompt with validation. Reuse for `ccli page create` title/space prompting (D-38).

### Established Patterns
- **Async-first handlers:** every CLI handler is `async fn`. Page API calls follow same `#[instrument(skip(client))]` + PAT-never-logged discipline.
- **D-NN decision IDs in comments:** Phase 3 uses D-30..D-44. Planner should annotate code comments with these IDs where decisions are implemented.
- **`AppError` taxonomy:** Auth / Network / Api / Config — classify page API errors into these existing variants. No new error variants needed.
- **Test seam:** pure state structs (no terminal I/O) are unit-testable. `PagesApp` state transitions should be tested the same way `App` (spaces) is tested.
- **Suspend/restore for browser fallback (D-23):** `disable_raw_mode()` → action → `enable_raw_mode()`. Same pattern applies for launching `$EDITOR` (D-41).

### Integration Points
- `src/cli/mod.rs` `Commands` enum — add `Page(PageArgs)` and `Blog(BlogArgs)` alongside existing `Space(SpaceArgs)` and `Init`.
- `src/main.rs` — dispatch `Some(Commands::Page(args))` and `Some(Commands::Blog(args))` to their handlers; `None` (bare `ccli`) stays as TUI launch.
- `src/tui/mod.rs` `run()` — currently hard-coded to Spaces screen. Extend to accept an initial `Screen` variant; `None` subcommand starts on `SpacesBrowse`, `ccli page list DEV` (TTY) starts on `PagesBrowse { space_key: "DEV" }`.
- `~/.config/ccli/config.toml` — base URL used for building `_links.webui` absolute URLs (D-24). Same `Client::base_url()` accessor.

</code_context>

<specifics>
## Specific Ideas

- **jira-cli drill-down is the exact UX model** — from the project list, Enter navigates to that project's issue list. Phase 3 mirrors this exactly: Spaces list → Enter → Pages list for that space.
- **`e` keybind is a first-class TUI action** — power users stay in the TUI for their entire workflow: browse → e → edit in vim → save → TUI resumes. No round-trip to the shell needed.
- **Shared `strip_storage_xml()` implementation** — `ccli page view` and the TUI preview pane use identical tag-stripping logic. Single function in a shared module (e.g., `src/output/xml.rs`). Keeps behavior consistent.
- **Blog posts and pages share the same TUI screen** — `PagesBrowse { space_key, content_type: ContentType }` where `ContentType` is `Page | BlogPost`. One screen implementation, two entry points. Less code duplication.
- **`--parent` flag for `ccli page create` is optional** — blog posts don't use it. Pages without `--parent` are created at the space root level (Confluence default behavior).

</specifics>

<deferred>
## Deferred Ideas

- **CQL search (`ccli page search`)** — PAGE-02 is explicitly Phase 4.
- **Page version history view** — viewing version history in TUI; complex, post-v1.
- **Sub-page tree navigation** — hierarchical page browsing deferred; flat list is sufficient for v1.
- **Blog posts in spaces TUI preview pane** — Phase 2's space preview shows metadata only (D-18). Adding a "Blog Posts (N)" count or link to the space preview is a small addition for a future iteration.
- **Clipboard support (`c`/`y` to copy page ID or URL)** — deferred from Phase 2 (D-25). Still deferred; re-evaluate after Phase 3 ships and page IDs become commonly yanked.
- **`ccli page create` with Markdown input** — out of scope in v1 per PROJECT.md (raw XML editing, no conversion layer). Could be a v2 feature.

</deferred>

---

*Phase: 03-pages-blog-posts*
*Context gathered: 2026-04-30*
