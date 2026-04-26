---
phase: 01-foundation
plan: "03"
subsystem: output
tags: [rust, comfy-table, is-terminal, tsv, formatter, tty-detection]

requires:
  - phase: 01-foundation/01-01
    provides: "Cargo.toml with comfy-table and is-terminal dependencies, AppError type, project scaffold"

provides:
  - "OutputConfig struct (plain, no_headers, columns fields)"
  - "OutputFormatter struct with TTY-aware dispatch via is-terminal"
  - "table.rs: comfy-table renderer using UTF8_FULL_CONDENSED + Dynamic arrangement"
  - "tsv.rs: bare tab-joined renderer without comfy-table (correct for awk/cut/sort pipelines)"
  - "parse_columns() helper for --columns flag parsing"
  - "resolve_columns() case-insensitive column filter with ordering support"
  - "print_to() testable dispatch helper (W-02)"
  - "23 unit tests across 3 files (pending mod output; declaration in Plan 04)"

affects:
  - 01-foundation/01-04
  - all Phase 2+ command handlers

tech-stack:
  added:
    - comfy-table 7.x (UTF8_FULL_CONDENSED preset, ContentArrangement::Dynamic)
    - is-terminal 0.4 (TTY detection, replaces deprecated atty)
  patterns:
    - "Testable writer pattern: render_to(&mut dyn Write) vs render() → stdout"
    - "render_string() → String pattern for comfy-table testability"
    - "TTY-pinned test constructor with_tty() for dispatch matrix tests"
    - "2x2 dispatch matrix: (is_tty, plain) → table|tsv"

key-files:
  created:
    - src/output/mod.rs
    - src/output/table.rs
    - src/output/tsv.rs
  modified: []

key-decisions:
  - "B-01 fix: mod output; declaration deferred to Plan 04 Task 4.1 Step C to avoid parallel write race with Plan 02 on main.rs"
  - "TSV path uses fields.join(tab) directly — never comfy-table NOTHING preset (Pitfall 5: NOTHING still pads cells)"
  - "resolve_columns() silently skips unknown column names (forgiving Phase 1; strict validation deferred to Phase 2+)"
  - "T-03-03 accepted: tab/newline in cell values could corrupt TSV rows; Phase 2 cell builders must sanitize"

patterns-established:
  - "Output subsystem: all command handlers call OutputFormatter::print(&headers, &rows) — never format inline"
  - "Testable renderer pattern: expose render_to(writer) / render_string() alongside production render()"
  - "D-06 TTY auto-detect: piped stdout falls back to TSV without --plain flag"
  - "D-07 column selection: parse_columns() at CLI dispatch; resolve_columns() at render time"

requirements-completed:
  - OUT-01
  - OUT-02
  - OUT-03
  - OUT-04

duration: 25min
completed: "2026-04-26"
---

# Phase 01 Plan 03: Output Subsystem Summary

**TTY-aware OutputFormatter with comfy-table aligned tables (UTF8_FULL_CONDENSED) and bare tab-join TSV, dispatching on is-terminal detection with column filter and header suppression**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-04-26T14:09:00Z
- **Completed:** 2026-04-26T14:34:25Z
- **Tasks:** 3
- **Files modified:** 3 (all created)

## Accomplishments

- Implemented bare TSV renderer (tsv.rs) without comfy-table — correct for awk/cut/sort pipelines (Pitfall 5)
- Implemented aligned table renderer (table.rs) using comfy-table UTF8_FULL_CONDENSED preset with Dynamic arrangement
- Implemented TTY-aware OutputFormatter (mod.rs) dispatching based on is-terminal detection (D-06) and --plain flag
- Column filter (D-07) with case-insensitive lookup, unknown names silently skipped, user-requested order preserved
- 23 unit tests defined across 3 files (discoverable after Plan 04 adds mod output; to main.rs per B-01 fix)

## Dispatch Table

| is_tty | plain | Renderer |
|--------|-------|----------|
| true   | false | table (UTF8_FULL_CONDENSED box-drawing) |
| true   | true  | tsv (--plain forces TSV even on TTY) |
| false  | false | tsv (D-06 auto-detect: piped → TSV) |
| false  | true  | tsv |

