---
phase: "02"
plan: "01"
subsystem: api
tags: [spaces, api, pagination, serde, httpmock]
dependency_graph:
  requires: [src/api/client.rs, src/api/error.rs, src/config/mod.rs]
  provides: [src/api/space.rs, Space, SpaceDetail, list_all_spaces, get_space_detail, resolve_webui_url]
  affects: [src/api/mod.rs]
tech_stack:
  added: [ratatui 0.30, crossterm 0.29, nucleo-matcher 0.3, open 5]
  patterns: [paginated-api-walk, instrument-skip-token, httpmock-unit-tests, serde-rename-links]
key_files:
  created: [src/api/space.rs]
  modified: [Cargo.toml, Cargo.lock, src/api/mod.rs]
decisions:
  - "D-14: list_all_spaces pre-fetches all pages on TUI launch"
  - "D-17: ccli space list uses same paginated walk"
  - "D-18: get_space_detail fetches description.plain + homepage expand"
  - "D-19: debounce + cache types live here; logic deferred to tui/app.rs"
  - "D-24: resolve_webui_url joins base_url + relative _links.webui; never hand-build from key"
metrics:
  duration_minutes: 11
  completed_date: "2026-04-28"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 3
  tests_added: 10
  tests_total: 90
---

# Phase 2 Plan 01: Spaces API Foundation Summary

**One-liner:** Paginated Spaces API client with typed serde structs, #[instrument(skip(client))] token safety, and 10 httpmock unit tests — unblocks Plans 02 and 03.

## What Was Built

**Task 1: Phase 2 crates added to Cargo.toml**
- `ratatui = "0.30"` — TUI framework for subsequent plans
- `crossterm = "0.29"` — terminal backend
- `nucleo-matcher = "0.3"` — fuzzy filtering (Plans 03/04)
- `open = "5"` — browser launch (`o` keybind)
- `cargo build --release` exits 0; all 968 lock file changes are new crate fetches

**Task 2: src/api/space.rs implemented**
- 8 serde structs: `SpaceListResponse`, `SpaceListLinks`, `Space`, `SpaceLinks`, `SpaceDetail`, `SpaceDescription`, `PlainBody`, `SpaceHomepage`
- `list_all_spaces()`: walks all pages of GET /rest/api/space until `_links.next` is None or `size < limit` (Pitfall 7 guard), returns Vec<Space> sorted by key ascending
- `get_space_detail()`: GET /rest/api/space/{key}?expand=description.plain,homepage for preview pane data
- `resolve_webui_url()`: joins base_url + relative `_links.webui` path; never constructs from key (D-24, Pitfall 3)
- `#[instrument(skip(client))]` on all public async fns — PAT in Authorization header never appears in tracing output (T-02-01)
- `src/api/mod.rs` updated with `pub mod space;` and re-exports of all public types/functions

## Tests

10 new unit tests via httpmock; 0 regressions in existing 80 tests (90 total passing).

| Test | What it covers |
|------|---------------|
| `list_all_spaces_returns_sorted_vec_on_200` | Two unsorted results returned sorted by key |
| `list_all_spaces_returns_auth_error_on_401` | HTTP 401 → AppError::Auth |
| `list_all_spaces_returns_auth_error_on_403` | HTTP 403 → AppError::Auth |
| `list_all_spaces_returns_api_error_on_500` | HTTP 500 → AppError::Api |
| `list_all_spaces_paginates_until_no_next_link` | 25+2 items across two pages; page 2 has no _links.next |
| `get_space_detail_returns_detail_on_200` | Full SpaceDetail deserialized; description.plain checked |
| `get_space_detail_returns_auth_error_on_401` | HTTP 401 → AppError::Auth |
| `get_space_detail_returns_api_error_on_404` | HTTP 404 → AppError::Api |
| `resolve_webui_url_joins_base_and_relative_path` | Base + relative = absolute URL |
| `resolve_webui_url_strips_trailing_slash_from_base` | Trailing slash stripped before join |

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 | 1118fac | chore(02-01): add phase 2 crates to Cargo.toml |
| Task 2 | 37d41b8 | feat(02-01): implement src/api/space.rs — Space structs and API functions |

## Deviations from Plan

None — plan executed exactly as written.

Additional tests added beyond the 4 specified in the plan:
- `list_all_spaces_returns_auth_error_on_403` (403 is an Auth boundary alongside 401)
- `list_all_spaces_returns_api_error_on_500` (5xx error mapping coverage)
- `list_all_spaces_paginates_until_no_next_link` (pagination behavior — plan specified the behavior, test was implicit)
- `get_space_detail_returns_detail_on_200` (basic happy path for detail endpoint)
- `get_space_detail_returns_auth_error_on_401` (401 coverage for detail endpoint)
- `get_space_detail_returns_api_error_on_404` (404 coverage for detail endpoint)

These are Rule 2 additions (missing error-path coverage for correctness), not deviations.

## Threat Model Coverage

| Threat | Mitigation | Status |
|--------|------------|--------|
| T-02-01: Token in tracing | `#[instrument(skip(client))]` on both async fns; `debug!` logs URL only | Implemented |
| T-02-02: URL tampering | `webui` is `Option<String>` — missing = None, not empty string | Implemented |
| T-02-03: Build-time network | Pure-Rust crates accepted; no network at build time | Accepted |
| T-02-04: Pagination DoS | Loop terminates on `_links.next.is_none() || fetched < limit` | Implemented |

## Known Stubs

None — all functions are wired to the real Confluence REST API.

## Self-Check: PASSED

- [x] `src/api/space.rs` exists
- [x] `src/api/mod.rs` contains `pub mod space;`
- [x] Commit 1118fac exists (chore - Cargo.toml crates)
- [x] Commit 37d41b8 exists (feat - space.rs implementation)
- [x] 90 tests pass, 0 fail
- [x] `cargo build --release` exits 0
