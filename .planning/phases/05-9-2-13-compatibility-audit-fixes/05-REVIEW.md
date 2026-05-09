---
phase: 05-9-2-13-compatibility-audit-fixes
reviewed: 2026-05-09T00:00:00Z
depth: standard
files_reviewed: 1
files_reviewed_list:
  - COMPAT-9213.md
findings:
  critical: 0
  warning: 4
  info: 3
  total: 7
status: issues_found
---

# Phase 05: Code Review Report

**Reviewed:** 2026-05-09
**Depth:** standard
**Files Reviewed:** 1
**Status:** issues_found

## Summary

`COMPAT-9213.md` is a public-facing compatibility matrix documenting ccli behavior against Confluence Data Center 9.2.13. The document was cross-referenced against the actual source files (`src/api/space.rs`, `src/api/page.rs`, `src/api/comment.rs`, `src/api/attachment.rs`, `src/cli/page.rs`, `src/api/client.rs`).

The document's overall framing is sound: it correctly discloses that no live instance was available and that all findings are static-analysis only. However, several factual errors and misleading framings were found that could cause users or future contributors to draw incorrect conclusions about correctness, scope, or risk severity.

No critical security or data-loss issues were found in the document itself (those belong in the source review). All findings below are accuracy, framing, and quality defects in the documentation artifact.

---

## Warnings

### WR-01: "Observed on 9.2.13" column header is misleading — no live observations were made

**File:** `COMPAT-9213.md:9`
**Issue:** Every row in the "Observed on 9.2.13" column begins with "No live instance available." The column heading implies live runtime observations were recorded. A reader scanning the table heading without reading the Note block at line 29 will believe each row documents empirical results. This is a truthfulness problem in the document's most-consumed artifact (the table).
**Fix:** Rename the column to "Assessment (9.2.13)" or "Static Analysis Notes", and move the "No live instance available" disclaimer into the column header's tooltip or the table caption rather than repeating it in every cell:

```markdown
| Endpoint Area | Expected Behavior | Assessment (static analysis only — no live 9.2.13 instance) | Status | Notes |
```

---

### WR-02: "Tested Versions" table uses "Test date" wording implying live testing

**File:** `COMPAT-9213.md:40-42`
**Issue:** The "Tested Versions" table presents a row with headers `ccli version | Confluence DC version | Test date` and the value `0.1.0 | 9.2.13 | 2026-05-08`. The term "Test date" conventionally means the date live integration testing was performed. Combined with the column value `9.2.13`, a reader will conclude a live Confluence 9.2.13 instance was tested on that date. In fact the document explicitly states no live instance was available. This table directly contradicts the disclaimer at lines 29 and 33 for any reader who reads only the table.
**Fix:** Rename the table header and add a clarifying note:

```markdown
## Assessed Versions

| ccli version | Confluence DC target | Analysis date | Method |
|--------------|----------------------|---------------|--------|
| 0.1.0 | 9.2.13 | 2026-05-08 | Static analysis (no live instance) |
```

---

### WR-03: Test Method section incorrectly scopes the CQL Search audit to `src/api/`

**File:** `COMPAT-9213.md:25`
**Issue:** The Test Method section states: "Each endpoint area was assessed by performing static analysis of the relevant API module (`src/api/`)." The CQL Search implementation (`handle_search`, `SearchResponse`, `SearchResult`, `SearchResultSpace`, `SearchResultLinks`) lives entirely in `src/cli/page.rs`, not in `src/api/`. This means the stated audit scope does not cover the file that actually contains the search deserialization risk (`SearchResultSpace.key: String`) that the CQL row's Notes column calls out. A future contributor following this document to conduct a re-audit would look in the wrong place.
**Fix:**

```markdown
Each endpoint area was assessed by performing static analysis of the relevant source module:
- Spaces, Pages, Blog Posts: `src/api/space.rs`, `src/api/page.rs`
- Comments: `src/api/comment.rs`
- Attachments: `src/api/attachment.rs`
- CQL Search: `src/cli/page.rs` (response structs and HTTP call live in the CLI layer, not `src/api/`)
```

---

