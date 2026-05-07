---
phase: 04
plan: 04
subsystem: cli
tags: [cli, editor-workflow, multipart-upload, file-io, tdd]
dependency_graph:
  requires: [04-01, 04-02]
  provides: [cli/comment.rs, cli/attachment.rs]
  affects: []
tech_stack:
  added: []
  patterns: [editor-workflow, plain-text-to-xml-wrapping, human-readable-sizes, table-output]
key_files:
  created: []
  modified:
    - src/cli/comment.rs
    - src/cli/attachment.rs
    - src/cli/page.rs
decisions:
  - "D-52 wrap_plain_text_to_storage_xml: splits on blank lines, joins single-newline lines with space, escapes & → &amp; first"
  - "D-53 editor template is empty — user starts typing immediately"
  - "T-03-14 sanitize_id digits-only enforced on all three handlers in both files"
  - "T-03-EDITOR Command::new(editor).arg(path) — never via shell"
metrics:
  duration: "3 minutes"
  completed: "2026-05-07"
  tasks: 2
  files: 3
---

# Phase 04 Plan 04: CLI Comment & Attachment Handlers Summary

**One-liner:** Full $EDITOR + plain-text-to-XML comment workflow and three attachment CLI handlers (list/get/add) replacing the Plan 02 stubs, with 13 unit tests.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Implement src/cli/comment.rs (handle_add + wrap) | 646e739 | src/cli/comment.rs |
| 2 | Implement src/cli/attachment.rs (list, get, add) | b499827 | src/cli/attachment.rs, src/cli/page.rs |

## What Was Built

### Task 1 — src/cli/comment.rs

Replaced the Plan 02 stub with the full implementation:

- `handle_add`: validates page_id via `sanitize_id`, writes an empty /tmp file (D-53), launches `$EDITOR`, reads back content, wraps it via `wrap_plain_text_to_storage_xml`, POSTs to `api::comment::add_comment`, prints `Comment added to page {ID}.`
- `wrap_plain_text_to_storage_xml`: splits input into paragraphs on blank lines, joins single-newline lines with a space (D-52), escapes `&` first then `<` then `>` to prevent double-escaping
- `sanitize_id`: digits-only guard (T-03-14)
- `launch_editor`: `Command::new(editor).arg(path)` — never via shell (T-03-EDITOR)

7 unit tests cover: single paragraph, two paragraphs, extra blank lines, XML escaping, empty/whitespace input error, internal newline collapsing, path traversal rejection.

### Task 2 — src/cli/attachment.rs

Replaced the Plan 02 stub with the full implementation:

- `handle_list`: validates id, fetches via `api::attachment::list_attachments`, renders 4-column table via `OutputFormatter` (Filename, Size, Media-Type, Date) — ATTCH-01/D-56
- `handle_get`: downloads bytes via `api::attachment::get_attachment`, writes to `./FILENAME` by default or `--output <path>` override, prints `Downloaded: {f} → {dest}` — ATTCH-02/D-54
- `handle_add`: checks file exists, stats for size, calls `api::attachment::add_attachment`, prints `Uploaded: {f} ({size}) → page {id}` — ATTCH-03/D-55
- `human_size`: `<1024→N B`, `<1MiB→N KB`, `else→N MB` with round-to-nearest
- `build_list_rows`: pure helper for unit testability

6 unit tests cover: human_size byte/KB/MB formatting, rounding, sanitize_id rejection, 4-column row structure.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed unused `OutputConfig` import in src/cli/page.rs test module**
- **Found during:** Task 2 verification (`cargo clippy --all-targets -- -D warnings`)
- **Issue:** `use crate::output::OutputConfig;` was unused in `cli::page` test module, causing `error[unused-imports]` with `-D warnings`
- **Fix:** Removed the unused import from the test module in page.rs
- **Files modified:** src/cli/page.rs
- **Commit:** b499827 (included in Task 2 commit)

## Known Stubs

None — both files are fully implemented. No "not yet implemented" strings remain.

## Threat Surface Scan

No new network endpoints, auth paths, or schema changes introduced. All security mitigations from the threat register are applied:
- T-04-18: `sanitize_id()` digits-only on every CLI handler — verified by tests
- T-04-20: temp file cleanup on both success and empty-editor paths
- T-04-21: `$EDITOR` via `Command::new` not shell

## Self-Check: PASSED

Files exist:
- src/cli/comment.rs — FOUND
- src/cli/attachment.rs — FOUND

Commits exist:
- 646e739 — FOUND (feat(04-04): implement cli/comment.rs)
- b499827 — FOUND (feat(04-04): implement cli/attachment.rs)

Tests: 227 passed; 0 failed (13 new + 214 pre-existing)
Clippy: 0 warnings, 0 errors
