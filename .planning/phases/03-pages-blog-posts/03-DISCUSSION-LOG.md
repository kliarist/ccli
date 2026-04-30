# Phase 3: Pages & Blog Posts - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-30
**Phase:** 03-pages-blog-posts
**Areas discussed:** TUI drill-down navigation, CLI page list scoping, Editor workflow for create/edit, Preview pane content

---

## TUI Drill-Down Navigation

| Option | Description | Selected |
|--------|-------------|----------|
| Drill into page list | Enter on space navigates to Pages screen — screen stack model | ✓ |
| CLI only, no TUI drill-down | Enter does nothing; pages only via ccli page list | |
| New TUI command, separate entry | ccli pages launches directly on a pages screen | |

**User's choice:** Drill into page list

---

| Option | Description | Selected |
|--------|-------------|----------|
| Enum-based screen stack | Vec<Screen> where Screen = SpacesBrowse \| PagesBrowse { space_key } | ✓ |
| Separate App structs | SpacesApp and PagesApp as separate structs with top-level RouterState | |

**User's choice:** Enum-based screen stack

---

| Option | Description | Selected |
|--------|-------------|----------|
| Open in browser (same as o) | Enter = browser open; e for editor; preview pane for content | ✓ |
| Open full-screen storage XML view | Enter opens scrollable full-screen XML view in TUI | |
| Launch $EDITOR immediately | Enter edits the page; o stays for browser | |

**User's choice:** Open in browser (same as `o`)

---

| Option | Description | Selected |
|--------|-------------|----------|
| Flat list of all space pages | All pages sorted by title, no hierarchy | ✓ |
| Hierarchical tree view | Pages indented by parent-child level, expandable/collapsible | |

**User's choice:** Flat list of all space pages

---

## CLI Page List Scoping

| Option | Description | Selected |
|--------|-------------|----------|
| Positional arg: ccli page list DEV | Space key is required positional argument | ✓ |
| Flag: ccli page list --space DEV | Space key as --space flag | |
| Optional: list all pages if no space given | No space = pages across all spaces via CQL | |

**User's choice:** Positional arg: `ccli page list DEV`

---

| Option | Description | Selected |
|--------|-------------|----------|
| TUI when TTY, table when piped | D-06 pattern: TTY → TUI, piped/--plain → table | ✓ |
| Always TUI | Always opens TUI regardless of TTY | |
| Always table | ccli page list always prints table; TUI only via drill-down | |

**User's choice:** TUI when TTY, table when piped

---

| Option | Description | Selected |
|--------|-------------|----------|
| Plain text (strip storage XML tags) | Strip tags, render paragraphs as text and headings as uppercase | ✓ |
| Raw storage XML | Print raw Confluence Storage Format XML directly | |

**User's choice:** Plain text (strip storage XML tags)

---

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, exact mirror | ccli blog mirrors ccli page; type=blogpost filter | ✓ |
| Blogs under space, no blog list command | Blog posts TUI only; no ccli blog list command | |
| Blogs have different scoping (no space key) | Blog list with no space = recent across all spaces | |

**User's choice:** Yes, exact mirror

---

## Editor Workflow for create/edit

| Option | Description | Selected |
|--------|-------------|----------|
| CLI flags + prompt for missing | --space, --title flags; missing values prompted via dialoguer; --parent optional | ✓ |
| Everything in the XML template | Open $EDITOR with placeholder comments; user fills template | |
| Interactive wizard before editor | Step-by-step dialoguer prompts before $EDITOR | |

**User's choice:** CLI flags + prompt for missing

---

| Option | Description | Selected |
|--------|-------------|----------|
| Show error, keep temp file | 409 → error message + preserve /tmp/ccli-edit-<ID>.xml | ✓ |
| Force overwrite (increment version) | Fetch current version, increment, retry PUT silently | |
| Retry loop with prompt | 409 → fetch latest, ask user to merge or overwrite | |

**User's choice:** Show error, keep temp file

---

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal boilerplate only | Just <p>Start writing here...</p> body | ✓ |
| Annotated template with common elements | Comments with heading/code/panel examples | |

**User's choice:** Minimal boilerplate only

---

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — e to edit selected page | Suspend TUI, open $EDITOR, restore TUI on exit | ✓ |
| No — edit via CLI only | ccli page edit <PAGE-ID> is the only edit entry point | |

**User's choice:** Yes — `e` keybinding in TUI to edit selected page

---

## Preview Pane Content

| Option | Description | Selected |
|--------|-------------|----------|
| Stripped plain text | Strip XML tags; headings as uppercase; shared with ccli page view | ✓ |
| Raw storage XML | Show raw Confluence Storage Format XML | |
| Metadata only (no body) | Title, author, last modified, version — no body content | |

**User's choice:** Stripped plain text

---

| Option | Description | Selected |
|--------|-------------|----------|
| Same D-19 debounce + cache pattern | 150ms settle, fetch ?expand=body.storage, cache by page ID | ✓ |
| Eager fetch (no debounce) | Fetch body immediately on selection change | |
| On-demand only (press p to load) | Empty preview until user presses p | |

**User's choice:** Same D-19 debounce + cache pattern

---

| Option | Description | Selected |
|--------|-------------|----------|
| Title only | Just page title in list pane; details in preview pane | ✓ |
| Title + last modified | Two-column: title and last modified date | |
| Title + author + status | Three-column: title, author, page status | |

**User's choice:** Title only

---

## Claude's Discretion

- API expand params for page list call: `?expand=version,ancestors` (enough for preview pane without a second request)
- Pagination strategy: walk all pages (same as D-14 for spaces)
- Module layout: `src/api/page.rs`, `src/cli/page.rs`, `src/cli/blog.rs`, `src/tui/screens/pages.rs`
- Plain text stripping implementation: `quick-xml` or `roxmltree` for tag removal
- Blog posts and pages share a single TUI screen (`PagesBrowse { space_key, content_type: ContentType }`)
- `$EDITOR` suspend/restore: `disable_raw_mode()` → write temp → launch editor → read file → PUT → `enable_raw_mode()`

## Deferred Ideas

- CQL search (`ccli page search`) — Phase 4
- Page version history view — post-v1
- Sub-page tree navigation — flat list sufficient for v1
- Clipboard support (`c`/`y`) — deferred from Phase 2, still deferred
- Markdown input for `ccli page create` — out of scope in v1 (PROJECT.md)
- Blog Posts section in space preview pane — small future iteration