### WR-04: Pagination empty-string sentinel is documented as "latent risk" but the code confirms it is an unguarded defect

**File:** `COMPAT-9213.md:11,12,13,36`
**Issue:** The document describes the empty-string `_links.next` pagination risk as "latent" and "unlikely given documented DC behavior." The framing understates the issue. Code inspection of `space.rs:132-134` and `page.rs:185-187` confirms the guard is `page.links.next.is_some()`. An `Option<String>` holding `Some("")` satisfies `is_some()` and therefore the pagination loop would never terminate if the server emits `"_links": {"next": ""}`. This is an infinite loop / hang defect, not a latent theoretical risk. The document's own "Caveats" section at line 36 says "the pagination loop would not terminate" — which is the correct severity — but the per-area Notes downgrade this to "structural risk is low" (Pages row) and "not observed" without noting that the code has no guard against it at all.
**Fix:** Align per-area Notes with the Caveats severity and accurately describe the code's actual behavior:

```markdown
Notes (Spaces): ... Latent defect: if 9.2.13 returns `_links.next: ""` (empty string), the
pagination guard (`is_some()` check) does not filter it and the loop would not terminate.
No guard for empty-string next links exists in the code. This is not observed in documented
DC behavior but has not been verified against a live instance.
```

---

## Info

### IN-01: FIXED and FAIL status values are defined but never used

**File:** `COMPAT-9213.md:19-21`
**Issue:** The Status legend defines three values — PASS, FIXED, and FAIL — but every row in the matrix uses only PASS. The FIXED and FAIL definitions give the impression that fixes were applied or failures discovered, which may confuse readers. Since no code changes were made during this audit, FIXED will never appear; since no broken areas were found, FAIL will never appear.
**Fix:** Either remove the unused status values from the legend or add a sentence explaining that only PASS appears because no regressions were found:

```markdown
Status values:
- **PASS** — Endpoint area is compatible with 9.2.13 per static analysis; no code change required.

_Note: FIXED (code change applied) and FAIL (broken, no fix available) statuses are defined for_
_future re-audits against live instances and are not used in this audit._
```

---

### IN-02: CQL search area states "server hard-caps results at 50" but the user-supplied limit is forwarded unclamped

**File:** `COMPAT-9213.md:16`
**Issue:** The CQL row Notes state: "The server-side result cap of 50 is a known Confluence DC behavior, not a ccli limitation." This is accurate but incomplete. Code inspection shows `handle_search` in `src/cli/page.rs:349` forwards the user-supplied `--limit` value directly to the server without clamping. A user passing `--limit 100` will silently receive at most 50 results. The document implies this is transparent behavior, but there is no user-visible warning when the limit exceeds the server cap, so the user may not realize they are receiving a truncated result set. This is a documentation gap, not a code defect, but the compatibility document's framing could mislead users into trusting `--limit 100` returns 100 results.
**Fix:** Add a sentence to the CQL row Notes:

```markdown
The user-supplied `--limit` value is forwarded to the server; values above 50 are silently
capped by the server with no warning to the caller.
```

---

### IN-03: Attachment download URL resolution describes "absolute and relative paths" but does not cover protocol-relative URLs

**File:** `COMPAT-9213.md:15`
**Issue:** The Attachments row Notes state "download URL resolution handles both absolute and relative paths." The code at `src/api/attachment.rs:127` implements this as `if download_path.starts_with("http")`. This correctly handles `http://...` and `https://...` (both start with "http") and relative paths starting with `/`. However, a protocol-relative URL such as `//cdn.example.com/path` would fail the `starts_with("http")` check, be treated as a relative path, and produce a malformed URL like `https://confluence.example.com//cdn.example.com/path`. Confluence DC 9.2.x is not known to return protocol-relative download URLs, so this is low risk, but the documentation overstates the robustness of the implementation.
**Fix:** Qualify the claim:

```markdown
Download URL resolution handles full HTTP/HTTPS URLs and server-relative paths (starting with `/`).
Protocol-relative URLs (`//host/path`) are not handled but are not returned by Confluence DC.
```

---

_Reviewed: 2026-05-09_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
