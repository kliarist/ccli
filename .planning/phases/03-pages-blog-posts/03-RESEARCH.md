# Phase 3: Pages & Blog Posts - Research

**Researched:** 2026-05-05
**Domain:** Rust / Ratatui TUI, Confluence DC REST API v1, XML tag stripping, editor suspend/restore
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**TUI Drill-Down Navigation**
- D-30: Enter on a space drills into that space's page list. Spaces browse screen gains Enter handler that pushes `PagesBrowse { space_key }` onto a navigation stack. Esc pops back to SpacesBrowse.
- D-31: Enum-based screen stack. App grows a `Vec<Screen>` where `Screen` is an enum (`SpacesBrowse`, `PagesBrowse { space_key }`). Push on Enter, pop on Esc. No new crates needed.
- D-32: Enter on a page in the TUI = open in browser (same as `o`). Preview is via the pane (truncated) or `ccli page view` (stdout). Enter is consistent with D-12.
- D-33: Flat list of all pages in the space, sorted by title ascending. No hierarchical tree view. Sub-page relationships visible in the preview pane (parent field).

**CLI Interface**
- D-34: `ccli page list <SPACE-KEY>` — space key is a required positional argument. Error when omitted. Same pattern for `ccli blog list <SPACE-KEY>`.
- D-35: TUI when TTY, table when piped (D-06 pattern). `ccli page list DEV` on interactive terminal opens TUI pages screen pre-loaded to that space. Piped or `--plain` → formatted table.
- D-36: `ccli page view <PAGE-ID>` prints plain text to stdout. Strip storage XML tags: paragraphs as text blocks, headings as uppercase lines, code blocks as indented text. Shared `strip_storage_xml()`.
- D-37: `ccli blog` is an exact mirror of `ccli page`. Blog posts fetched using `type=blogpost` filter. No UX divergence between pages and blog posts.

**Editor Workflow**
- D-38: `ccli page create` uses CLI flags with interactive fallback. Flags: `--space <KEY>` (required), `--title <TITLE>` (required), `--parent <PAGE-ID>` (optional). Prompts via `dialoguer` if omitted. `ccli blog create` same (no `--parent`).
- D-39: 409 version conflict on `ccli page edit`: show error and preserve temp file. Print to stderr with temp file path. Temp file is NOT deleted.
- D-40: Minimal storage XML template for `ccli page create`: `<p>Start writing here...</p>`. No comments, no macro examples.
- D-41: `e` keybinding in TUI pages screen to edit selected page. Suspend TUI (disable_raw_mode + LeaveAlternateScreen), fetch XML, write to `/tmp/ccli-edit-<PAGE-ID>.xml`, launch `$EDITOR`, read file, PUT to Confluence, restore TUI. Log success or error in status bar on resume.

**Preview Pane**
- D-42: Preview pane shows stripped plain text. Shared `strip_storage_xml()` function.
- D-43: Same D-19 debounce + session cache pattern for preview fetches. 150ms settle, cache keyed by page ID. Endpoint: `GET /rest/api/content/<ID>?expand=body.storage,version,ancestors`.
- D-44: List pane shows title only (left, 40% width). Author, last modified, version, and parent page shown in preview pane.

### Claude's Discretion

- API call for page list: `GET /rest/api/content?spaceKey=DEV&type=page&limit=50&expand=version,ancestors`
- Pagination: walk all pages until exhausted; spinner shows progress
- Module layout: `src/api/page.rs`, `src/cli/page.rs`, `src/cli/blog.rs`, `src/tui/screens/pages.rs`; Screen stack in `src/tui/app.rs` as `Vec<Screen>`
- `e` keybind editor pattern: disable_raw_mode → write temp → spawn $EDITOR → read file → PUT → enable_raw_mode
- Blog post TUI: `BlogsBrowse { space_key }` in Screen enum — but CONTEXT.md specifics note suggests sharing the same screen parameterized by `ContentType`
- Plain text stripping: `quick-xml` or simple regex; shared `src/output/xml.rs`

### Deferred Ideas (OUT OF SCOPE)

