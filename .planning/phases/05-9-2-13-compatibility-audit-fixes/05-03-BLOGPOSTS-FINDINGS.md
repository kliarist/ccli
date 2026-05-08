# COMPAT-03: Blog Posts — Findings

**Tested:** 2026-05-08
**Instance:** Confluence 9.2.13 (no live instance available — static analysis only)
**Status:** PASS (UNVERIFIED — no live instance)

## Expected Behavior

`GET /rest/api/content?type=blogpost` lists blog posts with version+ancestors expansions.
`POST /rest/api/content` with `type: "blogpost"` creates a new blog post (no `ancestors` field).
`PUT /rest/api/content/{id}` updates with version bump — same endpoint as pages.
Same shared code path as pages (`src/api/page.rs`) but with `ContentType::BlogPost` discriminator
passed to each API function.

## Sub-area Status

| Sub-area | Status | Notes |
| -------- | ------ | ----- |
| List (Step A) | PASS (UNVERIFIED) | Static analysis: `list_all_pages` passes `content_type.as_api_str()` ("blogpost") as the `type` query parameter. `ContentListResponse` and `Page` structs are fully Option-defended (same as COMPAT-02 pages path). Existing test `list_all_pages_uses_blogpost_type_for_blog` verifies the `type=blogpost` query param is sent correctly. |
| Create (Step B) | PASS (UNVERIFIED) | Static analysis: `create_page` with `ContentType::BlogPost` correctly omits `ancestors` field (Pitfall 5 — `if matches!(content_type, ContentType::Page)` guard). Existing test `create_blog_post_omits_ancestors_field` asserts this. `space.key` and `body.storage` are included as required. |
| Edit/Update (Step C) | PASS (UNVERIFIED) | Static analysis: `update_page` is content-type-agnostic for the PUT body — sends `id`, `type`, `title`, `space.key`, `version.number = current_version + 1`, and `body.storage`. Blog posts use the same PUT endpoint and shape as pages. 409 Conflict handling applies equally (D-39). |

## Observed on 9.2.13

No live instance was available for testing. All observations are based on static analysis of
`src/api/page.rs` against the research findings in `05-RESEARCH.md` and `05-PATTERNS.md`,
combined with the existing unit test suite.

Key structural findings from static analysis:

1. **`ContentType::BlogPost` discriminator** — `content_type.as_api_str()` returns `"blogpost"`,
   correctly driving `type=blogpost` on list queries and `"type": "blogpost"` in create/update
   request bodies. This is verified by the existing `list_all_pages_uses_blogpost_type_for_blog`
   and `create_blog_post_omits_ancestors_field` tests.

2. **`ancestors` omission for blog posts** — `create_page` with `ContentType::BlogPost` skips
   the `ancestors` JSON field entirely (Pitfall 5 guard). The Confluence DC REST API rejects
   blog post create requests that include an `ancestors` field with a 400 error. This defense
   is already in place and unit-tested.

3. **Shared serde shapes** — Blog posts share `Page`, `PageDetail`, `ContentListResponse`,
   `PageVersion`, `PageBody`, and `StorageBody` structs with pages. All variable fields use
   `Option<T>`. The `content_type: String` field (not enum) safely accepts both `"page"` and
   `"blogpost"` values without deserialization risk.

4. **Pagination** — Same `_links.next` `is_some()` logic applies to blogpost list responses.
   The same latent `_links.next: ""` empty-string risk noted in COMPAT-02 applies here too.
   No blogpost-specific pagination behavior differences documented in RESEARCH.md.

## Relationship to COMPAT-02

Blog posts share `src/api/page.rs` with regular pages via the `ContentType` enum discriminator.
COMPAT-02 (Plan 02) confirmed via static analysis that all CRUD struct fields are defensively
structured with `Option<T>`. Since blog posts use the same deserialization structs, COMPAT-02's
PASS result covers the blogpost deserialization path as well.

The only blogpost-specific code path is the `ancestors` omission guard in `create_page` and the
`type=blogpost` query parameter in `list_all_pages` — both are already unit-tested. No separate
code fix was needed for COMPAT-03.

COMPAT-03 is therefore a structural PASS: Plan 02's shared-code confirmation extends to blog
posts, and the blogpost-specific behaviors (Pitfall 5 ancestors guard, `type=blogpost` param)
are independently verified by the existing test suite.

## Fix Applied

None — Status is PASS (static analysis). `src/api/page.rs` is unchanged.

## Notes for COMPAT-9213.md

COMPAT-03 Blog Posts: Static analysis confirms blog posts share the same defensively-structured
serde shapes as pages (COMPAT-02). The `ContentType::BlogPost` discriminator correctly drives
`type=blogpost` on list queries and omits `ancestors` on create (Pitfall 5). Both behaviors are
unit-tested. No live instance available; structural risk is low. Same latent `_links.next: ""`
empty-string pagination risk as COMPAT-01/02.
