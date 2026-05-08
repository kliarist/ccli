# Phase 5: 9.2.13 Compatibility Audit & Fixes - Context

**Gathered:** 2026-05-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Systematically verify all six REST endpoint areas used by ccli against a live Confluence 9.2.13 instance. Document findings in COMPAT-9213.md (repo root, public-facing). Apply minimal in-place fixes for any 9.2.13-specific breakage, with a unit test per fix. Finish by writing COMPAT-9213.md as a 6-row test matrix after all areas are tested and fixed.

**6 endpoint areas (COMPAT-01–06):**
1. Spaces — `GET /rest/api/space`
2. Pages — `GET /rest/api/content` (list, get, create, update)
3. Blog Posts — `GET /rest/api/content?type=blogpost` (list, create, update)
4. Comments — `GET /rest/api/content/{id}/child/comment` + `POST`
5. Attachments — `GET /rest/api/content/{id}/child/attachment` + multipart POST + download
6. CQL Search — `GET /rest/api/content/search`

Out of phase: version detection shims, compatibility layers, opportunistic refactoring, spinning up the 9.2.13 instance (user handles that externally).

</domain>

<decisions>
## Implementation Decisions

### Audit Execution Method

- **D-57:** **Testing is against a live 9.2.13 instance** — user provides URL/token externally. The plan must NOT include a "spin up Confluence" step; assume a running instance is available at execution time.
- **D-58:** **Changelog/docs review first** — the research step reads Confluence 9.2.13 release notes and REST API docs to pre-identify known differences vs. 9.2.19 before any live testing. This makes testing more targeted.
- **D-59:** **Manual ccli runs + human observation** — run each ccli command against the 9.2.13 instance, observe stdout output and errors, record findings by hand. No automated response-capture scripts.

### COMPAT-9213.md Structure

- **D-60:** **Written all at once after testing** — test all 6 areas and apply fixes first; write COMPAT-9213.md as a final summary step.
- **D-61:** **Format: test matrix with 6 rows** — one row per endpoint area (maps 1:1 to COMPAT-01–06). Columns: `Endpoint Area | Expected Behavior | Observed on 9.2.13 | Status (PASS / FAIL / FIXED) | Notes`.
- **D-62:** **Location: repo root, public-facing** — committed to the repository root alongside CHANGELOG.md etc. Other Confluence DC users can reference it.

### Fix Strategy

- **D-63:** **Fix in-place in existing API modules** — edit `src/api/space.rs`, `page.rs`, `comment.rs`, `attachment.rs`, `cli/page.rs` directly. No version detection, no compatibility shims. Fixes must be backward-compatible (should not break 9.2.19 behavior).
- **D-64:** **Minimal scope** — only fix what actually breaks on 9.2.13. No opportunistic refactoring or unrelated cleanup.
- **D-65:** **Fix as you go** — test an area → find issue → fix immediately → confirm it passes → move to next area. Do not batch all testing before fixing.

### Test Coverage

- **D-66:** **Unit test per fix** — for every code change, add at least one unit test mocking the 9.2.13-compatible response shape, using the existing `httpmock` pattern established in Phases 2–4.
- **D-67:** **Update existing tests to the compatible shape** — when parsing logic changes, update existing mock responses so the same code/tests work for both 9.2.13 and 9.2.19. Do not maintain separate per-version test suites.
- **D-68:** **Acceptance bar: all 235 existing tests pass + at least one new test per code fix.** No specific count target beyond that.

### Claude's Discretion

- Specific response-shape differences between 9.2.13 and 9.2.19 are unknown until testing; the researcher will identify likely candidates from changelogs.
- Exact column widths and wording in COMPAT-9213.md rows can be determined during writing.
- Decision ID continuation: Phase 4 ended at D-56. Phase 5 starts at D-57.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 5 Requirements
- `.planning/REQUIREMENTS.md` — COMPAT-01 through COMPAT-06 (6 endpoint area requirements) and TMAT-01 (test matrix artifact); all with acceptance criteria
- `.planning/ROADMAP.md` §"Phase 5: 9.2.13 Compatibility Audit & Fixes" — goal, success criteria, requirement mapping

### Project Context
- `.planning/PROJECT.md` — Key decisions (PAT-only auth, Rust, jira-cli UX model); Out of Scope items (Confluence Cloud, Markdown conversion, OAuth); Current Milestone goal

### Prior Phase Decisions
- `.planning/phases/04-comments-attachments-search/04-CONTEXT.md` — Phase 4 locked decisions (D-45..D-56); API module patterns, `Client.inner()` usage, `OutputFormatter` reuse, `strip_storage_xml()`, httpmock test pattern
- `.planning/phases/03-pages-blog-posts/03-CONTEXT.md` — D-30..D-44 (screen stack, ContentType enum, editor workflow, pagination pattern)

### Target API (both versions under test)
- Confluence DC REST API v1 endpoints — all use `/rest/api/` prefix, PAT auth header
- 9.2.19 (development baseline): what the current code was built against
- 9.2.13 (audit target): live instance provided by user at execution time

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets (modules under audit)
- `src/api/space.rs` — `list_spaces()` + `get_space()` — uses `GET /rest/api/space` with `expand=description.plain,homepage`; pagination via `_links.next`
- `src/api/page.rs` — `list_all_pages()`, `get_page()`, `create_page()`, `update_page()` — uses `/rest/api/content` with `expand=body.storage,version,ancestors`; pagination via `_links.next`
- `src/api/comment.rs` — `list_comments()`, `add_comment()` — uses `/rest/api/content/{id}/child/comment?expand=body.storage,version&limit=50`
- `src/api/attachment.rs` — `list_attachments()`, `download_attachment()`, `upload_attachment()` — uses `/rest/api/content/{id}/child/attachment`; multipart POST for upload
- `src/api/client.rs` — `Client` struct with `inner()` (raw reqwest) and `base_url()`; PAT injected via default header; `test_connection()` probes `/rest/api/user/current`
- `src/cli/page.rs` — CQL search handler — uses `GET /rest/api/content/search?cql={CQL}&limit=25&expand=space`

### Established Patterns
- **httpmock unit tests** — all API modules have inline `#[cfg(test)]` tests using `httpmock::MockServer`; mock server at `test_client(&server.base_url())`. New tests for fixes must follow this exact pattern.
- **Error taxonomy** — `AppError::Api`, `AppError::Auth`, `AppError::Network` in `src/api/error.rs`; existing error handling must be preserved in any fix.
- **Pagination** — `_links.next` loop pattern in `list_all_pages()` and `list_spaces()`; check if 9.2.13 paginates differently.
- **expand params** — critical to test: 9.2.13 may return fewer `_expandable` fields or different expand key names.

### Integration Points
- All fixes land in `src/api/` modules — the TUI (`src/tui/`) and CLI (`src/cli/`) layers should not need changes unless response struct fields change.
- If a response struct field is removed or renamed, update the corresponding `src/tui/screens/` render code that reads it.

</code_context>

<specifics>
## Specific Ideas

- Changelog review is explicitly part of the research/planning step — downstream agents should search for Confluence 9.2.13 release notes and REST API changelog between 9.2.13 and 9.2.19.
- COMPAT-9213.md is a public artifact — write it for other Confluence DC CLI users, not just as internal phase evidence. Tone: factual, concise, no internal planning jargon.
- The `expand=` query parameters are a high-probability difference area — Confluence minor versions sometimes change which fields are expandable by default.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 5-9.2.13 Compatibility Audit & Fixes*
*Context gathered: 2026-05-08*