- CQL search (`ccli page search`) — Phase 4 (PAGE-02)
- Page version history view
- Sub-page tree navigation
- Blog posts count in spaces TUI preview pane
- Clipboard support (`c`/`y`)
- `ccli page create` with Markdown input

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PAGE-01 | User can list pages within a space in a TUI list + preview pane | TUI screen stack (D-31), PagesBrowse screen, pagination walk mirrors list_all_spaces |
| PAGE-03 | User can fuzzy-filter the page list in TUI by title | Same nucleo-matcher pattern as D-16; filter on title only (D-44) |
| PAGE-04 | User can view full page content in TUI preview pane | strip_storage_xml() shared function; debounce + cache (D-43) |
| PAGE-05 | User can create a new page via `ccli page create`, opens $EDITOR with template | dialoguer prompts (D-38); POST /rest/api/content; D-40 template |
| PAGE-06 | User can edit page's storage XML via `ccli page edit <PAGE-ID>` | suspend/restore pattern (D-41); PUT /rest/api/content/{id} with version+1 |
| PAGE-07 | User can press `o` on a page in TUI to open in system browser | open crate reuse, resolve_webui_url pattern (D-24) |
| PAGE-08 | User can run `ccli page view <PAGE-ID>` to print page content to stdout | strip_storage_xml() shared function; GET /rest/api/content/{id}?expand=body.storage |
| BLOG-01 | User can list blog posts in a space in TUI list + preview pane | Same PagesBrowse screen with ContentType::BlogPost; type=blogpost filter |
| BLOG-02 | User can create a new blog post via `ccli blog create` | Same pattern as PAGE-05 without --parent flag; type=blogpost in POST body |
| BLOG-03 | User can edit an existing blog post via `ccli blog edit <POST-ID>` | Same suspend/restore + PUT pattern as PAGE-06 |
| BLOG-04 | User can press `o` on a blog post in TUI to open in system browser | Same as PAGE-07 |

</phase_requirements>

---

## Summary

Phase 3 builds on a solid Phase 2 foundation that already delivers TUI infrastructure (event loop, spinner, debounce, preview cache, nucleo filter, suspend/restore for browser). All patterns for Phase 3 have proven precedents in the existing codebase — this phase is extension, not invention.

The two substantive additions relative to Phase 2 are: (1) a screen stack (`Vec<Screen>`) replacing the single-screen app, and (2) editor suspend/restore for in-TUI editing. The screen stack is a straightforward Rust enum extension to `App`. The editor workflow uses the same `disable_raw_mode` / `enable_raw_mode` + `LeaveAlternateScreen` / `EnterAlternateScreen` pair that already exists in `handle_open_browser` for the headless fallback (D-23).

The `strip_storage_xml()` function is the one genuinely new piece of logic — a small XML-to-plain-text converter shared between `ccli page view` and the TUI preview pane. The Confluence content REST API is a straightforward JSON API; POST body shape and PUT version increment are well-documented and match established community patterns.

**Primary recommendation:** Follow the module layout from CONTEXT.md exactly. Add `quick-xml = "0.39"` to Cargo.toml. Implement the screen stack as a `Vec<Screen>` on `App` with `PagesBrowseState` as the per-screen state struct. Replicate the spaces event loop pattern verbatim for the pages screen. All five new files (`src/api/page.rs`, `src/cli/page.rs`, `src/cli/blog.rs`, `src/tui/screens/pages.rs`, `src/output/xml.rs`) have clear existing templates to copy from.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Page/blog list fetch + pagination | API layer (`src/api/page.rs`) | — | Mirrors space.rs; all network I/O isolated to api/ |
| Page detail fetch (preview body) | API layer (`src/api/page.rs`) | — | Same debounce+channel pattern as space detail |
| XML tag stripping to plain text | Output layer (`src/output/xml.rs`) | — | Shared by TUI preview and CLI view; no terminal I/O |
| TUI screen stack management | TUI app layer (`src/tui/app.rs`) | — | `Vec<Screen>` owns all per-screen state structs |
| TUI pages/blogs browse rendering | TUI screen layer (`src/tui/screens/pages.rs`) | — | Pure render function; no I/O, no state mutation |
| CLI `page`/`blog` subcommand dispatch | CLI layer (`src/cli/page.rs`, `src/cli/blog.rs`) | — | TTY-detect dispatch via OutputFormatter (D-35) |
| Editor suspend/restore workflow | TUI event loop (`src/tui/mod.rs`) | API layer (PUT) | Crossterm lifecycle; API call on editor exit |
| Create page/blog (CLI + API) | CLI layer (prompt/flag handling) | API layer (POST) | dialoguer prompts → POST /rest/api/content |
| Browser open on `o` / Enter | TUI event loop | `open` crate | Same handle_open_browser pattern as Phase 2 |

---

## Standard Stack

### Core (already in Cargo.toml — verified)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.30 | TUI framework: List, Paragraph, Block, Layout | Already in use; Phase 3 uses no new widgets |
| crossterm | 0.29 | Terminal backend; raw mode on/off; alt screen | Already in use; D-41 uses disable_raw_mode/enable_raw_mode |
| nucleo-matcher | 0.3 | Fuzzy filter on page titles | Already in use per D-16; same pattern applies |
| tokio | 1 (full) | Async runtime; oneshot + mpsc channels for fetches | Already in use; same spawn pattern |
| reqwest | 0.13 | HTTP client; GET/POST/PUT to Confluence DC | Already in use; Client::clone() is cheap |
| serde / serde_json | 1 | JSON deserialize API responses; serialize POST/PUT body | Already in use |
| dialoguer | 0.12 | Interactive prompts for `ccli page create` (D-38) | Already in use in ccli init |
| open | 5 | Browser launch on `o` keybind (D-22) | Already in use |
| is-terminal | 0.4 | TTY auto-detect for D-35 output dispatch | Already in use in OutputFormatter |
| anyhow / thiserror | 1 / 2 | Error propagation; AppError variants | Already in use |

