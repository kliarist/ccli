# COMPAT-05: Attachments — Findings

**Tested:** 2026-05-08
**Instance:** Confluence 9.2.13 (static analysis — no live instance available)
**Status:** PASS (UNVERIFIED — no live instance)

## Expected Behavior

`GET /rest/api/content/{id}/child/attachment?limit=50` lists attachments with `extensions.mediaType` and `extensions.fileSize`.
`GET <_links.download>` resolved against base_url downloads bytes.
`POST /rest/api/content/{id}/child/attachment` with `X-Atlassian-Token: no-check` + multipart form uploads.

## Sub-area Status

| Sub-area | Status | Notes |
| -------- | ------ | ----- |
| List (Step A) | PASS (UNVERIFIED) | Static analysis: `AttachmentExtensions` uses `#[serde(rename = "mediaType")]` and `#[serde(rename = "fileSize")]` — matches expected 9.2.13 JSON field names. All struct fields are `Option<T>`. |
| Download (Step B) | PASS (UNVERIFIED) | Static analysis: `get_attachment` resolves `_links.download` path against `client.base_url()` when the path starts with `/`; handles absolute URLs starting with `http` directly. No regression found. |
| Upload (Step C) | PASS (UNVERIFIED) | Static analysis: `add_attachment` correctly sends `.header("X-Atlassian-Token", "no-check")` per DC XSRF requirement (Pitfall 5 satisfied). Multipart part named `"file"` with filename and MIME type set. Header is confirmed present via existing unit test `add_attachment_sends_multipart_with_x_atlassian_token_header`. |
| Visible after upload (Step E) | PASS (UNVERIFIED) | Not testable without live instance; upload returns HTTP 200/201 on success. |

## Observed on 9.2.13

No live instance was available for testing. All observations are based on static code analysis of `src/api/attachment.rs` and cross-referencing against the known 9.2.13 REST API behaviour documented in `05-RESEARCH.md` and `05-PATTERNS.md`.

Key static observations:
- `X-Atlassian-Token: no-check` header is present on POST — correct per Confluence DC XSRF bypass requirement (PATTERNS.md Pitfall 5).
- `AttachmentExtensions` camelCase renames (`mediaType`, `fileSize`) match the JSON keys returned by Confluence DC `/rest/api/content/{id}/child/attachment`.
- `AttachmentLinks` fields are `Option<String>` — tolerates any missing link fields from 9.2.13 without panic.
- Download URL resolution handles both absolute (`http://...`) and relative (`/download/...`) paths.
- Upload uses `multipart::Part::bytes(file_bytes).file_name(filename).mime_str(...)` — standard `reqwest` multipart encoding compatible with Confluence DC.
- Response handling accepts both HTTP 200 and 201 on upload success — correct for DC which may return either.

## Fix Applied

None — Status is PASS (UNVERIFIED). No changes made to `src/api/attachment.rs`.

`X-Atlassian-Token: no-check` header preserved (not modified).

## Notes for COMPAT-9213.md

COMPAT-05 (Attachments): Static analysis indicates full compatibility with Confluence 9.2.13. No code changes needed. Live verification against a 9.2.13 instance is recommended to confirm multipart upload behaviour, particularly the `X-Atlassian-Token: no-check` XSRF bypass and `_links.download` path resolution.
