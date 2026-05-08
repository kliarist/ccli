# ccli — Confluence CLI

## What This Is

A terminal-first CLI for Atlassian Confluence Data Center (targeting v9.2.19), written in Rust. Heavily inspired by [jira-cli](https://github.com/ankitpokhrel/jira-cli), it gives engineers a fast, keyboard-driven interface to browse, search, create, and edit Confluence content without ever leaving the terminal. Intended as an open source release for the Confluence community.

## Core Value

A developer can find, read, and edit any Confluence page from the terminal as fluidly as they browse code — with CQL-powered search and a jira-cli-style interactive TUI.

## Requirements

### Validated

**Authentication & Config (Phase 1)**
- [x] User can authenticate with a Confluence Data Center instance via Personal Access Token (PAT) — Validated in Phase 01: foundation
- [x] User runs `ccli init` to configure instance URL and token interactively on first run — Validated in Phase 01: foundation
- [x] Config is persisted to `~/.config/ccli/config.toml` — Validated in Phase 01: foundation

**Output & UX — Core (Phase 1)**
- [x] Non-interactive output defaults to formatted table (columns, human-readable) — Validated in Phase 01: foundation
- [x] `--plain` flag outputs tab-separated rows for piping — Validated in Phase 01: foundation
- [x] `--no-headers` flag suppresses column headers — Validated in Phase 01: foundation

### Active

**Authentication & Config**
*(moved to Validated above)*

**Spaces**
- [x] User can list all accessible spaces in a TUI list view — Validated in Phase 02: tui-shell-spaces
- [x] User can fuzzy-filter the space list by key or name — Validated in Phase 02: tui-shell-spaces
- [x] User can open a space in the browser from the TUI — Validated in Phase 02: tui-shell-spaces

**Pages**
- [x] User can list pages within a space in a TUI list + preview pane — Validated in Phase 03: pages-blog-posts
- [x] User can fuzzy-filter page lists in the TUI — Validated in Phase 03: pages-blog-posts
- [x] User can view the full page content (rendered as plain text or raw storage XML) — Validated in Phase 03: pages-blog-posts
- [x] User can create a new page by opening `$EDITOR` with a template — Validated in Phase 03: pages-blog-posts
- [x] User can edit an existing page's storage XML in `$EDITOR` — Validated in Phase 03: pages-blog-posts
- [x] User can open a page in the browser from the TUI — Validated in Phase 03: pages-blog-posts
- [x] User can search pages using CQL (Confluence Query Language) — Validated in Phase 04: comments-attachments-search

**Blog Posts**
- [x] User can list blog posts via `ccli blog list` — Validated in Phase 03: pages-blog-posts
- [x] User can create a new blog post via `$EDITOR` (`ccli blog create`) — Validated in Phase 03: pages-blog-posts
- [x] User can edit an existing blog post's storage XML via `$EDITOR` (`ccli blog edit`) — Validated in Phase 03: pages-blog-posts
- [x] User can open a blog post in the browser (`ccli blog view --open`) — Validated in Phase 03: pages-blog-posts

**Comments**
- [x] User can list comments on a page in the TUI — Validated in Phase 04: comments-attachments-search
- [x] User can add a new comment to a page from the terminal — Validated in Phase 04: comments-attachments-search

**Attachments**
- [x] User can list attachments on a page — Validated in Phase 04: comments-attachments-search
- [x] User can download an attachment to a local path — Validated in Phase 04: comments-attachments-search
- [x] User can upload a file as an attachment to a page — Validated in Phase 04: comments-attachments-search

**Output & UX**
- [x] TUI supports keyboard navigation: arrow keys, `/` to filter, `o` to open in browser, `q` to quit — Validated in Phase 02–04
*(Core output flags validated in Phase 01 — see Validated above)*

### Out of Scope

- Confluence Cloud API — targeting Data Center 9.2.19 only; Cloud has a different API surface
- Markdown-to-storage-format conversion — editing uses raw XML to avoid conversion edge cases
- OAuth 1.0a / basic auth — PAT is the recommended and sufficient auth method for DC 9.x
- Page tree management (move/reorder pages) — complex enough to defer post-v1
- Macros editor — Confluence macros are too complex for terminal editing in v1
- User/group management — not a page-workflow concern

## Context

- **Reference implementation:** [jira-cli](https://github.com/ankitpokhrel/jira-cli) — follow its UX patterns for TUI layout (list + preview pane), command structure (`ccli page list`, `ccli page view <ID>`), and output flags (`--plain`, `--no-headers`, `--columns`)
- **Target API:** Confluence Data Center REST API v1 and v2 (v9.2.19 supports both)
- **Content format:** Confluence Storage Format (XHTML-based) — pages are stored as structured XML; editing opens this directly in `$EDITOR`
- **TUI framework:** Likely [Ratatui](https://github.com/ratatui-org/ratatui) (Rust TUI library, successor to tui-rs) with async via Tokio
- **CQL:** Confluence Query Language is natively supported by the REST API — queries passed directly to `/rest/api/content/search`

## Constraints

- **Tech stack:** Rust — performance, single binary distribution, strong async ecosystem
- **Confluence version:** Data Center 9.2.19 — REST API v1 and v2, PAT authentication
- **Auth:** Personal Access Token only — avoids OAuth complexity, sufficient for DC 9.x target
- **Distribution:** Single binary, no runtime dependencies — installable via cargo or direct download
- **Editing:** Raw storage XML in `$EDITOR` — no conversion layer to maintain in v1

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust over Go | Single binary distribution, memory safety, performance — and differentiates from jira-cli (Go) | Confirmed — Phase 1 binary compiles to single static executable, 80 tests pass |
| PAT auth only | Simplest correct auth for DC 9.x; OAuth 1.0a is complex and rarely needed | Confirmed — Phase 1 PAT flow verified end-to-end |
| Raw XML editing | Markdown-to-storage conversion is lossy and complex to maintain for all macro types | Confirmed — Phase 3 editor workflow uses raw storage XML; quick-xml strips to plain text for preview |
| jira-cli UX model | Proven terminal UX for Atlassian tooling; users familiar with jira-cli will onboard instantly | Confirmed — Phase 2/3 TUI: list + preview pane, fuzzy filter, `o` browser-open, `e` editor |
| Binary name `ccli` | Short, memorable, unambiguous — mirrors `jira` from jira-cli | Confirmed — binary name set in Cargo.toml |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

## Current Milestone: v1.1 confluence-9213-compat

**Goal:** Audit and fix all REST API calls to work correctly against Confluence 9.2.13 (edition unconfirmed — standard Confluence instance).

**Target features:**
- API compatibility audit across all endpoints (spaces, pages, blog posts, comments, attachments, CQL search)
- Identify and fix any 9.2.13-specific response shape or behavior differences vs. the current 9.2.19 target
- Documented test matrix or integration tests verifying each endpoint against 9.2.13

---
## Current State

v1.0 milestone complete (4 phases, 21 plans, 235 tests passing). Starting v1.1 — compatibility audit against Confluence 9.2.13.

---
*Last updated: 2026-05-08 — Milestone v1.1 started: confluence-9213-compat*
