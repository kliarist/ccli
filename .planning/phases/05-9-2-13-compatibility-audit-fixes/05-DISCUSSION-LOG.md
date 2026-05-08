# Phase 5: 9.2.13 Compatibility Audit & Fixes - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-08
**Phase:** 05-9.2.13 Compatibility Audit & Fixes
**Areas discussed:** Audit execution method, COMPAT-9213.md structure, Fix strategy, Test coverage for fixes

---

## Audit Execution Method

| Option | Description | Selected |
|--------|-------------|----------|
| Live 9.2.13 instance | Test against a real Confluence 9.2.13 server | ✓ |
| Manual API probing only | Use curl/httpie to hit API directly | |
| Changelog/docs diff only | Read release notes, no live testing | |

**User's choice:** Live 9.2.13 instance

---

| Option | Description | Selected |
|--------|-------------|----------|
| Already running — just need URL/token | Instance is up; plan starts with probing | |
| Needs setup first | Spin up via Docker Compose as a plan step | |
| Setup is out of scope — I'll handle it | Manual/external; plan assumes live URL available | ✓ |

**User's choice:** Setup out of scope — user provides instance externally

---

| Option | Description | Selected |
|--------|-------------|----------|
| Manual ccli runs + human observation | Run each command, observe output, record by hand | ✓ |
| Automated scripts capture responses | Shell script or Rust harness captures HTTP responses | |
| Mix: automate collection, manual review | Scripts capture; human reviews differences | |

**User's choice:** Manual ccli runs + human observation

---

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — changelog review first | Read 9.2.13 release notes before live testing | ✓ |
| No — go straight to live testing | Test all 6 areas empirically | |

**User's choice:** Yes — changelog review before testing

---

## COMPAT-9213.md Structure

| Option | Description | Selected |
|--------|-------------|----------|
| Test matrix: endpoint + status | 6 rows, PASS/FAIL/FIXED columns, concise | ✓ |
| Detailed request/response log | Full curl, payloads, response JSON, diffs | |
| Narrative findings report | Prose per endpoint area | |

**User's choice:** Test matrix format

---

| Option | Description | Selected |
|--------|-------------|----------|
| One row per endpoint area (6 rows) | Maps 1:1 to COMPAT-01–06 | ✓ |
| One row per individual endpoint | Finer granularity, separate rows for list/get/create/update | |
| One row per feature+operation combo | Most granular | |

**User's choice:** One row per endpoint area (6 rows)

---

| Option | Description | Selected |
|--------|-------------|----------|
| Repo root — public facing | Committed alongside CHANGELOG.md for community users | ✓ |
| Planning dir only — internal artifact | Lives in .planning/, not published | |
| Repo root but clearly internal | Committed but marked as internal audit artifact | |

**User's choice:** Repo root, public-facing

---

| Option | Description | Selected |
|--------|-------------|----------|
| Pre-scaffold first | Create skeleton with 6 rows before testing | |
| Write during testing | Each audit step writes its row | |
| Write all at once after testing | Test everything first, write summary at the end | ✓ |

**User's choice:** Write all at once after all 6 areas tested and fixed

---

## Fix Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Fix in-place in existing API module | Edit src/api/ directly, backward-compatible | ✓ |
| Add version-conditional handling | Runtime version detection + branch behavior | |
| Document only — no code changes | Record in COMPAT-9213.md, no fixes | |

**User's choice:** Fix in-place in existing API modules

---

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal — only fix what breaks | Surgical changes only | ✓ |
| Opportunistic — clean up related code | Fix + refactor nearby tech debt | |

**User's choice:** Minimal scope

---

| Option | Description | Selected |
|--------|-------------|----------|
| Fix as you go | Test area → fix → confirm → next area | ✓ |
| Test all first, then batch-fix | Complete all audits, then fix in one pass | |

**User's choice:** Fix as you go (interleaved)

---

## Test Coverage for Fixes

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — add tests for each fix | Unit test per code change using httpmock | ✓ |
| Only if fix changes parsing logic | Tests only for deserialization changes | |
| No — COMPAT-9213.md is the test record | Manual validation only | |

**User's choice:** Add unit tests for every fix

---

| Option | Description | Selected |
|--------|-------------|----------|
| Update existing tests to work with both shapes | Compatible shape in existing mocks | ✓ |
| Keep old tests, add new 9.2.13-specific tests | Separate test suites | |
| Replace with the most restrictive shape | Update all tests to 9.2.13 shape | |

**User's choice:** Update existing tests to the compatible shape

---

| Option | Description | Selected |
|--------|-------------|----------|
| All existing tests pass + new tests per fix | 235 green + 1 new per fix | ✓ |
| Existing tests pass only | No new tests required | |
| Set a specific target | Define a minimum count | |

**User's choice:** All 235 existing tests pass + at least one new test per code fix

---

## Claude's Discretion

- Specific response-shape differences between 9.2.13 and 9.2.19 unknown until testing — researcher identifies likely candidates from changelogs
- Exact column widths and wording in COMPAT-9213.md rows determined during final write step
- Decision ID continuation: Phase 4 ended at D-56; Phase 5 starts at D-57

## Deferred Ideas

None — discussion stayed within phase scope.
