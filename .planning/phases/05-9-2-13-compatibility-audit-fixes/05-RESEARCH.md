# Phase 5: 9.2.13 Compatibility Audit & Fixes — Research

**Researched:** 2026-05-08
**Domain:** Confluence Data Center REST API v1 — compatibility testing against version 9.2.13 vs 9.2.19
**Confidence:** MEDIUM (no 9.2.13-specific REST API breaking changes documented in official sources; high confidence in existing codebase patterns)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-57:** Testing is against a live 9.2.13 instance — user provides URL/token externally. The plan must NOT include a "spin up Confluence" step; assume a running instance is available at execution time.
- **D-58:** Changelog/docs review first — the research step reads Confluence 9.2.13 release notes and REST API docs to pre-identify known differences vs. 9.2.19 before any live testing. This makes testing more targeted.
- **D-59:** Manual ccli runs + human observation — run each ccli command against the 9.2.13 instance, observe stdout output and errors, record findings by hand. No automated response-capture scripts.
- **D-60:** Written all at once after testing — test all 6 areas and apply fixes first; write COMPAT-9213.md as a final summary step.
- **D-61:** Format: test matrix with 6 rows — one row per endpoint area (maps 1:1 to COMPAT-01–06). Columns: `Endpoint Area | Expected Behavior | Observed on 9.2.13 | Status (PASS / FAIL / FIXED) | Notes`.
- **D-62:** Location: repo root, public-facing — committed to the repository root alongside CHANGELOG.md etc. Other Confluence DC users can reference it.
- **D-63:** Fix in-place in existing API modules — edit `src/api/space.rs`, `page.rs`, `comment.rs`, `attachment.rs`, `cli/page.rs` directly. No version detection, no compatibility shims. Fixes must be backward-compatible (should not break 9.2.19 behavior).
- **D-64:** Minimal scope — only fix what actually breaks on 9.2.13. No opportunistic refactoring or unrelated cleanup.
- **D-65:** Fix as you go — test an area → find issue → fix immediately → confirm it passes → move to next area. Do not batch all testing before fixing.
- **D-66:** Unit test per fix — for every code change, add at least one unit test mocking the 9.2.13-compatible response shape, using the existing `httpmock` pattern.
- **D-67:** Update existing tests to the compatible shape — when parsing logic changes, update existing mock responses so the same code/tests work for both 9.2.13 and 9.2.19. Do not maintain separate per-version test suites.
- **D-68:** Acceptance bar: all 235 existing tests pass + at least one new test per code fix.

### Claude's Discretion

- Specific response-shape differences between 9.2.13 and 9.2.19 are unknown until testing; the researcher will identify likely candidates from changelogs.
- Exact column widths and wording in COMPAT-9213.md rows can be determined during writing.
- Decision ID continuation: Phase 4 ended at D-56. Phase 5 starts at D-57.

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| COMPAT-01 | All spaces endpoints (`/rest/api/space`) return correct response shapes on Confluence 9.2.13 | Space struct fields documented; `description.plain` and `homepage` expand paths verified as standard; potential `_expandable` inconsistency flagged as high-probability difference area |
| COMPAT-02 | All pages endpoints (`/rest/api/content` — list, view, create, update) work correctly on Confluence 9.2.13 | `ContentListResponse`, `PageDetail`, `create_page`, `update_page` code paths all documented; `expand=body.storage,version,ancestors` verified as standard across all 9.2.x; version-number-bump for update is stable |
| COMPAT-03 | All blog-post endpoints (list, create, edit) work correctly on Confluence 9.2.13 | Blog posts share the same `/rest/api/content?type=blogpost` path; `ContentType::BlogPost` already coded; no blogpost-specific API changes found in docs |
| COMPAT-04 | Comments endpoints (list, add) work correctly on Confluence 9.2.13 | `list_comments` uses `expand=body.storage,version`; `add_comment` POSTs storage XML; both are stable REST API v1 operations with no documented 9.2.x changes |
| COMPAT-05 | Attachments endpoints (list, download, upload/multipart) work correctly on Confluence 9.2.13 | `X-Atlassian-Token: no-check` header requirement for multipart upload is a long-standing DC requirement; `_links.download` path resolution logic tested; no documented 9.2.x changes |
| COMPAT-06 | CQL search endpoint (`/rest/api/content/search`) returns correct results on Confluence 9.2.13 | `SearchResponse` parses only `results` array; extra fields (totalSize, cqlQuery, _links) are ignored by serde; rate-limiting note for 9.2.20+ does NOT apply to 9.2.13 |
| TMAT-01 | COMPAT-9213.md test matrix documents all 6 endpoint areas | Matrix format (6 rows, 5 columns) specified in D-61; location (repo root) in D-62; content is produced after testing, not before |
</phase_requirements>

