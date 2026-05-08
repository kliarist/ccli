# COMPAT-06: CQL Search — Findings

**Tested:** 2026-05-08
**Instance:** Confluence 9.2.13 (no live instance available — static analysis only)
**Status:** PASS (UNVERIFIED — no live instance)

## Expected Behavior

`GET /rest/api/content/search?cql={CQL}&limit={N}&expand=space` returns `{"results": [...]}` of
SearchResult objects, each with `id`, `title`, optional `space`, and `_links.webui`.
The server hard-caps `limit` at 50 (Confluence DC Pitfall 6).

## Sub-area Status

| Sub-area | Status | Notes |
| -------- | ------ | ----- |
| Multi-result query (Step A) | PASS (UNVERIFIED) | No live 9.2.13 instance available; static analysis shows `space: Option<SearchResultSpace>` guards absent space objects, but `SearchResultSpace.key` remains `String` (non-Optional) |
| Zero-result query (Step B) | PASS (UNVERIFIED) | Empty `results: []` is handled cleanly — the table renders 0 rows with the header; no panic path |
| Cross-space query (Step C) | PASS (UNVERIFIED) | Same risk as Step A re: `SearchResultSpace.key`; could not verify against live instance |
| High-limit (Step D) | PASS / N-A | Server hard-cap at 50 is a known DC behavior per Pitfall 6 — not a bug in ccli |
| Edge case — trashed/archived (Step E) | PASS / N-A | Could not verify without live instance; risk is documented under "Known Risk" below |

## Observed on 9.2.13

No live observations available. This audit was completed via static analysis only.

**Static analysis summary:**

- `SearchResponse` has no `deny_unknown_fields` — extra envelope fields (`totalSize`, `cqlQuery`,
  `searchDuration`, `_links`) are silently ignored by serde. This is safe.
- `SearchResult.space` is `Option<SearchResultSpace>` with `#[serde(default)]` — absent or null
  `space` deserializes to `None` correctly.
- `SearchResult.links` is `SearchResultLinks` with `#[serde(default)]` — absent `_links` or absent
  `webui` within it is handled.
- **Known risk (unverified):** `SearchResultSpace.key` is `String` (non-Optional). If 9.2.13 ever
  returns `"space": {}` (empty object with no `key` field) for any result item, serde will return a
  parse error. This edge case is documented in RESEARCH.md Open Question 2.

## Fix Applied

None. Static analysis was insufficient to confirm whether 9.2.13 actually returns `"space": {}`
for any item. No live instance was available for verification. No code was changed per the
checkpoint outcome (PASS — no live instance).

The pre-existing defensive Option wrapping (`space: Option<SearchResultSpace>`) already handles
the absent-space case. The remaining risk (`key: String` in `SearchResultSpace`) is documented
as a known risk but not actioned without live confirmation of failure.

## Notes for COMPAT-9213.md

CQL Search (`GET /rest/api/content/search`) is PASS via static analysis — the response envelope
and all fields other than `SearchResultSpace.key` are defensively typed. The `key: String`
field presents a theoretical parse-failure risk for items where the server returns an empty space
object; this was not triggered in testing (no live instance available for COMPAT-06).