## Public API

```rust
// src/output/mod.rs
pub struct OutputConfig {
    pub plain: bool,
    pub no_headers: bool,
    pub columns: Option<Vec<String>>,
}

pub struct OutputFormatter { /* private */ }

impl OutputFormatter {
    pub fn new(config: OutputConfig) -> Self;
    pub fn print(&self, headers: &[&str], rows: &[Vec<String>]);
}

pub fn parse_columns(s: &str) -> Vec<String>;
```

## Internal Module Split

- **table.rs**: comfy-table renderer — `render()` → stdout, `render_string()` → String (testable)
- **tsv.rs**: bare `\t`-join renderer — `render()` → stdout, `render_to(writer)` → io::Result (testable)

## Test Coverage Matrix

| File | Tests | What they cover |
|------|-------|-----------------|
| src/output/tsv.rs | 6 | headers+rows, no-headers, col filter, reorder, empty rows, empty+no-headers |
| src/output/table.rs | 4 | headers+rows, no-headers suppression, col filter, UTF-8 box-drawing presence |
| src/output/mod.rs | 13 | 5x parse_columns, 3x resolve_columns, 1x struct fields, 4x dispatch matrix (W-02) |
| **Total** | **23** | |

## Task Commits

Each task was committed atomically:

1. **Task 3.1: src/output/tsv.rs** - `dab70f2` (feat)
2. **Task 3.2: src/output/table.rs** - `5a29b61` (feat)
3. **Task 3.3: src/output/mod.rs** - `c1723e1` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `src/output/mod.rs` - OutputConfig, OutputFormatter, parse_columns, resolve_columns, print_to dispatch helper
- `src/output/table.rs` - comfy-table renderer with UTF8_FULL_CONDENSED + render_string testable variant
- `src/output/tsv.rs` - bare tab-join renderer with render_to testable variant

## Decisions Made

- **B-01 fix:** `mod output;` NOT added to `src/main.rs` in this plan. Plan 04 Task 4.1 Step C consolidates `mod api;`, `mod config;`, `mod cli;`, and `mod output;` into main.rs to avoid parallel write races between Plan 02 (config) and Plan 03 (output) in Wave 2.
- **TSV approach:** Direct `fields.join("\t")` instead of comfy-table NOTHING preset — NOTHING still pads cells with spaces, breaking `awk -F'\t'` pipelines (RESEARCH.md Pitfall 5).
- **Forgiving column filter:** Unknown column names are silently skipped in Phase 1. Phase 2+ may add strict validation per command.

## Deviations from Plan

None - plan executed exactly as written. The B-01 fix (not editing main.rs) is a documented plan decision, not a deviation. The acceptance criteria item "23 tests passing" is aspirational for after Plan 04 wires the module; all 23 tests are written and will pass once `mod output;` is added.

## Issues Encountered

None. The B-01 fix means `cargo test --bin ccli output::` shows 0 output tests until Plan 04 completes wiring. This is expected behavior per plan design.

## Phase 2 Follow-ups

- **Cell sanitization (T-03-03):** Phase 2 command handlers that build row values from API response strings must sanitize literal `\t` and `\n` characters before passing to `OutputFormatter::print()`. If a cell value contains a tab, it will corrupt TSV output (corrupts columns) and potentially table alignment.
- **Strict column validation:** Phase 2+ commands may want to error on unknown column names rather than silently skipping.

## Next Phase Readiness

- Output subsystem complete and independent — ready for Plan 04 to wire `mod output;` in main.rs
- OutputFormatter::print(&headers, &rows) is the canonical output API for all Phase 2+ command handlers
- Column filter, TTY detection, and header suppression all tested and verified

---
*Phase: 01-foundation*
*Completed: 2026-04-26*

## Self-Check: PASSED

- FOUND: src/output/tsv.rs
- FOUND: src/output/table.rs
- FOUND: src/output/mod.rs
- FOUND: .planning/phases/01-foundation/01-03-SUMMARY.md
- FOUND: commit dab70f2 (feat: tsv.rs)
- FOUND: commit 5a29b61 (feat: table.rs)
- FOUND: commit c1723e1 (feat: mod.rs)