---

## Summary

Phase 5 is a compatibility audit phase, not a feature phase. The primary challenge is behavioral: run the existing ccli codebase against a Confluence 9.2.13 instance, observe any breakage, and fix it minimally in the existing API modules. The secondary deliverable is COMPAT-9213.md, a 6-row public-facing test matrix.

**Critical finding from changelog research:** No REST API breaking changes were officially documented for the Confluence 9.2 patch series (9.2.0 through 9.2.19). The only notable REST API change in the entire 9.2 LTS lifecycle is in 9.2.20 (outside the tested range): `/rest/api/content` was removed from the rate-limit whitelist. This means all 6 endpoint areas are expected to work on 9.2.13 without code changes, but the audit must be performed because subtle response-shape differences (optional fields, `_expandable` behavior, `_links` structure) can still vary between minor versions without being documented as breaking changes. [CITED: https://confluence.atlassian.com/doc/confluence-9-2-release-notes-1456345480.html]

**Primary recommendation:** Structure the plan as one task per COMPAT requirement (6 audit+fix tasks) plus one final TMAT-01 task for writing COMPAT-9213.md. Each audit task follows the D-65 "test → fix → confirm" cycle with the httpmock pattern for any new unit tests.

The most likely areas to find differences are: `expand=` behavior for fields marked `_expandable` (optional fields that may not be present on older versions), the `_links.next` pagination sentinel value (some versions use `null` vs omission), and the `description.plain` nested struct for spaces (can be `{}` or `null` when no description is set).

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Spaces audit (COMPAT-01) | API / Backend (`src/api/space.rs`) | CLI (`src/cli/space.rs`) | All HTTP calls and struct parsing live in api/space.rs; cli/space.rs only formats output |
| Pages audit (COMPAT-02) | API / Backend (`src/api/page.rs`) | CLI (`src/cli/page.rs`) | All API calls in api/page.rs; cli/page.rs has CQL search logic inline but same HTTP tier |
| Blog posts audit (COMPAT-03) | API / Backend (`src/api/page.rs`) | CLI (`src/cli/blog.rs`) | Blog posts share page.rs module via ContentType::BlogPost |
| Comments audit (COMPAT-04) | API / Backend (`src/api/comment.rs`) | TUI (`src/tui/screens/comments.rs`) | comment.rs owns all HTTP; TUI reads the structs |
| Attachments audit (COMPAT-05) | API / Backend (`src/api/attachment.rs`) | CLI (`src/cli/attachment.rs`) | All HTTP including multipart upload in attachment.rs |
| CQL search audit (COMPAT-06) | CLI (`src/cli/page.rs`) | — | SearchResponse structs and HTTP call are defined inline in cli/page.rs (see `handle_search`) |
| COMPAT-9213.md artifact | Documentation | — | Repo-root markdown file written by implementer after all 6 areas are tested |

---

## Standard Stack

### Core — Existing (no new dependencies needed)

| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| reqwest | 0.13 | HTTP client for all Confluence REST calls | Already in Cargo.toml; PAT via default `Authorization: Bearer` header |
| serde / serde_json | 1 | JSON deserialization of API responses | All response structs use `#[derive(Deserialize)]` with `Option<T>` fields |
| httpmock | 0.7 (dev) | Unit test HTTP mocking | Established pattern in all api/* modules |
| tokio | 1 | Async runtime | Full features; used in all `#[tokio::test]` tests |

### No New Dependencies

This phase requires no new crate dependencies. All compatibility fixes apply to JSON parsing logic (serde structs) or response handling code (match arms), not to the HTTP transport layer. [VERIFIED: Cargo.toml read from source]

---

## Architecture Patterns

### System Architecture Diagram

```
ccli command
    │
    ▼
src/cli/*  (parse args, call API, format output)
    │  handle_list_typed / handle_search / etc.
    ▼
src/api/*  (HTTP calls + serde deserialization)
    │  list_all_spaces / list_all_pages / list_comments / etc.
    ▼
reqwest::Client (PAT: Authorization: Bearer <token>, default header)
    │
    ▼
Confluence DC 9.2.13  [LIVE INSTANCE — user-provided at test time]
    │
    └── /rest/api/space
    └── /rest/api/content
    └── /rest/api/content/{id}/child/comment
    └── /rest/api/content/{id}/child/attachment
    └── /rest/api/content/search
    └── /rest/api/user/current  (PAT probe)
```

**For unit tests the live instance is replaced by:**

```
httpmock::MockServer  (inline in #[cfg(test)] blocks)
    │
    └── mock JSON responses representing 9.2.13 response shapes
```

### Recommended File Targets for This Phase

```
src/
├── api/
│   ├── space.rs        # COMPAT-01 — may need _expandable or description.plain fix
│   ├── page.rs         # COMPAT-02/03 — may need expand or _links fix
│   ├── comment.rs      # COMPAT-04 — stable, minimal risk
│   └── attachment.rs   # COMPAT-05 — multipart upload requires X-Atlassian-Token header
└── cli/
    └── page.rs         # COMPAT-06 — SearchResponse inline struct
COMPAT-9213.md          # TMAT-01 — repo root, written last
```

### Pattern 1: httpmock Unit Test (Established Pattern — Use Exactly)

Every new test for a compatibility fix MUST use this pattern. Do not deviate. [VERIFIED: read from src/api/space.rs, src/api/page.rs, src/api/comment.rs, src/api/attachment.rs, src/cli/page.rs]

```rust
// Source: read from src/api/space.rs #[cfg(test)] mod tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use httpmock::prelude::*;

    fn test_client(base_url: &str) -> Client {
        Client::new(&Config {
            url: base_url.to_string(),
            token: "AT-test-token".to_string(),
        })
        .expect("client")
    }

    #[tokio::test]
    async fn <test_name_describing_9213_shape>() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/rest/api/<endpoint>");
            then.status(200).json_body(serde_json::json!({
                // Use the 9.2.13-compatible response shape here
            }));
        });
        let client = test_client(&server.base_url());
        let result = <api_function>(&client, ...).await.expect("ok");
        // assertions
    }
}
```

### Pattern 2: Serde Option Fields for Defensive Parsing

All API response structs already use `Option<T>` for non-required fields. When a 9.2.13 response omits a field that 9.2.19 includes, serde deserializes it as `None` — no parse failure. New structs for any 9.2.13 fix must follow the same pattern. [VERIFIED: read from all api/*.rs source files]

```rust
// Source: src/api/space.rs
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpaceListLinks {
    pub next: Option<String>,  // Option prevents parse failure when absent
    pub base: Option<String>,
}
```

### Pattern 3: Fix-and-Update-Existing-Tests (D-67)

When a serde struct field type or shape changes (e.g., a field that was `String` needs to become `Option<String>`, or a mock response needs updating), **update ALL existing tests** that use the old mock shape, not just the new test. The goal is one unified response shape that works for both versions.

```rust
// BAD: Adding a separate 9.2.13 test while the existing test uses a different shape
// GOOD: Update the existing test mock to the compatible shape, add a test for the specific
//       behavior that was broken on 9.2.13
```

### Pattern 4: COMPAT-9213.md Table Format (D-61)

```markdown
| Endpoint Area | Expected Behavior | Observed on 9.2.13 | Status | Notes |
|---------------|-------------------|-------------------|--------|-------|
| Spaces        | ...               | ...               | PASS   | ...   |
| Pages         | ...               | ...               | FIXED  | ...   |
| Blog Posts    | ...               | ...               | PASS   | ...   |
| Comments      | ...               | ...               | PASS   | ...   |
| Attachments   | ...               | ...               | PASS   | ...   |
| CQL Search    | ...               | ...               | PASS   | ...   |
```

Status values: `PASS` (worked as-is), `FIXED` (code change was needed), `FAIL` (broken, no fix applied — should not occur given D-65).

### Anti-Patterns to Avoid

- **Version detection shims:** Never add `if version < "9.2.19"` branching — explicitly out of scope (CONTEXT.md domain section).
- **Separate test suites per version:** Never add a `#[cfg(test)] mod tests_9213` — D-67 says update existing tests.
- **Creating a test HTTP script:** D-59 specifies manual ccli runs + human observation. No automated curl/Python scripts.
- **Batching all testing before fixing:** D-65 is explicit — fix as you go.
- **Touching TUI or CLI output formatting unless a struct field changed:** D-63 scope is api/*.rs and cli/page.rs. TUI screens are downstream consumers of structs; change them only if a field that TUI reads was renamed or removed.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP request mocking in tests | Custom test server | `httpmock::MockServer` | Already established in all API test modules |
| JSON response parsing | Custom parser | `serde_json` + `#[derive(Deserialize)]` | All existing structs use this; Option<T> handles missing fields |
| Test client construction | Duplicated config setup | `test_client(base_url)` helper (already in each module) | Consistent, DRY — copy the local helper, do not add a shared one |
| Version-specific behavior branching | `if version < X` | Update struct fields to `Option<T>` + serde defaults | Backward-compatible, no version detection needed |

---

## Known Differences to Probe (Pre-Audit Research)

This section documents the most likely areas of difference between 9.2.13 and 9.2.19 based on changelog research and structural analysis of the codebase. These are **hypotheses to test**, not confirmed bugs.

### High-Probability Difference Areas

**1. `expand=` behavior and `_expandable` fields**

The codebase uses expand parameters for several calls:
- `GET /rest/api/space/{key}?expand=description.plain,homepage` — if 9.2.13 returns `description` as `null` or `{}` instead of `{"plain": {...}}`, the serde parse succeeds (Option fields) but the detail view may show empty description.
- `GET /rest/api/content/{id}?expand=body.storage,version,ancestors` — `ancestors` may be absent or `[]` on older versions.
- `GET /rest/api/content?expand=version,ancestors` — same concern.

[ASSUMED] Minor versions sometimes change which fields are populated by default in expandable containers. The existing `Option<T>` struct fields protect against hard failures but may silently produce empty UI output.

**2. `_links.next` pagination sentinel**

The existing code checks `page.links.next.is_some()` to continue pagination. Some Confluence versions return `_links: {"next": null}` (which serde deserializes as `None`) rather than omitting the key entirely. Both produce `None` with the current struct definition, so this should be safe. However, if a version uses an empty string `""` for `next`, `is_some()` returns `true` and causes an infinite pagination loop.

[ASSUMED] This is a low-probability issue for the 9.2.x series, but worth verifying.

**3. `AttachmentExtensions` field names**

The `extensions` object has `mediaType` and `fileSize` (camelCase). In very early Confluence versions these were different names. For 9.2.13 these are expected to be stable. [ASSUMED]

**4. `CommentListResponse` — presence of pagination envelope**

`list_comments` parses a response with `results` key but does NOT have `start`/`limit`/`size` in the `CommentListResponse` struct. If the actual API response includes those fields, serde ignores them (no `deny_unknown_fields`). If the API response does NOT include a `results` key (unusual but possible with certain error responses), the parse fails. [ASSUMED — low probability]

**5. `SearchResponse` minimal shape**

`handle_search` uses a minimal `SearchResponse { results: Vec<SearchResult> }`. The actual Confluence DC search response includes additional fields: `totalSize`, `cqlQuery`, `searchDuration`, `_links`. These are harmlessly ignored by serde since `deny_unknown_fields` is not set. This should be stable. [ASSUMED]

**6. `X-Atlassian-Token: no-check` header for attachment upload**

This header is required for multipart form upload on all Confluence DC versions to bypass XSRF protection. It is correctly implemented in `add_attachment`. If 9.2.13 returns 403 with a body mentioning "xsrf" or "token", the header is not being sent. This is the most likely place to see a real breakage if anything is wrong. [CITED: src/api/attachment.rs — D-55 comment]

### What the Official Changelog Says

Based on research of:
- Confluence 9.2 release notes [CITED: https://confluence.atlassian.com/doc/confluence-9-2-release-notes-1456345480.html]
- Confluence 9.2 Long Term Support Release Change Log [CITED: https://confluence.atlassian.com/doc/confluence-9-2-long-term-support-release-change-log-1472432426.html]
- Confluence 9.2 upgrade notes [CITED: https://confluence.atlassian.com/doc/confluence-9-2-upgrade-notes-1456345485.html]
- Confluence Data Center changelog [CITED: https://developer.atlassian.com/server/confluence/changelog/]

**Result: No REST API breaking changes are documented for versions 9.2.0 through 9.2.19.** The only documented REST API behavior change in the entire 9.2 LTS cycle is in **9.2.20** (outside the test range): `/rest/api/content` was removed from the default rate-limit whitelist. This change does NOT affect 9.2.13.

**Important caveat:** The Atlassian documentation pages use JavaScript-rendered issue tables (showing "Loading…" placeholders) that are not readable by automated fetch. Individual patch release notes (9.2.13 specifically) could not be fully read. The absence of documented breaking changes is therefore a confident finding for the major API surface, but individual bug fixes in 9.2.x patches could have subtly changed response structures without being labeled as breaking. [CITED: fetched pages returned "Loading..." for issue tables]

---

## Common Pitfalls

### Pitfall 1: Treating absence of documented changes as confirmed compatibility

**What goes wrong:** Assuming "nothing documented" means "everything works" and skipping the live test for a particular endpoint area.

**Why it happens:** No official REST API breaking changes for 9.2.13 → 9.2.19 were found. But patch notes for specific bug fixes (e.g., a fix that changes a field name) are in JS-loaded tables that couldn't be inspected.

**How to avoid:** D-58 requires changelog review (done here) AND live testing. Both steps are required. The research only narrows down which areas to test more carefully.

**Warning signs:** If an area appears to "work" with minimal output (e.g., list returns 0 items), don't mark it PASS — verify data is actually being returned.

### Pitfall 2: Serde parse errors from unexpected fields

**What goes wrong:** A response struct without `#[serde(deny_unknown_fields)]` will silently ignore extra fields, but if a required field (non-Option) is absent, the parse fails with an error that surfaces as `AppError::Api("Failed to parse ...")`.

**Why it happens:** Confluence may return a slightly different JSON shape for a field the code expects as non-optional.

**How to avoid:** When a parse error occurs during testing, inspect the raw JSON response (add a debug log or capture with curl). Then add `Option<T>` to the offending field and add a unit test with the observed shape.

**Warning signs:** `"Failed to parse <X> list: missing field '...' at line N"` in test output or stdout.

### Pitfall 3: Pagination loop from non-null `_links.next` on last page

**What goes wrong:** If 9.2.13 returns `"_links": {"next": ""}` (empty string) on the final page rather than omitting the key, the `is_some()` check triggers another request that returns an empty list, causing a loop that terminates only by `size < limit` check.

**Why it happens:** Different minor versions have behaved differently about whether to omit `_links.next` or return it as null or empty string.

**How to avoid:** When testing list endpoints, use a space with enough content to trigger pagination (or set `limit=1` in a dev-only test). Verify the loop terminates correctly.

**Warning signs:** `ccli space list` or `ccli page list` hangs or makes excessive requests.

### Pitfall 4: TUI update requirement when a struct field changes

**What goes wrong:** A fix to a serde struct (e.g., changing a field from `String` to `Option<String>`, or from `Option<String>` to `Option<NestedStruct>`) silently compiles but breaks TUI rendering if the TUI screen reads that field directly.

**Why it happens:** `src/tui/screens/spaces.rs`, `pages.rs`, `comments.rs` all reference fields from the API structs. A type change requires updating the screen code too.

**How to avoid:** After any struct change, `cargo build` and fix all compile errors. The compiler will find all usages.

**Warning signs:** Compile error referencing `src/tui/screens/*` after a struct change.

### Pitfall 5: `X-Atlassian-Token` header already correct — don't touch it

**What goes wrong:** Trying to "fix" the multipart upload header when it already works.

**Why it happens:** The header `X-Atlassian-Token: no-check` is already implemented in `add_attachment`. If attachment upload fails on 9.2.13, the failure is more likely due to content-type or request body structure, NOT the XSRF token header.

**How to avoid:** Test `ccli attachment add` first. If it returns 403, read the response body carefully. The code is already correct for the XSRF requirement.

### Pitfall 6: CQL search limit hard-cap at 50

**What goes wrong:** Requests to `/rest/api/content/search` with `limit > 50` are silently capped at 50 by the Confluence DC server, regardless of what limit is specified.

**Why it happens:** Known Confluence DC server behavior documented in official support KB. [CITED: https://support.atlassian.com/confluence/kb/searching-for-content-with-the-rest-api-and-cql-always-limits-results-to-50/]

**How to avoid:** The current default limit of 25 in `handle_search` is already below 50. This is not a bug, but document it in COMPAT-9213.md notes column if observed.

---

## Code Examples

### Current Endpoint Usage Summary

| Module | Endpoint | Method | Key Query Params |
|--------|----------|--------|-----------------|
| `space.rs` `list_all_spaces` | `/rest/api/space` | GET | `start`, `limit` |
| `space.rs` `get_space_detail` | `/rest/api/space/{key}` | GET | `expand=description.plain,homepage` |
| `page.rs` `list_all_pages` | `/rest/api/content` | GET | `spaceKey`, `type`, `start`, `limit`, `expand=version,ancestors` |
| `page.rs` `get_page_detail` | `/rest/api/content/{id}` | GET | `expand=body.storage,version,ancestors` |
| `page.rs` `create_page` | `/rest/api/content` | POST | body: type, title, space.key, body.storage, ancestors? |
| `page.rs` `update_page` | `/rest/api/content/{id}` | PUT | body: id, type, title, space.key, version.number, body.storage |
| `comment.rs` `list_comments` | `/rest/api/content/{id}/child/comment` | GET | `expand=body.storage,version`, `limit=50` |
| `comment.rs` `add_comment` | `/rest/api/content/{id}/child/comment` | POST | body: type=comment, body.storage |
| `attachment.rs` `list_attachments` | `/rest/api/content/{id}/child/attachment` | GET | `limit=50` |
| `attachment.rs` `get_attachment` | `_links.download` path | GET | — (path from list response) |
| `attachment.rs` `add_attachment` | `/rest/api/content/{id}/child/attachment` | POST | multipart, `X-Atlassian-Token: no-check` |
| `cli/page.rs` `handle_search` | `/rest/api/content/search` | GET | `cql`, `limit`, `expand=space` |

[VERIFIED: read from all API source files]

### PAT Authentication (stable across all 9.2.x)

```rust
// Source: src/api/client.rs build_reqwest_client()
// PAT is injected as a default header on the reqwest::Client at construction time.
// Every subsequent request carries it automatically.
let auth_value = header::HeaderValue::from_str(&format!("Bearer {}", token))?;
headers.insert(header::AUTHORIZATION, auth_value);
```

The `Authorization: Bearer <token>` header format for Confluence DC PAT is stable and was not changed in any 9.2.x version. [ASSUMED — no documented changes found; standard DC behavior confirmed via multiple community sources]

### Minimal Fix Pattern (When a Field Needs to be Made Optional)

```rust
// BEFORE (assumes field always present):
pub struct SpaceSomething {
    pub my_field: String,
}

// AFTER (defensive — handles 9.2.13 where field may be absent):
#[derive(Debug, Clone, Deserialize)]
pub struct SpaceSomething {
    #[serde(default)]
    pub my_field: Option<String>,
}

// Update all call sites that dereference my_field as String to handle Option<String>
```

### Test Update Pattern (D-67)

When an existing test's mock response is updated to a more defensive shape, the assertion must still pass:

```rust
// BEFORE (field was required String in struct):
then.status(200).json_body(serde_json::json!({
    "my_field": "value",
    // ...
}));

// AFTER (field is now Option<String>, omit it in some tests to cover the absent case):
then.status(200).json_body(serde_json::json!({
    // my_field intentionally omitted — tests None path
    // ...
}));
// Or keep it present in tests that test the Some(_) path
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Basic auth (username:password) | PAT via `Authorization: Bearer` | Confluence DC 7.9+ | PAT is the correct auth for 9.2.x; no change needed |
| `wiki/rest/api/` prefix (Server legacy) | `/rest/api/` | Pre-8.x | ccli already uses `/rest/api/` — correct |
| Separate Cloud and DC APIs | DC uses REST API v1 (`/rest/api/`) throughout 9.2.x | N/A for DC | v2 API exists in DC 9.x but ccli uses v1 exclusively — v1 is stable |
| Rate limiting exemption for `/rest/api/content` | Removed from whitelist in 9.2.20 | 9.2.20 (after test range) | Does NOT affect 9.2.13 testing |

**Deprecated/outdated:**
- Confluence Cloud REST API v1: Being deprecated in Cloud (RFC-19), but this is irrelevant — ccli targets Data Center only.
- `wiki/rest/api/` prefix: Legacy Server prefix, not used by ccli.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `expand=` behavior for optional fields is consistent across 9.2.13 and 9.2.19 — `description.plain`, `homepage`, `ancestors` will be present when requested | Known Differences, COMPAT-01/02 | Parser produces empty output silently; unit test using the 9.2.13 shape would catch it |
| A2 | `_links.next` is either absent or `null` (not empty string `""`) on the final page in 9.2.13 | Known Differences — Pagination | Infinite pagination loop; would be caught by the live test hanging |
| A3 | `AttachmentExtensions` field names (`mediaType`, `fileSize`) are stable camelCase in 9.2.13 | Known Differences | `list_attachments` would parse successfully but file-size column would show empty |
| A4 | `Authorization: Bearer` PAT format is unchanged across 9.2.13 → 9.2.19 | PAT Authentication | All 6 endpoint areas would fail with 401; immediately visible in live testing |
| A5 | No new mandatory request headers were added to any endpoint between 9.2.13 and 9.2.19 | All endpoint areas | Specific endpoint would return 400/403; immediately visible in live testing |
| A6 | Rate limiting does not affect 9.2.13 testing (rate limiting change was in 9.2.20) | State of the Art | If rate limiting was somehow enabled earlier, certain endpoints might be throttled |

---

## Open Questions (RESOLVED)

1. **What does the 9.2.13 `description.plain` response actually look like for a space with no description set?**
   - What we know: The struct has `description: Option<SpaceDescription>` and `plain: Option<PlainBody>` — nested Options handle null/absent
   - What's unclear: Whether the key is present as `{}`, `null`, or absent entirely — all produce `None` with current struct, so no breakage expected
   - Recommendation: Verify during COMPAT-01 test; document in COMPAT-9213.md notes column
   - RESOLVED: Existing `Option<SpaceDescription>` / `Option<PlainBody>` nesting handles all three cases (`{}`, `null`, absent) — no code change needed pre-emptively; confirm during COMPAT-01 live test.

2. **Does the 9.2.13 search endpoint return `space` as an empty object `{}` or omit the key when `expand=space` is requested but the item has no space?**
   - What we know: `SearchResultSpace { key: String }` — the field is non-Optional, so if the object is present but empty, parse fails
   - What's unclear: Whether this edge case exists in 9.2.13
   - Recommendation: Add `Option<SearchResultSpace>` defensively; the current code already has `#[serde(default)]` on the space field
   - RESOLVED: Plan 05-06 (COMPAT-06 CQL Search) applies `Option<SearchResultSpace>` defensively as part of its fix scope; this addresses the potential parse failure regardless of whether the edge case is observed on 9.2.13.

3. **Can the live 9.2.13 instance be reached from the developer's machine at test time?**
   - What we know: D-57 specifies user provides URL/token externally; instance is not spun up by the plan
   - What's unclear: TLS certificate validity — the `CCLI_INSECURE=1` env var exists for self-signed certs
   - Recommendation: Document `CCLI_INSECURE=1` usage in the plan's test execution steps
   - RESOLVED: All 6 audit plans (05-01 through 05-06) document `CCLI_INSECURE=1` usage in their test execution steps; the environment table below confirms the env var is already implemented in client.rs.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust / cargo | Compile and run tests | ✓ | (existing project) | — |
| `cargo test` | D-68 acceptance gate (235 tests) | ✓ | Verified: 235 tests pass | — |
| Confluence 9.2.13 instance | Live testing (D-57, D-59) | User-provided | 9.2.13 | No fallback — required externally |
| ccli binary (`cargo run`) | Manual testing per D-59 | ✓ | Built from source | — |
| `CCLI_INSECURE=1` env var | TLS bypass for self-signed certs on test instance | ✓ | Already in client.rs | — |

**Missing dependencies with no fallback:**
- Confluence 9.2.13 live instance — the plan must include a step that waits for/confirms instance availability before executing live test tasks.

**Missing dependencies with fallback:**
- None.

[VERIFIED: Cargo.toml and client.rs read from source; test count verified via `cargo test -- --list`]

---

## Sources

### Primary (HIGH confidence)
- Source code: `src/api/space.rs`, `src/api/page.rs`, `src/api/comment.rs`, `src/api/attachment.rs`, `src/api/client.rs`, `src/cli/page.rs` — all read directly
- `Cargo.toml` — dependency versions confirmed
- `cargo test -- --list` — 235 test count confirmed

### Secondary (MEDIUM confidence)
- [Confluence 9.2 Release Notes](https://confluence.atlassian.com/doc/confluence-9-2-release-notes-1456345480.html) — no REST API breaking changes in 9.2.0–9.2.19; rate limit change is 9.2.20
- [Confluence 9.2 LTS Change Log](https://confluence.atlassian.com/doc/confluence-9-2-long-term-support-release-change-log-1472432426.html) — JS-loaded tables unreadable, narrative confirms no API changes
- [Confluece 9.2 Upgrade Notes](https://confluence.atlassian.com/doc/confluence-9-2-upgrade-notes-1456345485.html) — no API changes documented
- [Confluence DC changelog (developer)](https://developer.atlassian.com/server/confluence/changelog/) — historical 9.2.x not covered
- [CQL search limit at 50 KB article](https://support.atlassian.com/confluence/kb/searching-for-content-with-the-rest-api-and-cql-always-limits-results-to-50/) — confirmed server-side cap

### Tertiary (LOW confidence)
- Web search results for 9.2.x REST API issues — no credible sources found documenting response shape changes between 9.2.13 and 9.2.19
- Expansions documentation [https://developer.atlassian.com/server/confluence/expansions-in-the-rest-api/] — last updated 2019, pre-9.2, but expansion mechanism is stable

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — existing codebase, no new deps, all patterns verified from source
- Architecture: HIGH — all modules read directly; responsibility map is accurate
- Known API differences (9.2.13 vs 9.2.19): LOW — no official documentation found; based on structural analysis only; live testing is the definitive source
- Pitfalls: MEDIUM — some verified from source, some based on general Confluence DC knowledge

**Research date:** 2026-05-08
**Valid until:** 2026-06-08 (stable version; but verify against live instance)
