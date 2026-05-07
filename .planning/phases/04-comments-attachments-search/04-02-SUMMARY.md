---
phase: 04
plan: 02
subsystem: cli
tags: [cli, clap, cql, search, output-formatter]
completed: "2026-05-07T19:06:35Z"

dependency_graph:
  requires: [04-01]
  provides: [Commands::Comment, Commands::Attachment, PageCommands::Search, handle_search, cli::comment::run stub, cli::attachment::run stub]
  affects: [src/main.rs, src/cli/mod.rs, src/cli/page.rs, src/cli/comment.rs, src/cli/attachment.rs]

tech_stack:
  added: []
  patterns:
    - CQL search via reqwest .query() builder for percent-safe encoding (WR-05)
    - Local SearchResponse/SearchResult structs for /rest/api/content/search response decoding
    - run_search test helper isolates HTTP logic from config loading for unit tests

key_files:
  created:
    - src/cli/comment.rs
    - src/cli/attachment.rs
  modified:
    - src/cli/mod.rs
    - src/cli/page.rs
    - src/main.rs

decisions:
  - handle_search uses local SearchResponse/SearchResult structs rather than reusing api/page.rs types, because the search endpoint returns a different shape than the content list
  - Test helper run_search() duplicates the HTTP/parsing logic inline rather than extracting a separate testable helper from handle_search, keeping handle_search signature stable for Plan 04
  - Stub files use anyhow::bail! to give a clear error string until Plan 04 implements real handlers

metrics:
  duration: 5min
  completed: "2026-05-07"
  tasks: 3
  files: 5
---

# Phase 4 Plan 2: CLI Dispatch Surface — Search, Comment & Attachment Summary

**One-liner:** CQL page search via `/rest/api/content/search` with 4-column table output, plus Comment/Attachment dispatch stubs wired end-to-end through main.rs.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Extend Commands enum and create stub modules | a6b1627 | src/cli/mod.rs, src/cli/comment.rs, src/cli/attachment.rs, src/cli/page.rs, src/main.rs |
| 2 | Add 5 unit tests for page search handler | 558bc6c | src/cli/page.rs |
| 3 | Wire main.rs dispatch (included in Task 1) | a6b1627 | src/main.rs |

## What Was Built

### src/cli/mod.rs
- Extended `Commands` enum with `Comment(CommentArgs)` and `Attachment(AttachmentArgs)` variants
- Extended `PageCommands` enum with `Search { cql: String, limit: u32 }` (default limit=25, D-46)
- Added `CommentArgs`, `CommentCommands` (with `Add { page_id }` subcommand, D-51..D-53)
- Added `AttachmentArgs`, `AttachmentCommands` (with `List`, `Get`, `Add` subcommands, D-54..D-56)
- Added `pub mod comment;` and `pub mod attachment;` module declarations

### src/cli/page.rs
- Added `handle_search(cli, cql, limit)` function hitting `/rest/api/content/search`
- Local `SearchResponse`, `SearchResult`, `SearchResultSpace`, `SearchResultLinks` structs
- Uses `.query()` builder for percent-safe CQL encoding (WR-05, T-04-08)
- 4-column table output: Title, ID, Space, URL (D-45, D-46)
- Auth error on 401/403, invalid CQL error on 400 (surfaces API body), generic error on other codes
- 5 new unit tests all passing, using `run_search()` test helper with httpmock

### src/cli/comment.rs (stub)
- `pub async fn run(_cli: &Cli, _args: &CommentArgs) -> anyhow::Result<()>` returning bail!
- Plan 04 replaces this wholesale

### src/cli/attachment.rs (stub)
- `pub async fn run(_cli: &Cli, _args: &AttachmentArgs) -> anyhow::Result<()>` returning bail!
- Plan 04 replaces this wholesale

### src/main.rs
- Added `Some(Commands::Comment(ref args)) => cli::comment::run(&cli, args).await,`
- Added `Some(Commands::Attachment(ref args)) => cli::attachment::run(&cli, args).await,`

## Deviations from Plan

### Auto-decisions (no deviations)

Tasks 1, 2, and 3 were merged into 2 commits because:
- Task 1 required adding the `PageCommands::Search` dispatch arm in `page.rs` AND the Comment/Attachment arms in `main.rs` to make `cargo build` compile (Rust exhaustive match requires all variants handled)
- Task 2 added the test suite
- Task 3 verified wiring already done in Task 1

The plan design assumed these could be separate compilable steps, but Rust's exhaustive match enforcement means all three changes are needed together for the build to pass. This was handled by committing all structural changes in one feat commit and the tests in a second test commit.

## Security Notes (Threat Model)

All STRIDE mitigations from the plan are implemented:
- **T-04-08 (CQL injection):** `.query(&[("cql", cql), ...])` builder used — reqwest percent-encodes the value. No string concatenation into URL.
- **T-04-09 (400 body disclosure):** 400 response body surfaced verbatim in `invalid CQL — {body}` message. User-facing only, no info-level logging.
- **T-04-10 (DoS via large limit):** Default 25, Confluence server enforces its own cap.
- **T-04-12 (PAT in logs):** No `info!`/`debug!` for request URL. PAT in headers only.

## Known Stubs

| File | Stub Type | Reason |
|------|-----------|--------|
| src/cli/comment.rs | `anyhow::bail!("not yet implemented")` | Plan 04 provides full implementation |
| src/cli/attachment.rs | `anyhow::bail!("not yet implemented")` | Plan 04 provides full implementation |

## Self-Check: PASSED

| Item | Status |
|------|--------|
| src/cli/mod.rs | FOUND |
| src/cli/page.rs | FOUND |
| src/cli/comment.rs | FOUND |
| src/cli/attachment.rs | FOUND |
| src/main.rs | FOUND |
| SUMMARY.md | FOUND |
| Commit a6b1627 | FOUND |
| Commit 558bc6c | FOUND |