[VERIFIED: Cargo.toml in repo]

### New Dependency to Add

| Library | Version | Purpose | Why This One |
|---------|---------|---------|--------------|
| quick-xml | 0.39 | Parse Confluence storage XML to strip tags for strip_storage_xml() | Fastest Rust XML reader; event-based API well-suited to tag stripping without building a full DOM; roxmltree (0.21) is the alternative but requires DOM allocation |

[VERIFIED: `cargo search quick-xml` returned `quick-xml = "0.39.3"` as latest]

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| quick-xml | roxmltree | roxmltree builds a full read-only DOM — simpler API but higher memory. For tag stripping quick-xml's event-based reader is sufficient and lighter. |
| quick-xml | regex | Regex tag stripping is brittle: doesn't handle CDATA sections, nested quotes in attributes, or malformed XML gracefully. quick-xml is minimal effort for correctness. |
| Vec<Screen> stack | AppState variants per screen | State variants would make the event loop a match explosion as screens multiply. Vec<Screen> with per-screen state structs scales cleanly. |

**Installation:**
```bash
cargo add quick-xml@0.39
```

---

## Architecture Patterns

### System Architecture Diagram

```
User keyboard input
        │
        ▼
 src/tui/mod.rs (event loop)
  ┌─────────────────────────────────────────────────────────────┐
  │  tokio::select! / event::poll(100ms)                        │
  │                                                             │
  │  Screen stack top = SpacesBrowse ──────────────────────────►│ render_spaces()
  │                                                             │
  │  Screen stack top = PagesBrowse                             │
  │    { space_key, content_type }  ───────────────────────────►│ render_pages()
  │                                                             │
  │  Key: Enter (on space) ──► push PagesBrowse onto stack      │
  │  Key: Esc (on pages)   ──► pop stack → SpacesBrowse         │
  │  Key: o / Enter (page) ──► resolve_webui_url → open::that() │
  │  Key: e (page)         ──► EDITOR WORKFLOW (see below)      │
  └─────────────────────────────────────────────────────────────┘
        │
        ├── tokio::spawn ──► list_all_pages(space_key, type)
        │                     GET /rest/api/content?spaceKey=X&type=Y
        │                     → oneshot channel → app.set_pages()
        │
        ├── tokio::spawn ──► get_page_detail(id)
        │    (after 150ms     GET /rest/api/content/{id}?expand=body.storage,version,ancestors
        │     debounce)       → mpsc channel → app.cache_page_detail()
        │
        └── EDITOR WORKFLOW (D-41):
              disable_raw_mode()
              LeaveAlternateScreen
              GET /rest/api/content/{id}?expand=body.storage,version ─► write /tmp/ccli-edit-{id}.xml
              Command::new($EDITOR).arg(path).status()
              read /tmp/ccli-edit-{id}.xml
              PUT /rest/api/content/{id}  { version: { number: N+1 }, body.storage.value: ... }
              EnterAlternateScreen
              enable_raw_mode()
              status bar: "Saved." (green) or "Conflict: ..." (red) or "Error: ..." (red)

CLI (non-TUI paths):
  ccli page list <KEY>  ──TTY──► TUI (PagesBrowse pre-loaded)
                         ──pipe─► OutputFormatter table
  ccli page view <ID>   ──────► GET body.storage → strip_storage_xml() → stdout
  ccli page create      ──────► dialoguer prompts → POST /rest/api/content
  ccli page edit <ID>   ──────► GET body.storage → /tmp/ccli-edit-{id}.xml → $EDITOR → PUT
  (ccli blog * mirrors ccli page * with type=blogpost)
```

### Recommended Project Structure

```
src/
├── api/
│   ├── mod.rs            # add page:: re-exports
│   ├── page.rs           # list_all_pages, get_page_detail, create_page, update_page
│   └── space.rs          # (unchanged)
├── cli/
│   ├── blog.rs           # handle_blog_{list,view,create,edit} — thin wrappers calling page handlers
│   ├── mod.rs            # add Page(PageArgs) + Blog(BlogArgs) to Commands enum
│   ├── page.rs           # handle_page_{list,view,create,edit}
│   └── (init.rs, space.rs unchanged)
├── output/
│   ├── mod.rs            # (unchanged)
│   └── xml.rs            # pub fn strip_storage_xml(xml: &str) -> String
└── tui/
    ├── app.rs            # EXTEND: Screen enum, Vec<Screen> stack, PagesBrowseState struct
    ├── mod.rs            # EXTEND: event loop routes to render_spaces or render_pages
    └── screens/
        ├── mod.rs        # add pub mod pages
        ├── pages.rs      # render_pages(f, screen_state, area) — pure render
        └── spaces.rs     # EXTEND: Enter key triggers D-30 drill-down
```

