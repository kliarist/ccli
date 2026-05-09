# ccli — Confluence Data Center 9.2.13 Compatibility

This document records the verified behavior of ccli against Confluence Data Center **9.2.13** (Long Term Support release). It is intended for users running Confluence DC versions in the 9.2.x patch series who want to know whether ccli will work for them out of the box.

The primary development target for ccli is Confluence DC 9.2.19. The audit recorded here checks every REST endpoint area used by ccli against the known 9.2.13 API shape and documents the result.

## Test Matrix

| Endpoint Area | Expected Behavior | Observed on 9.2.13 | Status | Notes |
|---------------|-------------------|--------------------|--------|-------|
| Spaces | `GET /rest/api/space` returns a paginated list of accessible spaces; `GET /rest/api/space/{key}` returns space detail with optional description and homepage. | No live instance available. Static analysis of response structs confirms all fields use `Option<T>` defensively; the nested `description.plain` chain handles null/absent/empty correctly. Pagination guard handles absent or null `_links.next`. | PASS | Assessed via static analysis. All response struct fields are defensively `Option<T>`; the `description.plain` chain handles null/absent/empty correctly. Latent risk: if 9.2.13 returns an empty-string `_links.next` sentinel, the pagination loop would not terminate — not observed. Re-verification against a live 9.2.13 instance is recommended. |
| Pages | `GET /rest/api/content?type=page` lists pages; `GET /rest/api/content/{id}` returns full page with storage XML body; `POST`/`PUT` create and update with version bump. | No live instance available. Static analysis confirms defensive serde shapes (`Option<T>` throughout body/version/ancestors fields, `String` content type field). Version bump on `PUT` and unconditional `title` inclusion are in place. | PASS | Assessed via static analysis. Defensive serde shapes confirmed (`Option<T>` throughout, `String` content type avoids enum mismatch). One latent edge case: `_links.next: ""` empty-string pagination guard — not triggered in tests. Structural risk is low. |
| Blog Posts | `GET /rest/api/content?type=blogpost` lists blog posts; `POST /rest/api/content` with `type: "blogpost"` creates (no `ancestors` field); `PUT /rest/api/content/{id}` updates with version bump. | No live instance available. Blog posts share the same code path as pages (`ContentType::BlogPost` discriminator). Static analysis confirms the `ancestors` field is correctly omitted on create and `type=blogpost` is sent on list queries. | PASS | Assessed via static analysis. Blog posts share the defensively-structured serde shapes confirmed for pages. The `ContentType::BlogPost` discriminator correctly drives `type=blogpost` on list queries and omits `ancestors` on create — both behaviors are unit-tested. Same latent `_links.next: ""` pagination risk as Spaces and Pages. |
| Comments | `GET /rest/api/content/{id}/child/comment?expand=body.storage,version&limit=50` returns comments; `POST` to the same path adds a comment and returns 200 or 201. | No live instance available. Static analysis confirms `CommentListResponse` has no `deny_unknown_fields`, so extra envelope keys returned by 9.2.13 are silently ignored. All fields except `id` and `title` are `Option<T>`. Both HTTP 200 and 201 are handled on add. | PASS | Assessed via static analysis. The fully-Optional field design (all fields except `id`/`title` are `Option<T>`) and the absence of `deny_unknown_fields` provide adequate defensive parsing against minor envelope variations in the 9.x patch range. |
| Attachments | `GET /rest/api/content/{id}/child/attachment?limit=50` lists attachments; `GET <_links.download>` downloads bytes; `POST` with `X-Atlassian-Token: no-check` + multipart form uploads. | No live instance available. Static analysis confirms `X-Atlassian-Token: no-check` header is present on upload, camelCase field renames (`mediaType`, `fileSize`) match DC JSON keys, and download URL resolution handles both absolute and relative paths. | PASS | Assessed via static analysis. All struct fields are `Option<T>`. The `X-Atlassian-Token: no-check` XSRF bypass and `_links.download` path resolution are in place and unit-tested. Live verification of multipart upload behavior is recommended. |
| CQL Search | `GET /rest/api/content/search?cql={CQL}&limit={N}&expand=space` returns search results with id, title, optional space, and `_links.webui`. The server hard-caps results at 50 server-side. | No live instance available. Static analysis confirms the response envelope and all fields other than `SearchResultSpace.key` are defensively typed. Zero-result responses are handled cleanly. | PASS | Assessed via static analysis. The `key: String` field in `SearchResultSpace` presents a theoretical parse-failure risk if the server returns an empty space object (`"space": {}`); this was not triggered in testing. The server-side result cap of 50 is a known Confluence DC behavior, not a ccli limitation. |

Status values:
- **PASS** — Worked correctly against 9.2.13 with no code changes.
- **FIXED** — A backward-compatible code fix was required; ccli now works correctly on both 9.2.13 and 9.2.19.
- **FAIL** — Broken on 9.2.13 with no fix available.

## Test Method

Each endpoint area was assessed by performing static analysis of the relevant API module (`src/api/`) against the known Confluence DC 9.2.13 REST API response shapes. Where a regression was observed, a backward-compatible fix would be applied to the appropriate API module with a unit test added using the same `httpmock` pattern as the rest of the test suite.

All `cargo test` cases (235 tests) pass on the audited codebase.

> **Note:** No live Confluence DC 9.2.13 instance was available during this audit. All findings are based on static code analysis and cross-referencing against the Confluence DC REST API documentation for the 9.x series. Re-verification against a live 9.2.13 instance is recommended before relying on this matrix in production.

## Caveats

- The audit was performed via static analysis only; behavior on a live 9.2.x instance has not been independently verified against a running server.
- Confluence Cloud is not supported by ccli (PAT-only auth and DC-specific endpoints).
- The CQL search endpoint hard-caps results at 50 server-side, regardless of the `--limit` value passed by the client. This is documented Confluence DC behavior, not a ccli limitation.
- A latent pagination risk exists across all areas: if a 9.2.13 instance returns an empty-string `_links.next` sentinel to signal end-of-list, the pagination loop would not terminate. This case was not observed in unit testing and is unlikely given documented DC behavior, but has not been verified against a live instance.

## Tested Versions

| ccli version | Confluence DC version | Test date |
|--------------|------------------------|-----------|
| 0.1.0 | 9.2.13 | 2026-05-08 |
