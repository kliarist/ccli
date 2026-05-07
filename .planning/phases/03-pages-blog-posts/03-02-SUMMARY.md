---
phase: 03-pages-blog-posts
plan: 02
subsystem: output
tags: [rust, quick-xml, xml-parsing, storage-xml, confluence, plain-text, tui]

# Dependency graph
requires:
  - phase: 03-pages-blog-posts-plan-01
    provides: quick-xml = "0.39" added to Cargo.toml

provides:
  - "src/output/xml.rs: strip_storage_xml(xml: &str) -> String — Confluence storage XML to plain text"
  - "src/output/mod.rs: pub mod xml; pub use xml::strip_storage_xml; re-exports"

affects:
  - 03-pages-blog-posts-plan-04  # TUI preview pane — imports strip_storage_xml (D-42)
  - 03-pages-blog-posts-plan-05  # ccli page view stdout — imports strip_storage_xml (D-36)

# Tech tracking
tech-stack:
  added: []  # quick-xml was added in plan 01
  patterns:
    - "quick-xml 0.39 event-based reader with Reader::from_str — uses read_event() not read_event_into()"
    - "Event::GeneralRef handling required for entity decoding in quick-xml 0.39 (entities no longer embedded in Text events)"
    - "depth counters (heading_depth, code_depth) for nested tag context tracking"
    - "#[allow(dead_code)] on pub fn for future-phase wiring (consistent with codebase pattern)"

key-files:
  created:
    - src/output/xml.rs
  modified:
    - src/output/mod.rs

key-decisions:
  - "quick-xml 0.39 emits &amp;/&lt;/&gt;/&quot; as Event::GeneralRef (not embedded in Text) — resolved via resolve_predefined_entity() from quick_xml::escape"
  - "Use Reader::from_str() with read_event() (no buffer) rather than from_reader() with read_event_into(&mut buf)"
  - "trim_text(true) kept; entity text arrives via GeneralRef events so trimming whitespace-only Text events is safe"
  - "collapse_blank_lines() post-processes output to cap consecutive newlines at two"

patterns-established:
  - "TDD RED/GREEN: stub returns empty String, 9 failing tests → full implementation, 14 passing"
  - "emit_text() helper centralises context-aware text emission (heading/code/li modes)"

requirements-completed:
  - PAGE-04
  - PAGE-08

# Metrics
duration: 9min
completed: 2026-05-06
---

# Phase 03 Plan 02: strip_storage_xml Summary

**Pure event-based Confluence storage XML to plain-text converter using quick-xml 0.39 GeneralRef API, with 14 unit tests covering all seven UI-SPEC transformation rules**

## Performance

- **Duration:** 9 min
- **Started:** 2026-05-06T14:02:07Z
- **Completed:** 2026-05-06T14:11:00Z
- **Tasks:** 1 (TDD: 2 commits — RED + GREEN)
- **Files modified:** 2

## Accomplishments

- Implemented `strip_storage_xml(xml: &str) -> String` in `src/output/xml.rs` using quick-xml 0.39 event reader
- Handled quick-xml 0.39's new `Event::GeneralRef` variant for entity decoding (`&amp;`, `&lt;`, `&gt;`, `&quot;`)
- All seven UI-SPEC transformation rules covered: headings uppercase, paragraphs with blank line, code/pre two-space indent, list item dash prefix, entity decoding, blank-line collapse, malformed XML tolerance, and ac:*/ri:* namespace stripping (Pitfall 7)
- 14 unit tests pass; 138 total project tests pass; cargo build and clippy clean with no warnings

## Task Commits

Each task was committed atomically (TDD):

1. **RED — failing tests** - `673c164` (test)
2. **GREEN — full implementation** - `95d0421` (feat)

**Plan metadata:** (docs commit follows)

_Note: TDD task — RED commit has stub implementation, GREEN has full implementation_

## Files Created/Modified

- `src/output/xml.rs` — `strip_storage_xml()` + `collapse_blank_lines()` helper + 14 unit tests
- `src/output/mod.rs` — added `pub mod xml;` and `#[allow(unused_imports)] pub use xml::strip_storage_xml;`

## Decisions Made

- **quick-xml 0.39 API change:** `Event::GeneralRef` is a new event variant in 0.39 that carries entity references (`&amp;` etc.) separately from `Event::Text`. The plan code used `e.unescape()` which no longer exists on `BytesText` — replaced with `resolve_predefined_entity()` from `quick_xml::escape` applied to `GeneralRef` events.
- **`read_event()` not `read_event_into()`:** `Reader::from_str()` returns a slice reader whose method is `read_event()` (returns by value, no buffer arg). The buffered `read_event_into(&mut buf)` is for `Reader::from_reader()`. Used the correct slice reader API.
- **`#[allow(dead_code)]` on public function:** Consistent with the rest of the codebase (e.g., `src/api/page.rs`). The function is public and will be called in Plans 04 and 05.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] quick-xml 0.39 emits entities as Event::GeneralRef, not embedded in Text**
- **Found during:** Task 1 GREEN phase
- **Issue:** Plan code used `e.unescape()` on `BytesText` — this method does not exist in quick-xml 0.39. In 0.39, entity references (`&amp;`, `&lt;`, etc.) are emitted as `Event::GeneralRef` events rather than being embedded in `Event::Text` with escape sequences.
- **Fix:** Added `Ok(Event::GeneralRef(ref e))` arm to the main loop. Uses `resolve_predefined_entity()` for named entities (amp, lt, gt, quot, apos) and `e.resolve_char_ref()` for numeric references (`&#60;`). Unknown entities are silently dropped.
- **Files modified:** src/output/xml.rs
- **Verification:** `entity_decoded_amp_lt_gt_quot` test passes; all 14 tests pass
- **Committed in:** 95d0421 (GREEN feat commit)

**2. [Rule 1 - Bug] Reader::from_str uses read_event() not read_event_into()**
- **Found during:** Task 1, compile check
- **Issue:** Plan code used `reader.read_event_into(&mut buf)` — this is for `Reader::from_reader()` (buffered). `Reader::from_str()` uses `read_event()` (no buffer arg, returns by value).
- **Fix:** Used `reader.read_event()` without buffer; removed `buf` and `buf.clear()` lines.
- **Files modified:** src/output/xml.rs
- **Verification:** Code compiles, all tests pass
- **Committed in:** 95d0421 (GREEN feat commit)

---

**Total deviations:** 2 auto-fixed (both Rule 1 — quick-xml 0.39 API differences from plan's assumed API)
**Impact on plan:** Both fixes required due to quick-xml 0.39 API evolution from earlier versions. Behavior is identical to plan intent. No scope creep.

## Issues Encountered

- quick-xml 0.39 changed the text event API significantly from older versions: entities are now `Event::GeneralRef`, decode methods changed, and `from_str` vs `from_reader` have different method signatures. The plan's code examples were based on an older API. Discovered and fixed during GREEN phase.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `strip_storage_xml` is importable as `crate::output::strip_storage_xml` (or `crate::output::xml::strip_storage_xml`)
- Ready for Plan 03 (blog post API client), Plan 04 (TUI preview pane), and Plan 05 (`ccli page view`)
- No blockers

---
*Phase: 03-pages-blog-posts*
*Completed: 2026-05-06*

## Self-Check: PASSED

- FOUND: src/output/xml.rs
- FOUND: src/output/mod.rs
- FOUND: .planning/phases/03-pages-blog-posts/03-02-SUMMARY.md
- FOUND: commit 673c164 (RED — test)
- FOUND: commit 95d0421 (GREEN — feat)
- Tests: 14 passed, 0 failed