### Pattern 1: Screen Stack (D-31)

**What:** `Vec<Screen>` where `Screen` is an enum. Push on Enter-space, pop on Esc-from-pages. Each Screen variant carries its own state.

**When to use:** Any time the TUI needs multi-level navigation.

**Example:**
```rust
// Source: CONTEXT.md D-31 + 03-UI-SPEC.md Screen Stack Contract
pub enum Screen {
    SpacesBrowse,
    PagesBrowse {
        space_key: String,
        content_type: ContentType,
        state: Box<PagesBrowseState>,
    },
}

pub enum ContentType {
    Page,
    BlogPost,
}

pub struct App {
    // Phase 2 fields unchanged
    pub spaces: Vec<Space>,
    pub list_state: ListState,
    pub filtered_indices: Vec<usize>,
    pub state: AppState,
    pub preview_cache: HashMap<String, SpaceDetail>,
    pub pending_preview_key: Option<String>,
    pub last_selection_change: Option<Instant>,
    pub spinner_frame: usize,
    pub spaces_fetched_count: usize,
    pub error: Option<String>,

    // Phase 3 addition
    pub screen_stack: Vec<Screen>,
}

// Event loop routing:
match app.screen_stack.last() {
    None | Some(Screen::SpacesBrowse) => render_spaces(f, &mut app, area),
    Some(Screen::PagesBrowse { state, .. }) => render_pages(f, state, area),
}
```

[ASSUMED] — exact field placement on App is discretionary; the above follows CONTEXT.md logic.

### Pattern 2: PagesBrowseState (per-screen state struct)

**What:** A self-contained state struct for a single pages/blog posts browse screen. Owns list, filter, cache, list_state, and status.

**When to use:** One instance per `PagesBrowse` Screen variant. Allows returning to SpacesBrowse and back without losing scroll position.

**Example:**
```rust
// Source: 03-UI-SPEC.md Module Layout Contract + CONTEXT.md
pub struct PagesBrowseState {
    pub pages: Vec<Page>,
    pub list_state: ListState,
    pub filtered_indices: Vec<usize>,
    pub browse_state: AppState,          // Loading / Browse / Filter / Modal
    pub preview_cache: HashMap<String, PageDetail>,
    pub pending_preview_id: Option<String>,
    pub last_selection_change: Option<Instant>,
    pub spinner_frame: usize,
    pub pages_fetched_count: usize,
    pub error: Option<String>,
    pub status_message: Option<StatusMessage>,  // for editor result display (D-41)
    pub space_key: String,
    pub content_type: ContentType,
}

pub struct StatusMessage {
    pub text: String,
    pub style: StatusStyle,  // Success (green) | Error (red) | Info (reset)
}
```

### Pattern 3: Confluence Content API (GET list)

**What:** Paginated walk over `/rest/api/content` with type and spaceKey filters.

**When to use:** `list_all_pages(client, space_key, content_type)` — mirrors `list_all_spaces`.

**Example:**
```rust
// Source: CONTEXT.md Claude's Discretion + verified GET pattern from Confluence docs
let url = format!(
    "{}/rest/api/content?spaceKey={}&type={}&start={}&limit=50&expand=version,ancestors",
    client.base_url().trim_end_matches('/'),
    space_key,
    match content_type { ContentType::Page => "page", ContentType::BlogPost => "blogpost" },
    start,
);
// Pagination: same as list_all_spaces — walk until _links.next absent or size < limit
// Sort result by title ascending (D-33)
all.sort_by(|a, b| a.title.cmp(&b.title));
```

[CITED: developer.atlassian.com/server/confluence/confluence-rest-api-examples]

### Pattern 4: Confluence Content API (POST create)

**What:** Create a page or blog post.

**Example:**
```rust
// Source: CITED from developer.atlassian.com/server/confluence/confluence-rest-api-examples
// Required fields: type, title, space.key, body.storage.value, body.storage.representation
let body = serde_json::json!({
    "type": match content_type { ContentType::Page => "page", ContentType::BlogPost => "blogpost" },
    "title": title,
    "space": { "key": space_key },
    // ancestors is optional — omit for blog posts; include for pages with --parent
    "ancestors": parent_id.map(|id| serde_json::json!([{"id": id}])),
    "body": {
        "storage": {
            "value": xml_content,          // e.g. "<p>Start writing here...</p>"
            "representation": "storage"
        }
    }
});
// POST /rest/api/content → 201 Created on success
```

### Pattern 5: Confluence Content API (PUT update with version increment)

**What:** Update an existing page. MUST increment version.number from the current version or receive HTTP 409.

