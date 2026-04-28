# Requirements: ccli

**Defined:** 2026-04-24
**Core Value:** A developer can find, read, and edit any Confluence page from the terminal as fluidly as they browse code — with CQL-powered search and a jira-cli-style interactive TUI.

## v1 Requirements

### Init & Configuration

- [ ] **INIT-01**: User can run `ccli init` to interactively configure the Confluence instance URL and Personal Access Token on first run
- [ ] **INIT-02**: Configuration is persisted to `~/.config/ccli/config.toml`
- [ ] **INIT-03**: User can override config values via environment variables (`CCLI_URL`, `CCLI_TOKEN`)
- [ ] **INIT-04**: User sees a clear error message with remediation hint when credentials are missing or invalid

### Spaces

- [x] **SPC-01**: User can list all accessible Confluence spaces in a TUI list view
- [ ] **SPC-02**: User can fuzzy-filter the space list by space key or name
- [ ] **SPC-03**: User can press `o` on a space to open it in the system browser
- [x] **SPC-04**: User can run `ccli space list` to output space list as a formatted table

### Pages

- [ ] **PAGE-01**: User can list pages within a space in a TUI list + preview pane
- [ ] **PAGE-02**: User can search pages using a CQL expression (`ccli page search "type=page AND space=DEV AND text~\"deploy\""`)
- [ ] **PAGE-03**: User can fuzzy-filter the page list in the TUI by title
- [ ] **PAGE-04**: User can view full page content (storage XML or plain-text rendering) in the TUI preview pane
- [ ] **PAGE-05**: User can create a new page by running `ccli page create`, which opens `$EDITOR` with a storage XML template
- [ ] **PAGE-06**: User can edit an existing page's storage XML by running `ccli page edit <PAGE-ID>`, which opens `$EDITOR`
- [ ] **PAGE-07**: User can press `o` on a page in the TUI to open it in the system browser
- [ ] **PAGE-08**: User can run `ccli page view <PAGE-ID>` to print page content to stdout

### Blog Posts

- [ ] **BLOG-01**: User can list blog posts in a space in a TUI list + preview pane
- [ ] **BLOG-02**: User can create a new blog post via `ccli blog create`, which opens `$EDITOR` with a storage XML template
- [ ] **BLOG-03**: User can edit an existing blog post's storage XML via `ccli blog edit <POST-ID>`
- [ ] **BLOG-04**: User can press `o` on a blog post in the TUI to open it in the system browser

### Comments

- [ ] **CMNT-01**: User can list all comments on a page in the TUI
- [ ] **CMNT-02**: User can add a page-level comment via `ccli comment add <PAGE-ID>` (opens `$EDITOR`)

### Attachments

- [ ] **ATTCH-01**: User can list all attachments on a page via `ccli attachment list <PAGE-ID>`
- [ ] **ATTCH-02**: User can download an attachment to a local path via `ccli attachment get <PAGE-ID> <FILENAME>`
- [ ] **ATTCH-03**: User can upload a local file as an attachment to a page via `ccli attachment add <PAGE-ID> <FILE-PATH>`

### TUI & Navigation

- [ ] **TUI-01**: TUI list pane supports arrow-key navigation and `j`/`k` vim bindings
- [ ] **TUI-02**: TUI supports `/` to activate fuzzy search/filter within a list
- [ ] **TUI-03**: TUI displays a content preview pane alongside the list pane (split view)
- [ ] **TUI-04**: TUI supports `o` keybinding to open the selected item in the system browser
- [ ] **TUI-05**: TUI supports `q` or `Esc` to quit or go back
- [ ] **TUI-06**: TUI displays a key-binding help bar at the bottom of the screen

### Output Formatting

- [ ] **OUT-01**: Non-interactive (piped) output defaults to a formatted table with aligned columns
- [ ] **OUT-02**: `--plain` flag outputs tab-separated values without box-drawing characters (for piping)
- [ ] **OUT-03**: `--no-headers` flag suppresses column header row
- [ ] **OUT-04**: `--columns` flag allows specifying which columns to include in output

## v2 Requirements

### Search & Query

- **SRCH-01**: User can save named CQL queries as aliases in config
- **SRCH-02**: User can search across all spaces simultaneously

### Page Management

- **PAGE-V2-01**: User can move a page to a different parent or space
- **PAGE-V2-02**: User can archive and restore pages
- **PAGE-V2-03**: User can view page version history

### Markdown Editing

- **EDIT-V2-01**: User can edit pages as Markdown with automatic Confluence Storage Format conversion

### Labels

- **LBL-01**: User can add/remove labels on pages
- **LBL-02**: User can list pages by label

### Notifications

- **NOTF-01**: User can watch/unwatch a page from the TUI

## Out of Scope

| Feature | Reason |
|---------|--------|
| Confluence Cloud API | Different API surface; targeting DC 9.2.19 only — Cloud support is a separate future effort |
| OAuth 1.0a / Basic auth | PAT is sufficient and recommended for DC 9.x; OAuth adds significant complexity for minimal gain |
| Markdown-to-storage-format conversion in v1 | Conversion is lossy for complex macros; raw XML avoids maintenance burden |
| Macro editor | Confluence macros are too complex for terminal editing |
| Page tree reordering / move | Complex tree operations; defer post-v1 |
| User & group management | Not a page-workflow concern |
| Real-time collaboration indicators | Requires WebSocket / long-polling; out of scope for CLI |
| Admin/space configuration | Space admin operations are too broad for v1 scope |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| INIT-01 | Phase 1 | Pending |
| INIT-02 | Phase 1 | Pending |
| INIT-03 | Phase 1 | Pending |
| INIT-04 | Phase 1 | Pending |
| OUT-01 | Phase 1 | Pending |
| OUT-02 | Phase 1 | Pending |
| OUT-03 | Phase 1 | Pending |
| OUT-04 | Phase 1 | Pending |
| TUI-01 | Phase 2 | Pending |
| TUI-02 | Phase 2 | Pending |
| TUI-03 | Phase 2 | Pending |
| TUI-04 | Phase 2 | Pending |
| TUI-05 | Phase 2 | Pending |
| TUI-06 | Phase 2 | Pending |
| SPC-01 | Phase 2 | Complete |
| SPC-02 | Phase 2 | Pending |
| SPC-03 | Phase 2 | Pending |
| SPC-04 | Phase 2 | Complete |
| PAGE-01 | Phase 3 | Pending |
| PAGE-02 | Phase 4 | Pending |
| PAGE-03 | Phase 3 | Pending |
| PAGE-04 | Phase 3 | Pending |
| PAGE-05 | Phase 3 | Pending |
| PAGE-06 | Phase 3 | Pending |
| PAGE-07 | Phase 3 | Pending |
| PAGE-08 | Phase 3 | Pending |
| BLOG-01 | Phase 3 | Pending |
| BLOG-02 | Phase 3 | Pending |
| BLOG-03 | Phase 3 | Pending |
| BLOG-04 | Phase 3 | Pending |
| CMNT-01 | Phase 4 | Pending |
| CMNT-02 | Phase 4 | Pending |
| ATTCH-01 | Phase 4 | Pending |
| ATTCH-02 | Phase 4 | Pending |
| ATTCH-03 | Phase 4 | Pending |

**Coverage:**
- v1 requirements: 35 total
- Mapped to phases: 35
- Unmapped: 0 ✓

---
*Requirements defined: 2026-04-24*
*Last updated: 2026-04-24 after roadmap creation*
