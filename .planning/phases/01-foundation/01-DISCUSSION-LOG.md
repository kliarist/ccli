# Phase 1: Foundation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-24
**Phase:** 01-foundation
**Areas discussed:** ccli init flow, Error message style, Output table behavior, Async architecture

---

## ccli init flow

### Prompt style

| Option | Description | Selected |
|--------|-------------|----------|
| dialoguer crate | Styled prompts — colored labels, inline validation, masked PAT field | ✓ |
| Plain readline prompts | Simple 'Enter URL:' via stdin, no styling | |

**User's choice:** dialoguer crate
**Notes:** Standard approach for Rust CLIs; matches expected quality bar.

---

### Connection test on save

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — test and confirm | Lightweight API call before writing config; show success or clear error | ✓ |
| No — save immediately | Write config on entry, discover auth problems later | |
| Yes — test, but allow saving anyway | Test + show result, let user save regardless | |

**User's choice:** Yes — test and confirm
**Notes:** Connection validation before saving provides immediate feedback on bad tokens/URLs.

---

### Re-run behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Pre-fill with current values | Show existing URL + masked PAT hint; Enter to keep | ✓ |
| Prompt fresh, overwrite on confirm | Fresh prompts + 'Config exists — overwrite?' gate | |
| Error: already configured | Fail with --force message | |

**User's choice:** Pre-fill with current values
**Notes:** Makes token rotation frictionless — most common re-run scenario.

---

## Error message style

### Error format

| Option | Description | Selected |
|--------|-------------|----------|
| Error + hint line | Two-line: 'Error: msg' + 'Run ccli init...' on stderr | ✓ |
| Error + hint + detail | Three parts including HTTP status/request ID | |
| Just the message | Single line, no hint | |

**User's choice:** Error + hint line
**Notes:** Matches jira-cli style — compact and actionable.

---

### Debug verbosity

| Option | Description | Selected |
|--------|-------------|----------|
| RUST_LOG env var | Standard Rust tracing/env_logger; `RUST_LOG=debug ccli ...` | ✓ |
| Explicit --debug flag | More discoverable for non-Rust users | |
| No debug mode | Keep simple, add later | |

**User's choice:** RUST_LOG env var
**Notes:** Idiomatic Rust; no additional CLI surface needed.

---

## Output table behavior

### TTY auto-detection

| Option | Description | Selected |
|--------|-------------|----------|
| TTY auto-detect | Formatted table in terminal; plain tab-separated when piped | ✓ |
| Always formatted unless --plain | Formatted always; user must pass --plain | |

**User's choice:** TTY auto-detect
**Notes:** Matches jira-cli behavior; makes piping work without extra flags.

---

### --columns flag format

| Option | Description | Selected |
|--------|-------------|----------|
| Comma-separated names | `--columns key,name,status` — shell-friendly | ✓ |
| Numbered positions | `--columns 1,3,4` — brittle if column order changes | |
| You decide | Leave to Claude | |

**User's choice:** Comma-separated names
**Notes:** Matches jira-cli convention; clear and memorable.

---

## Async architecture

### Runtime choice

| Option | Description | Selected |
|--------|-------------|----------|
| tokio + async from Phase 1 | Wire up tokio/reqwest now; Phase 2 TUI requires it | ✓ |
| Sync HTTP in Phase 1, switch in Phase 2 | ureq now, refactor later | |

**User's choice:** tokio + async from Phase 1
**Notes:** Avoids a disruptive refactor at Phase 2 boundary when Ratatui TUI is introduced.

---

### Config profile support

| Option | Description | Selected |
|--------|-------------|----------|
| Single profile for v1 | One URL + one PAT; simple and clean | ✓ |
| Multi-profile from day 1 | Named profiles with --profile flag | |

**User's choice:** Single profile for v1
**Notes:** YAGNI — multi-profile can be a v2 schema evolution if the need arises.

---

## Claude's Discretion

- Table formatting library (comfy-table, tabled, prettytable-rs) — Claude picks the most actively maintained
- Project structure (crate layout, module organization) — Claude designs for clean separation
- Default columns per resource type — Claude defines sensible defaults

## Deferred Ideas

None.
