---
phase: 03-pages-blog-posts
plan: 01
subsystem: api
tags: [rust, reqwest, serde, httpmock, confluence-api, pagination]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: Config struct, AppError enum, Client with PAT auth header
  - phase: 02-tui-shell-spaces
    provides: Client::inner(), Client::base_url(), space.rs patterns to mirror

provides:
  - src/api/page.rs with list_all_pages, get_page_detail, create_page, update_page
  - ContentType enum (Page | BlogPost) with as_api_str() helper
  - Page, PageDetail, PageVersion, PageAuthor, PageAncestor, PageLinks, PageBody, StorageBody structs
  - ContentListResponse, ContentListLinks for paginated list responses
  - quick-xml = "0.39" dependency declared for Plan 02 XML stripper

affects:
  - 03-02-PLAN (XML stripper uses quick-xml added here)
  - 03-03-PLAN (CQL search calls same /rest/api/content endpoint)
  - 03-04-PLAN (TUI pages list calls list_all_pages; detail pane calls get_page_detail)
  - 03-05-PLAN (CLI page commands call create_page, update_page)

# Tech tracking
tech-stack:
  added:
    - quick-xml = "0.39" (XML parsing — used by Plan 02 strip_xml_tags)
  patterns:
    - "Paginated API walk: loop until _links.next absent OR size < limit (Pitfall 6)"
    - "ContentType enum as_api_str() drives type=page vs type=blogpost on every endpoint"
    - "Conditional ancestors field: omitted entirely for BlogPost (Pitfall 5)"
    - "#[allow(dead_code)] on API functions not yet wired to CLI (pre-Phase 3 CLI plans)"

key-files:
  created:
    - src/api/page.rs (385 lines: ContentType enum, 8 serde structs, 4 async fn, 16 httpmock tests)
  modified:
    - src/api/mod.rs (added pub mod page + full re-exports with #[allow(unused_imports)])
    - src/tui/mod.rs (Rule 3 fix: collapsed nested if-let per clippy::single_match)
    - Cargo.lock (updated after quick-xml added in Task 1)

key-decisions:
  - "D-33: list_all_pages sorts by title ascending (not insertion order from API)"
  - "D-37: ContentType enum encodes type=page vs type=blogpost; never use raw strings at call sites"
  - "D-39: 409 on update_page returns AppError::Api with message starting Conflict:"
  - "D-43: get_page_detail expands body.storage,version,ancestors in single request"
  - "Pitfall 1: update_page always sends current_version+1; caller provides current version"
  - "Pitfall 5: ancestors array is omitted entirely (not set to []) for blog posts"

patterns-established:
  - "API function pattern: #[allow(dead_code)] + #[instrument(skip(client))] on all public async fn"
  - "Error mapping: 401/403 -> AppError::Auth; other non-2xx -> AppError::Api; connect/timeout -> AppError::Network"
  - "Test helper: test_client(base_url) builds Client from Config with dummy token, mirrors space.rs"

requirements-completed: [PAGE-01, PAGE-04, PAGE-05, PAGE-06, PAGE-08, BLOG-01, BLOG-02, BLOG-03]

# Metrics
duration: 35min
completed: 2026-05-05
---

# Phase 3 Plan 01: Content API Foundation Summary

**Confluence Content API client with four async functions (list, detail, create, update) for pages and blog posts, 16 httpmock tests, and quick-xml dependency wired for Plan 02 XML stripping.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-05-05
- **Completed:** 2026-05-05
- **Tasks:** 2 (Task 1 was pre-completed; Task 2 executed in this session)
- **Files modified:** 4

## Accomplishments

- Four public async functions: list_all_pages (paginated), get_page_detail (body.storage expand), create_page (ancestors conditional on ContentType), update_page (version+1, 409 Conflict mapping)
- 16 httpmock unit tests covering sorted-200, 401/403/500 auth+api errors, multi-page pagination, blog-post ancestors omission (Pitfall 5), version-increment verification (Pitfall 1), and 409 Conflict message format
- quick-xml = "0.39" added to Cargo.toml enabling Plan 02 to implement the XML storage stripper
- All 124 project tests pass; cargo clippy -- -D warnings clean

## Task Commits

1. **Task 1: Add quick-xml dependency and serde structs** - `0d5e8ab` (feat)
2. **Task 2: Implement API functions and httpmock tests** - `b28013e` (feat)

## Files Created/Modified

- `src/api/page.rs` - ContentType enum, 8 serde structs, 4 async API functions, 16 httpmock tests (385 lines)
- `src/api/mod.rs` - pub mod page; re-exports with #[allow(unused_imports)]
- `src/tui/mod.rs` - Rule 3 fix: collapsed nested if-let (pre-existing clippy::single_match)
- `Cargo.lock` - Updated after quick-xml addition

## Decisions Made

- D-33: list_all_pages sorts results by title ascending after collecting all pages — matches plan locked decision, not delegated to caller
- D-39: 409 → `AppError::Api("Conflict: page was updated by someone else (version N).")` — specific message prefix for UI to detect
- Pitfall 5: blog post bodies are built without `ancestors` key entirely (not `ancestors: []`) — Confluence API behavior difference
- Added `#[allow(dead_code)]` on all 4 functions and ContentType enum since no CLI plan wires them yet; this mirrors the space.rs approach from Phase 2 (space functions ARE used by TUI, pages will be wired in Plans 04/05)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing clippy::single_match in src/tui/mod.rs**
- **Found during:** Task 2 verification (cargo clippy -- -D warnings)
- **Issue:** `match detail_result { Ok(detail) => ..., Err(_) => {} }` pattern at line 107 failed clippy::single_match
- **Fix:** Collapsed nested `if let Ok((key, detail_result))` + inner `if let Ok(detail)` into single `if let Ok((key, Ok(detail)))` pattern
- **Files modified:** src/tui/mod.rs
- **Verification:** cargo clippy -- -D warnings exits 0, all 124 tests still pass
- **Committed in:** b28013e (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 Rule 3 blocking)
**Impact on plan:** The pre-existing clippy issue was not introduced by this plan but blocked the clippy acceptance gate. Fix is behaviorally equivalent — preview errors remain silently swallowed (non-fatal per design comment). No scope creep.

## Issues Encountered

- `use serde::{Deserialize, Serialize};` in Task 1 structs included `Serialize` which was unused (no struct derives Serialize; serde_json::json! doesn't need it). Fixed by removing `Serialize` from the import before adding functions.
- `cargo test --lib api::page` fails with "no library targets" — project is binary-only. Used `cargo test` with grep filter instead; acceptance criteria adjusted accordingly.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 02 (XML stripper) can now use `quick-xml = "0.39"` and import `StorageBody` from `crate::api::page`
- Plan 03 (CQL search) can add a `search_pages` function following the same pagination pattern
- Plan 04 (TUI pages screen) has all structs and `list_all_pages` / `get_page_detail` ready to call
- Plan 05 (CLI page commands) has `create_page` and `update_page` ready
- No blockers. All 4 API functions compile clean, fully tested, and follow the space.rs discipline.

---
*Phase: 03-pages-blog-posts*
*Completed: 2026-05-05*
