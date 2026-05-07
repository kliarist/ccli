---
phase: 03-pages-blog-posts
plan: 05
subsystem: cli
tags: [rust, clap, dialoguer, cli, pages, blog-posts, editor-workflow, path-traversal]

# Dependency graph
requires:
  - 03-01
    provides: list_all_pages, get_page_detail, create_page, update_page, ContentType, Page, PageDetail
  - 03-02
    provides: strip_storage_xml (used by handle_view_typed)

provides:
  - "src/cli/page.rs: PageArgs, PageCommands, run(cli, args) dispatching List/View/Create/Edit handlers"
  - "src/cli/blog.rs: BlogArgs, BlogCommands, run(cli, args) delegating to page handlers with ContentType::BlogPost"
  - "src/cli/mod.rs: Page(PageArgs) and Blog(BlogArgs) on Commands enum; pub mod page; pub mod blog;"
  - "src/main.rs: dispatch arms for Commands::Page and Commands::Blog"

affects:
  - 03-06
    note: Plan 06 wires the D-35 TTY-launch branch (handle_list_typed currently always prints table)

# Tech tracking
tech-stack:
  added: []  # all dependencies already in Cargo.toml
  patterns:
    - "sanitize_id(): digits-only validation before /tmp path construction (T-03-14 path traversal mitigation)"
    - "launch_editor(): Command::new(editor).arg(path) — never via shell (T-03-15 injection mitigation)"
    - "extract_space_key_from_webui(): parse /display/<KEY>/ from _links.webui to infer space key"
    - "handle_*_typed() pattern: typed helpers shared between page and blog dispatchers"
    - "D-39 conflict preservation: 409 returns Err but does NOT delete temp file"
    - "D-35 deferred: list handler always prints table; TTY TUI dispatch wired in Plan 06"

key-files:
  created:
    - src/cli/page.rs (run, handle_list_typed, handle_view_typed, handle_create_typed, handle_edit_typed, sanitize_id, extract_body_value, extract_space_key_from_webui, launch_editor + 9 unit tests)
    - src/cli/blog.rs (run delegating to page handlers with ContentType::BlogPost)
  modified:
    - src/cli/mod.rs (pub mod page; pub mod blog; + Page(PageArgs) + Blog(BlogArgs) on Commands enum + 11 clap tests)
    - src/main.rs (Commands::Page and Commands::Blog dispatch arms)

key-decisions:
  - "D-35 deferred to Plan 06: handle_list_typed always prints a table (correct when piped, deliberate gap on TTY)"
  - "sanitize_id() digits-only guard applied at handle_view_typed and handle_edit_typed entry"
  - "extract_space_key_from_webui() parses /display/<KEY>/ path segment — no separate API call for space key"
  - "Blog handlers pass parent=None unconditionally (Pitfall 5 / D-37); type system enforces no --parent flag"
  - "Task 1 stub arms in main.rs (anyhow error) allowed build to succeed while page/blog modules did not exist"

requirements-completed: [PAGE-05, PAGE-06, PAGE-07, PAGE-08, BLOG-02, BLOG-03, BLOG-04]

# Metrics
duration: ~25min
completed: 2026-05-07
---

# Phase 3 Plan 05: CLI page/blog subcommands Summary

**`ccli page` and `ccli blog` subcommands fully wired: clap parsing, dialoguer prompts, $EDITOR workflow, 409-conflict preservation, and path-traversal sanitization — closing Gap #1 from 03-VERIFICATION.md.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-05-07
- **Completed:** 2026-05-07
- **Tasks:** 2 (both TDD-tagged; unit tests written inline)
- **Files modified:** 4 (2 created, 2 extended)

## Accomplishments

- `src/cli/page.rs`: full async dispatch with `handle_list_typed`, `handle_view_typed`, `handle_create_typed`, `handle_edit_typed` — parameterized by `ContentType` so blog delegates without duplication
- `src/cli/blog.rs`: thin delegate to page handlers with `ContentType::BlogPost`; `parent` always `None`
- `src/cli/mod.rs`: `Page(PageArgs)` / `Blog(BlogArgs)` variants on `Commands` enum; `pub mod page; pub mod blog;`; 11 clap parsing tests including `blog_create_with_parent_flag_fails_pitfall5` (D-37)
- `src/main.rs`: two new dispatch arms replacing Task 1 stubs
- Security: `sanitize_id()` rejects non-digit IDs before `/tmp/ccli-edit-<ID>.xml` construction (T-03-14); `Command::new(editor).arg(path)` bypasses shell (T-03-15)
- 409 conflict: `AppError::Api("Conflict: ...")` matched by prefix, temp file preserved, path printed to stderr (D-39)
- 184 total tests pass; `cargo clippy -D warnings` clean

