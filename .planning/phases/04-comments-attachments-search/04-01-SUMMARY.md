---
phase: "04"
plan: "01"
subsystem: api
tags: [rust, api, confluence, multipart, httpmock]
dependency_graph:
  requires: []
  provides: [comment-api, attachment-api, mime-detection]
  affects: [src/api/mod.rs, Cargo.toml]
tech_stack:
  added: [mime_guess@2, bytes@1, reqwest/multipart]
  patterns: [instrument-skip-client, query-builder-WR05, httpmock-unit-tests]
key_files:
  created:
    - src/api/comment.rs
    - src/api/attachment.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - src/api/mod.rs
decisions:
  - "Added bytes = \"1\" as explicit direct dependency (not importable as transitive dep)"
  - "Added reqwest multipart feature — needed for attachment upload (not in original Cargo.toml)"
  - "Used json_body_partial instead of body_includes in httpmock tests (body_includes not in httpmock 0.7)"
metrics:
  duration: "4 minutes"
  completed_date: "2026-05-07"
  tasks_completed: 3
  files_modified: 5
---

# Phase 04 Plan 01: Comments and Attachments API Layer Summary

**One-liner:** REST API client layer for page comments and attachments using reqwest multipart and mime_guess for Confluence DC.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add mime_guess dependency and api module exports | 246dd37 | Cargo.toml, Cargo.lock, src/api/mod.rs, src/api/comment.rs, src/api/attachment.rs |
| 2 | Implement src/api/comment.rs | c8c1f8b | src/api/comment.rs |
| 3 | Implement src/api/attachment.rs | 7fad9bd | src/api/attachment.rs, Cargo.toml, Cargo.lock |

## What Was Built

### src/api/comment.rs
- `Comment` struct with `CommentVersion`, `CommentAuthor`, `CommentBody`, `StorageBody`, `CommentLinks`
- `list_comments(client, page_id)` — GET `/rest/api/content/{id}/child/comment?expand=body.storage,version&limit=50`
- `add_comment(client, page_id, xml_body)` — POST storage XML wrapped comment body
- Both async fns carry `#[instrument(skip(client))]` (PAT never logged)
- Error dispatch: 401/403 → Auth, connect/timeout → Network, other → Api

### src/api/attachment.rs
- `Attachment` struct with `AttachmentExtensions`, `AttachmentVersion`, `AttachmentLinks`
- `list_attachments(client, page_id)` — GET `/rest/api/content/{id}/child/attachment?limit=50`
- `get_attachment(client, page_id, filename)` — list then download raw `bytes::Bytes`
- `add_attachment(client, page_id, file_path)` — multipart upload with `X-Atlassian-Token: no-check` header, MIME auto-detected via `mime_guess`
- All 3 async fns carry `#[instrument(skip(client))]`

### Cargo.toml Changes
- `mime_guess = "2"` — MIME type detection for attachment uploads (D-55)
- `bytes = "1"` — direct dep for `bytes::Bytes` return type in `get_attachment`
- `reqwest` multipart feature enabled — required for `reqwest::multipart::Form`

## Test Results

9 unit tests total, all passing:
- `api::comment::tests`: 4 tests (list_200, auth_401, add_body, add_403)
- `api::attachment::tests`: 5 tests (list_200, list_401, get_bytes, get_not_found, add_multipart_header)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Missing reqwest multipart feature**
- **Found during:** Task 3
- **Issue:** `reqwest::multipart` module not accessible without `multipart` feature in Cargo.toml
- **Fix:** Added `"multipart"` to reqwest features in Task 1
- **Files modified:** Cargo.toml
- **Commit:** 246dd37

**2. [Rule 3 - Blocking] bytes crate not importable as transitive dependency**
- **Found during:** Task 3
- **Issue:** `use bytes::Bytes` fails — Rust requires direct dependency to import crate
- **Fix:** Added `bytes = "1"` to Cargo.toml dependencies (per plan's own fallback instruction)
- **Files modified:** Cargo.toml
- **Commit:** 7fad9bd

**3. [Rule 1 - Bug] httpmock 0.7 has no body_includes method**
- **Found during:** Task 2
- **Issue:** Plan specified `.body_includes()` for httpmock assertions, but httpmock 0.7 only has `json_body_partial`
- **Fix:** Used `json_body_partial(r#"{"type":"comment"}"#)` which is the established pattern in page.rs tests
- **Files modified:** src/api/comment.rs
- **Commit:** c8c1f8b

## Known Stubs

None — all API functions are fully implemented with real HTTP calls.

## Threat Flags

No new threat surface beyond what was planned. All T-04-01 through T-04-07 mitigations implemented as specified.

## Self-Check: PASSED

- src/api/comment.rs: FOUND
- src/api/attachment.rs: FOUND
- 04-01-SUMMARY.md: FOUND
- Commit 246dd37: FOUND
- Commit c8c1f8b: FOUND
- Commit 7fad9bd: FOUND
