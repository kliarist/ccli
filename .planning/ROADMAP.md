# Roadmap: ccli

## Overview

ccli is built in four coarse phases. Phase 1 lays the Rust project skeleton, API client, authentication, and output formatting so every later phase has a reliable foundation. Phase 2 constructs the Ratatui TUI shell with all keyboard navigation primitives and immediately validates it with the Spaces feature. Phase 3 delivers the core value proposition — browsing, viewing, creating, and editing Pages and Blog Posts. Phase 4 completes the v1 surface by adding Comments, Attachments, and CQL search.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Foundation** - Rust project skeleton, Confluence API client, PAT auth, `ccli init`, and output formatting flags
- [ ] **Phase 2: TUI Shell & Spaces** - Ratatui TUI with full keyboard navigation, delivered through the Spaces feature end-to-end
- [ ] **Phase 3: Pages & Blog Posts** - List, view, create, and edit Pages and Blog Posts — the core content workflow
- [ ] **Phase 4: Comments, Attachments & Search** - Page-level comments and attachments, plus CQL search across spaces

## Phase Details

### Phase 1: Foundation
**Goal**: The ccli binary exists, authenticates against a real Confluence DC instance, and all output formatting contracts work
**Depends on**: Nothing (first phase)
**Requirements**: INIT-01, INIT-02, INIT-03, INIT-04, OUT-01, OUT-02, OUT-03, OUT-04
**Success Criteria** (what must be TRUE):
  1. User can run `ccli init` and enter a Confluence URL and PAT interactively; config is saved to `~/.config/ccli/config.toml`
  2. User can override config by setting `CCLI_URL` and `CCLI_TOKEN` environment variables
  3. User sees a clear error with a remediation hint when credentials are missing or invalid
  4. Non-interactive output from any ccli command formats as an aligned table; `--plain` outputs tab-separated rows; `--no-headers` suppresses headers; `--columns` selects columns
**Plans**: TBD

### Phase 2: TUI Shell & Spaces
**Goal**: Users can navigate a full Ratatui TUI and use it to browse all accessible Confluence spaces
**Depends on**: Phase 1
**Requirements**: TUI-01, TUI-02, TUI-03, TUI-04, TUI-05, TUI-06, SPC-01, SPC-02, SPC-03, SPC-04
**Success Criteria** (what must be TRUE):
  1. User can run `ccli space list` and see all accessible spaces as a formatted table
  2. User can launch the TUI, navigate the space list with arrow keys and `j`/`k`, and see a preview pane alongside the list
  3. User can press `/` to fuzzy-filter the space list by key or name in real time
  4. User can press `o` on a space to open it in the system browser
  5. User can press `q` or `Esc` to quit the TUI, and a key-binding help bar is visible at the bottom
**Plans**: 5 plans
Plans:
- [x] 02-01-PLAN.md — Add Phase 2 crates (ratatui, crossterm, nucleo-matcher, open) and implement src/api/space.rs with pagination and preview fetch
- [ ] 02-02-PLAN.md — Wire non-interactive CLI: SpaceArgs/SpaceCommands in cli/mod.rs, ccli space list handler, main.rs Option<Commands> dispatch
- [ ] 02-03-PLAN.md — Implement src/tui/app.rs — App state machine, fuzzy filter (nucleo), debounce/cache, key handlers
- [ ] 02-04-PLAN.md — Implement src/tui/screens/spaces.rs — pure render function with all widget states
- [ ] 02-05-PLAN.md — Replace tui/mod.rs stub with complete event loop + human-verify checkpoint
**UI hint**: yes

### Phase 3: Pages & Blog Posts
**Goal**: Users can list, view, create, and edit Confluence Pages and Blog Posts entirely from the terminal
**Depends on**: Phase 2
**Requirements**: PAGE-01, PAGE-02, PAGE-03, PAGE-04, PAGE-05, PAGE-06, PAGE-07, PAGE-08, BLOG-01, BLOG-02, BLOG-03, BLOG-04
**Success Criteria** (what must be TRUE):
  1. User can run `ccli page list` or open a space in the TUI to see pages in a list + preview pane, filterable by title
  2. User can run `ccli page view <PAGE-ID>` to print page content to stdout
  3. User can run `ccli page create` to open `$EDITOR` with a storage XML template and publish the result as a new page
  4. User can run `ccli page edit <PAGE-ID>` to open the page's storage XML in `$EDITOR` and save changes back to Confluence
  5. User can list, create, edit, and open Blog Posts using `ccli blog` subcommands with the same TUI and editor experience as pages
**Plans**: TBD
**UI hint**: yes

### Phase 4: Comments, Attachments & Search
**Goal**: Users can manage page-level comments and attachments, and search pages with CQL from the terminal
**Depends on**: Phase 3
**Requirements**: CMNT-01, CMNT-02, ATTCH-01, ATTCH-02, ATTCH-03, PAGE-02
**Success Criteria** (what must be TRUE):
  1. User can run `ccli attachment list <PAGE-ID>` to list attachments, download one with `ccli attachment get`, and upload a file with `ccli attachment add`
  2. User can view all comments on a page in the TUI and add a new comment via `ccli comment add <PAGE-ID>` using `$EDITOR`
  3. User can run `ccli page search "<CQL>"` and receive a formatted list of matching pages
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation | 0/? | Not started | - |
| 2. TUI Shell & Spaces | 1/5 | In Progress|  |
| 3. Pages & Blog Posts | 0/? | Not started | - |
| 4. Comments, Attachments & Search | 0/? | Not started | - |