**Example:**
```rust
// Source: CITED from developer.atlassian.com/server/confluence/confluence-rest-api-examples
// Flow: GET to read current version.number → increment → include in PUT body
let body = serde_json::json!({
    "id": page_id,
    "type": match content_type { ContentType::Page => "page", ContentType::BlogPost => "blogpost" },
    "title": current_title,   // required even when not changing title
    "space": { "key": space_key },
    "version": { "number": current_version + 1 },
    "body": {
        "storage": {
            "value": new_xml_content,
            "representation": "storage"
        }
    }
});
// PUT /rest/api/content/{id} → 200 OK on success, 409 on stale version
```

### Pattern 6: Editor Suspend/Restore (D-41)

**What:** Suspend TUI, launch $EDITOR, restore TUI. Same mechanism as D-23 browser headless fallback already in mod.rs.

**Example:**
```rust
// Source: CONTEXT.md D-41 + existing handle_open_browser in src/tui/mod.rs
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, LeaveAlternateScreen, EnterAlternateScreen};
use std::io::stdout;

// 1. Suspend
disable_raw_mode()?;
execute!(stdout(), LeaveAlternateScreen)?;

// 2. Write temp file
let path = format!("/tmp/ccli-edit-{}.xml", page_id);
std::fs::write(&path, xml_content)?;

// 3. Launch editor ($EDITOR → $VISUAL → vi fallback)
let editor = std::env::var("EDITOR")
    .or_else(|_| std::env::var("VISUAL"))
    .unwrap_or_else(|_| "vi".to_string());
std::process::Command::new(&editor).arg(&path).status()?;

// 4. Read modified content
let new_xml = std::fs::read_to_string(&path)?;

// 5. PUT to Confluence
// ...

// 6. Restore TUI
execute!(stdout(), EnterAlternateScreen)?;
enable_raw_mode()?;
// terminal.draw() on next loop iteration
```

### Pattern 7: strip_storage_xml() Contract

**What:** Convert Confluence storage XML to human-readable plain text. Shared between TUI preview pane and `ccli page view`.

**Location:** `src/output/xml.rs`

**Transformation rules (from 03-UI-SPEC.md):**
- `<h1>`..`<h6>` content → UPPERCASED text line + blank line
- `<p>` content → text line + blank line
- `<code>` / `<pre>` content → lines prefixed `"  "` (2-space indent)
- `<li>` content → prefixed `"- "`
- All other tags → strip tag, retain inner text
- HTML entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`) → decoded to characters
- Consecutive blank lines → collapsed to one blank line

**Implementation approach with quick-xml:**
```rust
// Source: quick-xml crate (event-based reader — no DOM allocation)
use quick_xml::Reader;
use quick_xml::events::Event;

