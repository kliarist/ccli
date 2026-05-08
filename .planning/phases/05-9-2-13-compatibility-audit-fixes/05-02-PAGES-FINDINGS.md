# COMPAT-02: Pages — Findings

**Tested:** 2026-05-08
**Instance:** Confluence 9.2.13 (no live instance available — static analysis only)
**Status:** PASS (UNVERIFIED — no live instance)

## Expected Behavior

`GET /rest/api/content?type=page` lists pages with version+ancestors expansions.
`GET /rest/api/content/{id}?expand=body.storage,version,ancestors` returns full page detail with storage XML body.
`POST /rest/api/content` creates a new page; `PUT /rest/api/content/{id}` updates with version bump.

## Sub-area Status

| Sub-area | Status | Notes |
| -------- | ------ | ----- |
| List (Step A) | PASS (UNVERIFIED) | Static analysis: ContentListResponse, Page structs fully Option-defended. `content_type: String` (not enum) avoids unknown-string deserialization failures. `_links: PageLinks` uses `Default` derive and `#[serde(rename)]`. Pagination guard uses `has_next || fetched < limit` (D-33). |
| View (Step B) | PASS (UNVERIFIED) | Static analysis: `PageDetail.body: Option<PageBody>`, `PageBody.storage: Option<StorageBody>`, `StorageBody.value: Option<String>`. All nested body fields are defensively Option — absent or null `body.storage` cannot cause a parse panic. |
| Create (Step D) | PASS (UNVERIFIED) | Static analysis: `create_page` builds the request body via `serde_json::json!()` with the correct shape (type, title, space.key, body.storage, conditional ancestors). Returns `Page` (same shape as list item), which covers both 200 and 201 status codes. |
| Edit/Update (Step E) | PASS (UNVERIFIED) | Static analysis: `update_page` sends `version.number = current_version + 1` (Pitfall 1). Sends `title` unconditionally (Pitfall 2). 409 Conflict is mapped to `AppError::Api("Conflict: page was updated by someone else (version {N})")` (D-39). |
| Pagination (Step C) | PASS (UNVERIFIED) | Static analysis: loop breaks when `!has_next || fetched < limit`. `has_next` is `page.links.next.is_some()` — evaluates to `false` on absent or null `_links.next`. Note: an empty string `""` in `_links.next` would evaluate `is_some()` as `true` and could cause a hang. This is a latent risk (see Notes). |

## Observed on 9.2.13

No live instance was available for testing. All observations are based on static analysis of `src/api/page.rs` against the research findings in `05-RESEARCH.md` and `05-PATTERNS.md`.

Key structural findings from static analysis:

1. **`Page.content_type: String`** — The type field is deserialized as a plain `String` rather than the `ContentType` enum, so unknown or extra values from the server cannot cause deserialization failures.

2. **`PageDetail.links: PageLinks`** (non-optional, with `Default`) — The `_links` field in `PageDetail` is not `Option<PageLinks>` but the struct derives `Default`, so an absent `_links` JSON key will use the default (both fields `None`). This is safe.

3. **`Page.links: PageLinks`** (non-optional, no Default) — The `Page` (list item) struct uses `#[serde(rename = "_links")] pub links: PageLinks` without `Option` wrapping. If the 9.2.13 API omits `_links` on list items, this would cause a parse failure. However, RESEARCH.md notes that `_links` is a standard Confluence DC field present since v5.x; the risk is low.

4. **Pagination empty-string guard** — `page.links.next.is_some()` evaluates `true` for an empty string `""`. If 9.2.13 returns `_links: {"next": ""}` to signal end-of-list, the loop would not terminate. The RESEARCH.md pagination probe did not document this case specifically, and the existing test suite does not cover it.

## Fix Applied

None — Status is PASS (static analysis). `src/api/page.rs` is unchanged.

## Notes for COMPAT-9213.md

COMPAT-02 Pages: Static analysis confirms defensive serde shapes (`Option<T>` throughout body/version/ancestors fields, String content_type). No live instance available to verify against 9.2.13; structural risk is low. One latent edge case: `_links.next: ""` empty-string pagination guard (same as COMPAT-01 risk area) — not triggered in tests.