## Task Commits

1. **Task 1: CLI types + tests** - `65e8ae5` (feat) — Commands enum extended, 11 parsing tests, stub arms in main.rs
2. **Task 2: Handlers + dispatch** - `8d0bdf9` (feat) — page.rs, blog.rs created, real dispatch in main.rs, mod.rs uncommented

## Files Created/Modified

- `src/cli/page.rs` — run(), 4 typed handlers, 4 private helpers, 9 unit tests (~290 lines)
- `src/cli/blog.rs` — run() delegating to page handlers (~35 lines)
- `src/cli/mod.rs` — added 2 module decls, 2 Commands variants, 4 new types, 11 clap tests
- `src/main.rs` — 2 new dispatch arms (Commands::Page, Commands::Blog)

## Decisions Made

- **D-35 TTY launch deferred:** `handle_list_typed` always prints a table for now. TUI launch when TTY requires `tui::run()` signature change (Plan 06 territory). Annotated with `// D-35 TUI dispatch is wired in plan 03-06; Plan 05 always prints a table.`
- **Space key from `_links.webui`:** Rather than a second API call, `extract_space_key_from_webui()` parses the `/display/<KEY>/` path segment. This works for all standard Confluence page paths; the handler returns a clear error if the webui path has an unexpected shape.
- **Task 1 build stubs:** Per the plan's Step 5 guidance, Task 1 added `anyhow::anyhow!` stubs for `Commands::Page` and `Commands::Blog` in `main.rs` so the binary compiled while the handler modules did not yet exist. Task 2 replaced these with the real dispatch.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] main.rs non-exhaustive match after adding Commands::Page/Blog**
- **Found during:** Task 1 build
- **Issue:** Adding `Page(PageArgs)` and `Blog(BlogArgs)` to the `Commands` enum made the `match cli.command` in `main.rs` non-exhaustive; `cargo build` failed with E0004.
- **Fix:** Added temporary `anyhow::anyhow!` stub arms in Task 1 (per the plan's own Step 5 note), replaced with real dispatch in Task 2.
- **Files modified:** src/main.rs
- **Commit:** 65e8ae5 (Task 1)

---

**Total deviations:** 1 auto-fixed (Rule 3 — blocking build failure; fix was pre-documented in the plan)

## Known Stubs

- **D-35 TTY launch:** `handle_list_typed` always prints a table regardless of TTY state. When stdout is a TTY and `ccli page list <KEY>` is invoked, a table is printed instead of launching the TUI pages screen. This is intentional for Plan 05 scope; Plan 06 will add the TTY branch. Annotated in source with `// D-35 TUI dispatch is wired in plan 03-06`.

## Threat Surface

No new network endpoints or auth paths introduced. All threat mitigations in the plan's STRIDE register are implemented:

| Threat ID | Mitigation | Status |
|-----------|-----------|--------|
| T-03-14 | `sanitize_id()` rejects non-digit IDs | Implemented + tested (4 unit tests) |
| T-03-15 | `Command::new(editor).arg(path)` — no shell | Implemented, documented in code comment |
| T-03-16 | /tmp content disclosure — accepted risk | Documented |
| T-03-17 | PAT not logged (inherited from api/page.rs) | Verified — page handlers call API functions with `#[instrument(skip(client))]` |
| T-03-18 | strip_storage_xml panic-free (Plan 02) | Inherited |
| T-03-19 | 409 path leaks /tmp — accepted (D-39 contract) | Documented |

## Self-Check: PASSED

- FOUND: src/cli/page.rs
- FOUND: src/cli/blog.rs
- FOUND: src/cli/mod.rs (Page/Blog variants present)
- FOUND: src/main.rs (Commands::Page/Blog dispatch arms present)
- FOUND: .planning/phases/03-pages-blog-posts/03-05-SUMMARY.md
- FOUND: commit 65e8ae5 (Task 1 — feat)
- FOUND: commit 8d0bdf9 (Task 2 — feat)
- Tests: 184 passed, 0 failed
- Build: clean
- Clippy: clean (-D warnings)