pub fn strip_storage_xml(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut output = String::new();
    let mut in_heading = false;
    let mut in_code = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6" => in_heading = true,
                    b"code" | b"pre" => in_code = true,
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6" => {
                        in_heading = false;
                        output.push('\n');
                    }
                    b"p" | b"li" => output.push('\n'),
                    b"code" | b"pre" => in_code = false,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default();
                if in_heading {
                    output.push_str(&text.to_uppercase());
                } else if in_code {
                    for line in text.lines() {
                        output.push_str("  ");
                        output.push_str(line);
                        output.push('\n');
                    }
                } else {
                    output.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,   // Malformed XML: stop, return what we have
            _ => {}
        }
        buf.clear();
    }
    // Collapse consecutive blank lines
    // ...
    output
}
```

[ASSUMED] — exact quick-xml API shape confirmed by crate search (version 0.39.3); specific method names (`read_event_into`, `unescape`) based on training knowledge and should be verified against quick-xml 0.39 docs before implementation.

### Anti-Patterns to Avoid

- **Building URLs from the space key by hand**: Always use `_links.webui` from the API response + `resolve_webui_url()`. Keys may contain characters that produce invalid URLs (D-24, Pitfall 3 from Phase 2 RESEARCH).
- **Using `size` field as total count**: `size` in the content list response is the count on the current page, not the total. Use `_links.next` absence + `size < limit` to detect end of pagination (same Pitfall 7 from Phase 2).
- **Sending PUT without reading current version.number first**: The GET for the editor content already returns `version.number`. Store it and send `version.number + 1` in the PUT. Never guess or hardcode the version.
- **Deleting the temp file on 409**: D-39 explicitly preserves the temp file so the user can manually merge. The error message includes the path.
- **Mutating screen stack inside render functions**: render functions must remain pure (no I/O, no state mutation). All stack mutations happen in the event loop.
- **Stale ListState index after filter**: Pitfall 5 from Phase 2 — always reset `list_state.select(Some(0))` or `select(None)` after `apply_filter()`. `PagesBrowseState` must apply the same discipline.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| XML tag stripping | Custom regex-based stripper | quick-xml event reader | Regex breaks on attribute quotes, CDATA, namespaced tags; quick-xml handles these correctly |
| Fuzzy filtering on page titles | Custom scoring algorithm | nucleo-matcher (already dep) | Already integrated in Phase 2; same API applies to page titles |
| HTTP client with auth headers | Custom HTTP client | reqwest::Client (existing) | Client already configured with PAT Bearer header, TLS settings, 30s timeout |
| Interactive terminal prompts | Custom stdin reader | dialoguer (existing dep) | Handles TTY edge cases, echo suppression; already used in `ccli init` |
| Browser opening | shell exec | open crate (existing) | Cross-platform; sandboxed (no shell injection); already used in Phase 2 |
| TTY detection | `libc::isatty()` | is-terminal (existing dep) | Correct on all platforms; already in OutputFormatter |

**Key insight:** Every hard problem in Phase 3 already has a library solving it in Cargo.toml. The only new dependency is quick-xml for XML parsing.

---

## Common Pitfalls

### Pitfall 1: Stale version.number in PUT (HTTP 409)
**What goes wrong:** PUT to update page returns 409 Conflict.
**Why it happens:** `version.number` in PUT body must be exactly `current_version + 1`. If the page was updated by someone else between the GET and the PUT, the version numbers diverge.
**How to avoid:** Always fetch the page with `?expand=body.storage,version` immediately before opening the editor. Store the version number. When the user saves the editor, PUT with stored version + 1.
**Warning signs:** Any 409 response from PUT /rest/api/content/{id}.

### Pitfall 2: Title missing from PUT body causes title reset
**What goes wrong:** Page title is silently blanked after an edit.
**Why it happens:** The PUT body requires `title` even when not changing it. Omitting it resets to empty.
**How to avoid:** Fetch `title` along with `body.storage` in the GET (it's a top-level field in the content response). Include it verbatim in every PUT request.

### Pitfall 3: Stale ListState index after filter / list reload
**What goes wrong:** TUI panics on index-out-of-bounds when navigating a freshly-loaded page list.
**Why it happens:** Phase 2 established this as Pitfall 5 — `filtered_indices` shrinks but `list_state.selected()` still holds the old index.
**How to avoid:** Every call to `apply_filter()` on `PagesBrowseState` MUST reset `list_state.select(Some(0))` or `select(None)`. Same discipline as `App::apply_filter()` in Phase 2.

### Pitfall 4: TUI not restored if editor or PUT panics
**What goes wrong:** Terminal left in raw mode / alternate screen after a crash in the editor workflow.
**Why it happens:** `std::process::Command::status()` can error; `std::fs::read_to_string` can error. If any of these return early before `enable_raw_mode()` + `EnterAlternateScreen`, the user's terminal is stuck.
**How to avoid:** Use a guard struct or `defer`-style closure that calls `enable_raw_mode()` + `EnterAlternateScreen` unconditionally, OR wrap the entire editor block in a function that always restores before returning (even on `Err`). Match the defensive pattern from Phase 2's `ratatui::restore()` call.

### Pitfall 5: blog post `ancestors` field in POST
**What goes wrong:** Blog post creation fails or is silently placed wrong.
**Why it happens:** Blog posts do not have parent pages — the `ancestors` array must be omitted entirely from the POST body for blog posts, not sent as an empty array (which may error depending on DC version).
**How to avoid:** In `create_page(...)`, only include `"ancestors"` when `content_type == ContentType::Page && parent_id.is_some()`. Omit the key for blog posts.

### Pitfall 6: `size` vs `total` in content list response
**What goes wrong:** Pagination loop exits after first page even when more pages exist.
**Why it happens:** Content list response has no `total` field. `size` is the count of results in the current response page. Same pitfall as spaces list (Phase 2 Pitfall 7).
**How to avoid:** Check `_links.next` absence for pagination stop condition — same as `list_all_spaces`.

### Pitfall 7: quick-xml namespace-prefixed tags
**What goes wrong:** `strip_storage_xml()` misidentifies Confluence macro tags with namespace prefixes (e.g., `<ac:structured-macro>`).
**Why it happens:** Confluence storage format heavily uses `ac:` and `ri:` namespace-prefixed elements. quick-xml's `e.name().as_ref()` returns the full qualified name including prefix.
**How to avoid:** In `strip_storage_xml()`, match on `b"ac:structured-macro"` etc., or use `.local_name()` to strip the prefix. For Phase 3 the simplest safe approach is: for unrecognized tags, strip tag and retain text content — which is already the default rule.

---

## Code Examples

### API serde structs for content response
```rust
// Source: Confluence DC REST API + CONTEXT.md D-43 expand fields
// In src/api/page.rs

