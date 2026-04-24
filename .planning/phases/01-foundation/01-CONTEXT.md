# Phase 1: Foundation - Context

**Gathered:** 2026-04-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Set up the Rust project skeleton, implement the Confluence Data Center API client with PAT authentication, build the `ccli init` interactive setup flow, persist config to `~/.config/ccli/config.toml`, support `CCLI_URL`/`CCLI_TOKEN` env var overrides, and deliver all output formatting flags (`--plain`, `--no-headers`, `--columns`).

Requirements: INIT-01, INIT-02, INIT-03, INIT-04, OUT-01, OUT-02, OUT-03, OUT-04.

</domain>

<decisions>
## Implementation Decisions

### ccli init Flow

- **D-01:** Use the **dialoguer** crate for interactive prompts — colored labels, input validation inline, masked PAT field.
- **D-02:** On `ccli init` completion, **test the connection** (lightweight API call, e.g., `GET /rest/api/user/current`) before writing config. Show a clear success confirmation or error. Do not save if the test fails.
- **D-03:** On re-run with an existing config, **pre-fill prompts with current values** — show the existing URL, show a masked PAT hint (e.g., `AT*****xyz`). User can press Enter to keep or type to replace. This makes token rotation frictionless without requiring --force.

### Error Message Style

- **D-04:** Error format is **two-line**: `Error: <message>` on stderr, followed by a remediation hint line (e.g., `Run 'ccli init' to reconfigure.`). Matches jira-cli style — compact and actionable.
- **D-05:** Debug verbosity via **RUST_LOG env var** (standard Rust `tracing`/`env_logger` approach). `RUST_LOG=debug ccli ...` surfaces HTTP request/response details and stack context. No separate --debug flag in Phase 1.

### Output Table Behavior

- **D-06:** **TTY auto-detect**: when stdout is a TTY, format as aligned table; when stdout is piped, output tab-separated rows automatically (no --plain needed). `--plain` is still accepted explicitly for scripts that want to be unambiguous. Matches jira-cli behavior.
- **D-07:** `--columns` takes **comma-separated column names** (e.g., `--columns key,name,status`). Shell-friendly, matches jira-cli convention.

### Async Architecture

- **D-08:** Use **tokio + reqwest (async)** from Phase 1. The Ratatui TUI in Phase 2 requires an async runtime — starting with tokio now avoids a disruptive refactor at Phase 2 boundary.
- **D-09:** Config supports a **single profile** (one URL + one PAT). No multi-profile support in v1. Schema kept flat and simple; multi-profile can be added as a schema evolution post-v1.

### Claude's Discretion

- Table formatting library choice (comfy-table, tabled, prettytable-rs) — pick the most actively maintained with good Unicode/width support.
- Project structure (single crate vs. workspace, module layout for `api/`, `config/`, `output/`) — structure for clean separation, no strong user preference.
- Default columns per resource type — design sensible defaults; user didn't specify.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Reference Implementation
- `https://github.com/ankitpokhrel/jira-cli` — UX patterns to follow: command structure (`ccli page list`), output flags (`--plain`, `--no-headers`, `--columns`), TUI layout (list + preview pane). See PROJECT.md §Context.

### Target API
- Confluence Data Center REST API v1: `https://docs.atlassian.com/ConfluenceServer/rest/latest/` — primary API surface for Phase 1 auth validation (`/rest/api/user/current`)
- Confluence Data Center REST API v2: available in DC 9.2.19 alongside v1

### Requirements & Planning Docs
- `.planning/REQUIREMENTS.md` — INIT-01 through INIT-04, OUT-01 through OUT-04 (Phase 1 requirements with acceptance criteria)
- `.planning/PROJECT.md` — Key decisions (Rust, PAT-only auth, raw XML editing, jira-cli UX model), constraints, out-of-scope items

### Key Libraries (Rust ecosystem)
- `dialoguer` crate — interactive prompts with masking (chosen for ccli init)
- `reqwest` + `tokio` — async HTTP client and runtime (chosen)
- `tracing` / `env_logger` — RUST_LOG-based debug verbosity (chosen)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — blank project; everything is greenfield.

### Established Patterns
- No existing patterns to carry forward. Phase 1 establishes the baseline patterns all subsequent phases will follow.

### Integration Points
- `~/.config/ccli/config.toml` — config file path established in Phase 1; all later phases read from here.
- `CCLI_URL` / `CCLI_TOKEN` env vars — override mechanism defined in Phase 1; later phases inherit.
- API client module — all Phase 2+ features build on the HTTP client and auth layer from Phase 1.

</code_context>

<specifics>
## Specific Ideas

- jira-cli is the explicit UX reference — follow its command structure (`ccli page list`, `ccli space list`) and flag conventions faithfully.
- Binary name: `ccli` — short, mirrors `jira` from jira-cli.
- Config path: `~/.config/ccli/config.toml` — explicitly specified, do not deviate.
- Auth validation call: `GET /rest/api/user/current` or equivalent lightweight endpoint — just enough to confirm the PAT works without side effects.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-foundation*
*Context gathered: 2026-04-24*