#[derive(Debug, Clone, Deserialize)]
pub struct ContentListResponse {
    pub results: Vec<Page>,
    pub start: u32,
    pub limit: u32,
    pub size: u32,   // per-page count, NOT total (Pitfall 6)
    #[serde(rename = "_links")]
    pub links: ContentListLinks,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ContentListLinks {
    pub next: Option<String>,
    pub base: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Page {
    pub id: String,          // Confluence content IDs are strings
    pub title: String,
    #[serde(rename = "type")]
    pub content_type: String,    // "page" or "blogpost"
    pub version: Option<PageVersion>,
    pub ancestors: Option<Vec<PageAncestor>>,
    #[serde(rename = "_links")]
    pub links: PageLinks,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PageVersion {
    pub number: u32,
    pub when: Option<String>,   // ISO 8601 timestamp
    pub by: Option<PageAuthor>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PageAuthor {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PageAncestor {
    pub id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PageLinks {
    pub webui: Option<String>,
    #[serde(rename = "self")]
    pub self_url: Option<String>,
}

// For preview fetch (includes body.storage)
#[derive(Debug, Clone, Deserialize)]
pub struct PageDetail {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub content_type: String,
    pub version: Option<PageVersion>,
    pub ancestors: Option<Vec<PageAncestor>>,
    pub body: Option<PageBody>,
    #[serde(rename = "_links")]
    pub links: PageLinks,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PageBody {
    pub storage: Option<StorageBody>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageBody {
    pub value: Option<String>,
}
```

[ASSUMED] — field naming conventions (`displayName` camelCase) based on training knowledge; verify against actual DC 9.2.19 API response before finalising deserialization struct.

### Confluencing Commands enum extension
```rust
// Source: existing src/cli/mod.rs pattern
// Add to Commands enum:
pub enum Commands {
    Init,
    Space(SpaceArgs),
    Page(PageArgs),   // Phase 3
    Blog(BlogArgs),   // Phase 3
}

#[derive(clap::Args, Debug)]
pub struct PageArgs {
    #[command(subcommand)]
    pub command: PageCommands,
}

#[derive(Subcommand, Debug)]
pub enum PageCommands {
    /// List pages in a space (TUI when TTY, table when piped).
    List {
        /// Space key (e.g. DEV)
        space_key: String,
    },
    /// Print page content as plain text to stdout.
    View {
        /// Page ID
        page_id: String,
    },
    /// Create a new page (opens $EDITOR with storage XML template).
    Create {
        #[arg(long)]
        space: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        parent: Option<String>,
    },
    /// Edit an existing page's storage XML in $EDITOR.
    Edit {
        /// Page ID
        page_id: String,
    },
}
// BlogArgs/BlogCommands mirrors PageArgs/PageCommands without --parent on Create
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Ratatui block::Title for right-aligned subtitles | `Line::right_aligned()` on a second title call | ratatui 0.30 (already adopted in Phase 2) | spaces.rs already uses the new API; pages.rs must follow same pattern |
| Single AppState enum for all screens | `Vec<Screen>` stack with per-screen state structs | Phase 3 (new) | This is the D-31 design decision |
| Hand-rolled XML regex stripping | quick-xml event reader | Phase 3 (new) | More correct; handles namespaces, CDATA, entities |

**Deprecated/outdated:**
- `block::Title` API: Phase 2 already adopted `Line::right_aligned()` / `Line::left_aligned()` — Phase 3 pages.rs must use the same approach confirmed in spaces.rs.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | quick-xml 0.39 uses `read_event_into(&mut buf)` API with `Event::Text(e).unescape()` for entity decoding | Pattern 7: strip_storage_xml | Code won't compile; fix by checking quick-xml 0.39 changelog/docs |
| A2 | Confluence DC 9.2.19 returns `displayName` inside `version.by` author object in content list response | Code Examples: serde structs | Deserialize silently returns None; preview pane shows "—" for author — tolerable degradation |
| A3 | The `Screen` enum with `PagesBrowseState` boxed inside the variant is the best ownership model | Pattern 1: Screen Stack | May need `Rc<RefCell<>>` or separate HashMap if mutable borrow conflicts arise with render |
| A4 | `ccli page list DEV` on TTY should push PagesBrowse and launch TUI (D-35) rather than always opening TUI from SpacesBrowse | Architecture Patterns | If wrong: user running `ccli page list DEV` would see SpacesBrowse first, not PagesBrowse |

---

## Open Questions

1. **quick-xml 0.39 exact API surface**
   - What we know: quick-xml 0.39.3 is current. Event-based reader confirmed as the right approach.
   - What's unclear: Whether `read_event_into` or `read_event` is the 0.39 method name; whether `.unescape()` on `Event::Text` handles `&amp;`, `&lt;`, `&gt;`, `&quot;` automatically.
   - Recommendation: The implementing plan should start with `cargo add quick-xml@0.39` and run `cargo doc --open` or check [docs.rs/quick-xml/0.39](https://docs.rs/quick-xml/0.39) before writing strip_storage_xml(). The logic is simple enough that verifying method names at implementation time costs less than speculating now.

2. **Screen stack ownership and borrow checker**
   - What we know: `App` will own `Vec<Screen>`; `PagesBrowseState` inside a Screen variant will be mutably borrowed by the event loop AND passed to render functions.
   - What's unclear: Whether the render function needs `&mut PagesBrowseState` (for `list_state`) and whether Rust's borrow checker allows this cleanly with the enum variant holding the state.
   - Recommendation: Box the state inside the variant (`Box<PagesBrowseState>`) and pass it as `&mut` to render. If borrow conflicts arise, move state out of the enum into a parallel `HashMap<usize, PagesBrowseState>` keyed by stack depth.

3. **`ccli page list DEV` TUI launch entry point**
   - What we know: D-35 says TTY → TUI pre-loaded to that space. The current `tui::run()` always starts on SpacesBrowse.
   - What's unclear: Whether `tui::run()` should accept an optional initial screen, or whether `handle_page_list` should construct and push a PagesBrowse screen before entering the TUI event loop.
   - Recommendation: Extend `tui::run(initial_screen: Option<Screen>)`. When `initial_screen` is `Some(PagesBrowse {...})`, push it onto the stack before the first draw. This keeps TUI startup logic in one place.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust / cargo | Build | ✓ | rustc 1.95.0 (2026-04-14) | — |
| quick-xml | strip_storage_xml() | Not yet in Cargo.toml | 0.39.3 (crates.io) | — (must add) |
| $EDITOR env var | D-41 editor launch | Runtime — varies | — | Fall back to $VISUAL then vi |
| /tmp writeable | Temp file for editor | ✓ (macOS/Linux std) | — | — |

[VERIFIED: Rust version via `rustc --version`; quick-xml version via `cargo search quick-xml`]

**Missing dependencies with no fallback:**
- `quick-xml` must be added to Cargo.toml. No fallback; regex approach is rejected (see Alternatives Considered).

---

## Security Domain

Security enforcement is not explicitly configured off — applying standard checks.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | PAT auth already in Client — no new auth surface |
| V3 Session Management | No | CLI / TUI — stateless request model |
| V4 Access Control | No | Confluence server enforces access; ccli passes PAT |
| V5 Input Validation | Yes | Page ID from CLI args; space key from CLI args |
| V6 Cryptography | No | TLS via reqwest; no custom crypto |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via page ID in temp file name | Tampering | Sanitize page ID: Confluence IDs are numeric strings — validate digits-only before constructing `/tmp/ccli-edit-{id}.xml` path |
| Shell injection via `$EDITOR` value | Elevation of privilege | Use `std::process::Command::new(editor).arg(path).status()` — NOT a shell exec. Never pass through `sh -c`. Already the planned approach (D-41). |
| PAT logged in debug output for new API calls | Information disclosure | Apply `#[instrument(skip(client))]` to all page API functions, same as space.rs |
| Temp file readable by other users | Information disclosure | Page content in /tmp may contain confidential page body. For v1 this is acceptable (same risk as editing in $EDITOR normally). Note in code comments. |

---

## Sources

### Primary (HIGH confidence)
- Cargo.toml in repo — verified all existing dependencies and versions
- `src/api/space.rs` — verified pagination pattern, serde struct conventions, error handling
- `src/tui/app.rs` — verified AppState, App struct, filter + debounce patterns
- `src/tui/mod.rs` — verified event loop, suspend/restore (D-23), channel patterns
- `src/tui/screens/spaces.rs` — verified render function signature, widget patterns
- `src/cli/mod.rs`, `src/cli/space.rs`, `src/cli/init.rs` — verified CLI structure, OutputFormatter, dialoguer usage
- `03-CONTEXT.md` — D-30 through D-44 locked decisions
- `03-UI-SPEC.md` — Screen stack contract, widget specs, keybinding table, strip_storage_xml rules

### Secondary (MEDIUM confidence)
- [developer.atlassian.com/server/confluence/confluence-rest-api-examples](https://developer.atlassian.com/server/confluence/confluence-rest-api-examples/) — POST/PUT body shape, required fields, version increment pattern
- [WebSearch: Confluence DC REST API 409 Conflict](https://community.atlassian.com/forums/Confluence-questions/HTTP-error-409-with-REST-API/qaq-p/797717) — confirmed 409 is stale version.number

### Tertiary (LOW confidence)
- `cargo search quick-xml` result: version 0.39.3 is current (HIGH — registry-verified)
- quick-xml 0.39 API method names (`read_event_into`, `unescape`) — ASSUMED from training knowledge; verify at implementation time

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all existing deps verified from Cargo.toml; quick-xml version verified from registry
- Architecture patterns: HIGH — all patterns have direct precedents in existing Phase 2 code
- API body shape: MEDIUM — verified from official Confluence REST API examples page; DC 9.2.19-specific behaviour not independently confirmed
- strip_storage_xml() API: MEDIUM — quick-xml version confirmed; exact method names in 0.39 are ASSUMED
- Pitfalls: HIGH — Pitfalls 1-3 from Phase 2 RESEARCH confirmed still apply; new pitfalls derived from locked decisions

**Research date:** 2026-05-05
**Valid until:** 2026-06-05 (stable stack; quick-xml version may update but 0.39.x patch releases are backward-compatible)
