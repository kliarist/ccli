---
phase: 01-foundation
verified: 2026-04-26T21:00:00Z
status: human_needed
score: 8/9 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Run ccli init end-to-end (six-phase protocol from Plan 05 Task 5.2)"
    expected: "Phase A: connected as <user>, config saved at ~/.config/ccli/config.toml with -rw------- permissions. Phase B: URL pre-filled on re-run, masked PAT hint shown, Enter keeps token. Phase C: two-line stderr error on bad PAT, config NOT overwritten. Phase D: inline rejection of 'not-a-url', two-line network error for unreachable host. Phase E: PAT string absent from RUST_LOG=debug output. Phase F: CCLI_TOKEN env var override causes Auth error with wrong token."
    why_human: "dialoguer prompts require an interactive TTY. Automated mocks cover the helper functions (mask_pat_hint, validate_url) but cannot drive the full interactive flow. INIT-01 closure requires live Confluence DC 9.x instance verification."
---

# Phase 1: Foundation Verification Report

**Phase Goal:** Establish the complete Rust CLI foundation — a compilable, testable binary skeleton with working config management, output formatting, Confluence API client, and the ccli init interactive setup command.
**Verified:** 2026-04-26T21:00:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Project compiles with `cargo build` and produces a binary named `ccli` | VERIFIED | `cargo build` exits 0; `target/debug/ccli` exists (29 MB); `--help` prints clap-generated text showing `ccli` name |
| 2 | `ccli --help` prints clap-generated help text with all global flags and `init` subcommand | VERIFIED | Binary output confirmed: `--plain`, `--no-headers`, `--columns <COLS>`, and `init` subcommand all listed |
| 3 | AppError variants can be matched to produce remediation hints (INIT-04) | VERIFIED | `hint_for()` pure function at `src/main.rs:45`; all 4 variants covered; 4 unit tests pass |
| 4 | tracing-subscriber reads RUST_LOG env var and defaults to `warn` | VERIFIED | `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))` at `src/main.rs:16-18` |
| 5 | main.rs walks the anyhow::Error chain to recover inner AppError through context wrappers (B-02) | VERIFIED | `err.chain().find_map(|e| e.downcast_ref::<AppError>()).cloned()` at `src/main.rs:25`; verified by `chain_walk_recovers_inner_app_error_through_context_wrapper` test |
| 6 | Config persisted to `~/.config/ccli/config.toml` with atomic write and 0o600 permissions (INIT-02) | VERIFIED | `config_path()` uses `home::home_dir()` → `.config/ccli/config.toml`; `save_to()` uses tmp+rename; `fs::set_permissions(0o600)` on Unix; 3 path tests + `save_to_sets_0600_permissions_on_unix` test pass |
| 7 | CCLI_URL / CCLI_TOKEN env vars override config values; env-only synthesis when no file exists (INIT-03) | VERIFIED | `load_from()` reads both env vars; env-only branch at `src/config/mod.rs:47-51`; 5 tests including `ccli_url_env_var_overrides_file_value`, `load_from_env_only_synthesizes_config_when_both_vars_set_and_file_missing` pass |
| 8 | OutputFormatter dispatches: TTY → aligned table, piped/--plain → TSV; --no-headers suppresses header; --columns selects columns (OUT-01..04) | VERIFIED | TTY dispatch `if self.is_tty && !self.config.plain` at `src/output/mod.rs:59`; 4-case dispatch matrix tests pass; UTF8_FULL_CONDENSED table and bare `\t`-join TSV both verified; 23 output tests total |
| 9 | `ccli init` interactive flow: prompts for URL and PAT, validates connection, writes config only on success (INIT-01) | NEEDS HUMAN | `src/cli/init.rs` wired: `api::test_connection` called at line 68, `config::save` called at line 77 (after test_connection); `validate_url` and `mask_pat_hint` unit tests (11) pass; full interactive run requires human checkpoint (Task 5.2) |

**Score:** 8/9 truths verified (1 requires human verification)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` | Crate manifest with 14 pinned Phase 1 dependencies | VERIFIED | `name = "ccli"`, `edition = "2021"`, `[[bin]] name = "ccli"`, all 14 deps present; `tempfile = "3"` and `httpmock = "0.7"` in `[dev-dependencies]`; forbidden deps (`dirs`, `atty`, `prettytable-rs`) absent |
| `src/main.rs` | Async tokio entry point, tracing init, error dispatch via anyhow chain walk | VERIFIED | `#[tokio::main]`, `tracing_subscriber::fmt()`, `EnvFilter::new("warn")`, `err.chain().find_map(...)`, `hint_for()`, `handle_error()`, `Cli::parse()` dispatch; 6 unit tests |
| `src/api/error.rs` | AppError enum (Auth/Network/Config/Api) with Clone derive | VERIFIED | `#[derive(Error, Debug, Clone)]`; all 4 variants with correct display strings; 6 unit tests |
| `src/api/mod.rs` | Module re-exports: `pub mod client`, `pub mod error`, re-exports of Client/test_connection/AppError | VERIFIED | All re-exports present |
| `src/api/client.rs` | Client struct with reqwest, test_connection, CCLI_INSECURE, instrument(skip(token)) | VERIFIED | All present; `#[instrument(skip(token))]` at line 96; `danger_accept_invalid_certs` at line 81; `/rest/api/user/current`; 12 tests including W-04 Pitfall 2 cases |
| `src/config/path.rs` | `config_path()` resolving to `~/.config/ccli/config.toml` via `home::home_dir()` | VERIFIED | `home::home_dir()` used; no `dirs::` reference; 3 unit tests all pass |
| `src/config/mod.rs` | Config struct, load/save/load_or_error + pub(crate) testable helpers, env-only synthesis | VERIFIED | All public and pub(crate) functions present; atomic write; 0o600; env vars; 11 unit tests pass |
| `src/output/mod.rs` | OutputConfig, OutputFormatter, parse_columns, TTY-aware dispatch, print_to helper | VERIFIED | All present; `is_terminal::IsTerminal`; dispatch matrix; 13 tests |
| `src/output/table.rs` | comfy-table renderer using UTF8_FULL_CONDENSED + Dynamic arrangement | VERIFIED | `UTF8_FULL_CONDENSED`, `ContentArrangement::Dynamic`; `render_string()` testable variant; 4 tests |
| `src/output/tsv.rs` | Bare TSV renderer using `fields.join("\t")` — no comfy-table | VERIFIED | No `comfy_table` import; `join("\t")` present; `render_to()` testable variant; 6 tests |
| `src/cli/mod.rs` | Cli (clap Parser), Commands enum (Init), global output flags, output_config helper | VERIFIED | `#[derive(Parser)]`, `Commands::Init`, `global = true` on all 3 flags, `pub fn output_config()`; 8 tests |
| `src/cli/init.rs` | run() handler, mask_pat_hint, validate_url, wired to main.rs | VERIFIED | `pub async fn run`, `mask_pat_hint`, `validate_url`, `api::test_connection` before `config::save`; `allow_empty_password(true)`; `with_initial_text`; 11 tests |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/main.rs` | `src/api/error.rs` | `use api::error::AppError`; chain walk | WIRED | `use api::error::AppError` at line 8; `err.chain().find_map(|e| e.downcast_ref::<AppError>())` at line 25 |
| `src/main.rs` | `tracing-subscriber` | `tracing_subscriber::fmt()` | WIRED | Line 14; EnvFilter at lines 16-18 |
| `src/main.rs` | `src/cli/mod.rs` | `Cli::parse()`, `match cli.command` | WIRED | `use cli::{Cli, Commands}` line 9; `<Cli as clap::Parser>::parse()` line 34; `Commands::Init =>` line 37 |
| `src/main.rs` | `src/cli/init.rs` | `cli::init::run(&cli).await` | WIRED | Line 37: `Commands::Init => cli::init::run(&cli).await` |
| `src/config/mod.rs` | `src/config/path.rs` | `use path::config_path` | WIRED | Line 10: `use path::config_path`; called in `load()` and `save()` |
| `src/config/mod.rs` | `std::env::var` | CCLI_URL / CCLI_TOKEN env lookup in `load_from()` | WIRED | Lines 42-43: both env vars read with filter for non-empty |
| `src/config/mod.rs` | `src/api/error.rs` | `use crate::api::error::AppError` in `load_or_error` | WIRED | Line 9: `use crate::api::error::AppError`; used at lines 100, 109, 112 |
| `src/output/mod.rs` | `src/output/table.rs` | `table::render_string()` called when is_tty && !plain | WIRED | Line 61: `table::render_string(...)` in TTY branch |
| `src/output/mod.rs` | `src/output/tsv.rs` | `tsv::render_to()` called when !is_tty || plain | WIRED | Line 65: `tsv::render_to(...)` in piped/plain branch |
| `src/output/mod.rs` | `is-terminal crate` | `std::io::stdout().is_terminal()` in `OutputFormatter::new` | WIRED | Line 12: `use is_terminal::IsTerminal`; line 30: `.is_terminal()` |
| `src/cli/mod.rs` | `src/output/mod.rs` | `use crate::output::{parse_columns, OutputConfig}` | WIRED | Line 9: import; `output_config()` uses both at lines 52-56 |
| `src/api/client.rs` | `src/api/error.rs` | `use crate::api::error::AppError` | WIRED | Line 22; AppError::Auth/Network/Api all used in `test_connection` |
| `src/api/client.rs` | `src/config/mod.rs` | `use crate::config::Config`; `Client::new(&Config)` | WIRED | Line 23; `Client::new(config: &Config)` line 35 |
| `src/api/client.rs` | `reqwest::Client::builder` | `ReqwestClient::builder().default_headers(...).timeout(...)` | WIRED | Lines 76-86 in `build_reqwest_client()` |
| `src/cli/init.rs` | `dialoguer::{Input, Password}` | `Input::with_theme + Password::with_theme` | WIRED | Lines 29, 49 |
| `src/cli/init.rs` | `src/api/client.rs` | `api::test_connection(&url, &token).await` | WIRED | Line 68 |
| `src/cli/init.rs` | `src/config/mod.rs` | `config::load()` and `config::save(&cfg)` | WIRED | Lines 25, 77 |

### Data-Flow Trace (Level 4)

Not applicable for Phase 1 foundation code — the output subsystem is a formatting library (no DB queries), the config subsystem reads from the filesystem (not rendering dynamic external data), and `cli::init` writes to config only after successful connection. No component renders dynamic data from an async source in a way that could be hollow-propped. API connectivity is verified by httpmock integration tests.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Binary compiles and runs | `cargo build` | exit 0, `target/debug/ccli` exists | PASS |
| `--help` shows all flags | `ccli --help` | Lists `--plain`, `--no-headers`, `--columns`, `init` subcommand | PASS |
| All 80 tests pass | `cargo test --bin ccli` | `80 passed; 0 failed` | PASS |
| No forbidden deps | grep Cargo.toml | No `dirs`, `atty`, `prettytable-rs` | PASS |
| PAT not logged in debug! | grep `debug!` in client.rs | `debug!("PAT validation probe: GET {}", url)` — logs URL only | PASS |
| tsv.rs has no comfy_table import | grep tsv.rs | Absent | PASS |
| `ccli init` runs without segfault (no TTY) | Step 7b SKIPPED | Binary requires TTY for dialoguer prompts; human-verify checkpoint gates this | SKIP |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| INIT-01 | Plan 05 | User can run `ccli init` to interactively configure URL and PAT | NEEDS HUMAN | Code wired and unit tests pass; interactive flow requires human verification (Task 5.2) |
| INIT-02 | Plan 02 | Config persisted to `~/.config/ccli/config.toml` | SATISFIED | `config_path()` always returns `~/.config/ccli/config.toml`; atomic save verified by unit tests |
| INIT-03 | Plan 02 | CCLI_URL / CCLI_TOKEN env var override | SATISFIED | `load_from()` reads both env vars; env-only synthesis when no file; 5 tests covering overlay and synthesis |
| INIT-04 | Plans 01, 04 | Clear error with remediation hint | SATISFIED | `hint_for()` covers all 4 AppError variants; `handle_error()` emits two-line stderr; anyhow chain walk recovers through context wrappers |
| OUT-01 | Plan 03 | Aligned table when stdout is TTY (non-interactive in TUI sense) | SATISFIED | `is_tty && !plain` dispatches to `table::render_string()`; UTF8_FULL_CONDENSED + Dynamic; TTY dispatch test confirms box-drawing chars present |
| OUT-02 | Plan 03 | `--plain` outputs tab-separated values | SATISFIED | `plain=true` dispatches to `tsv::render_to()`; TSV dispatch tests confirm no box-drawing chars; piped (is_tty=false) also routes to TSV |
| OUT-03 | Plan 03 | `--no-headers` suppresses column header row | SATISFIED | `no_headers` passed to both table and TSV renderers; `no_headers_omits_header_text` and `no_headers_suppresses_header_line` tests pass |
| OUT-04 | Plan 03 | `--columns` selects columns in output | SATISFIED | `resolve_columns()` filters + reorders; `parse_columns()` parses comma-separated names; case-insensitive lookup; 3 resolve_columns tests + 5 parse_columns tests pass |

**Note on OUT-01:** REQUIREMENTS.md states "Non-interactive (piped) output defaults to a formatted table." This wording is ambiguous but CONTEXT.md D-06 locks the intended behavior as "TTY → table, piped → TSV." The ROADMAP SC 4 ("Non-interactive output from any ccli command formats as an aligned table") confirms "non-interactive" means "non-TUI CLI commands" (vs the Phase 2 Ratatui TUI), not "piped stdout." The implementation follows the CONTEXT.md D-06 decision and the PLAN.md's explicit truth statement for OUT-01. No gap.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/api/client.rs` | 100 | `debug!(... url)` with comment "// never log the token value" | Info | Comment-only; actual log line logs URL, not token. `#[instrument(skip(token))]` at line 96 provides the real protection. No impact. |
| `src/output/table.rs` | 37 | `row[i].as_str()` — raw index, no bounds check | Warning (pre-existing per REVIEW.md WR-01) | Panics if a caller passes rows with fewer columns than headers. Not a stub; this is a latent correctness issue for Phase 2+ callers. No Phase 1 commands produce mismatched rows. |
| `src/output/tsv.rs` | 40 | `row[i].as_str()` — same as above | Warning (pre-existing per REVIEW.md) | Same latent panic risk as table.rs. No Phase 1 impact. |

No stubs found. No TODO/FIXME/placeholder patterns. No empty implementations (`return null`, `return {}`, `return []`). All data flows through real logic: config reads from filesystem, API calls hit real endpoints (tested with httpmock), output rendering uses real comfy-table or tab-join.

### Human Verification Required

#### 1. ccli init Live Interactive Flow (INIT-01)

**Test:** Follow the six-phase verification protocol from Plan 05 Task 5.2:
- **Phase A — Fresh first run:** Remove `~/.config/ccli/config.toml`. Run `./target/release/ccli init`. Verify URL prompt has no pre-fill; PAT prompt has no hint; output ends with `Connected as <YourDisplayName> — configuration saved.`; exit code 0; file exists at `~/.config/ccli/config.toml` with `-rw-------` (0600) permissions.
- **Phase B — Re-run with pre-fill:** Run again. Verify URL is pre-filled. Verify PAT prompt shows `[current: AT*****xyz]` masked hint. Press Enter on both; confirm reconnects and saves successfully.
- **Phase C — Invalid PAT failure:** Enter a wrong PAT. Confirm two-line stderr: `Error: Connection test failed: Authentication failed: Invalid or expired token. Check your PAT.` / `Run 'ccli init' to reconfigure your credentials.`; exit code 1; config NOT overwritten with bad token.
- **Phase D — Invalid URL failure:** Enter `not-a-url` at the URL prompt. Confirm dialoguer rejects it inline with an error about `http://` or `https://`. Enter `https://invalid.invalid.invalid`; confirm two-line Network error.
- **Phase E — RUST_LOG=debug PAT protection:** Run with `RUST_LOG=debug`. Confirm `PAT validation probe: GET <url>/rest/api/user/current` appears in output; confirm the literal PAT string does NOT appear anywhere in debug output.
- **Phase F — Env var override:** Run with `CCLI_TOKEN=clearly-wrong`. Confirm the env override token is used and triggers an Auth error.

**Expected:** All six phases pass; user types "approved."

**Why human:** dialoguer `Input::interact_text()` and `Password::interact()` require an interactive TTY with echo-off masking. This cannot be driven by automated tests without pulling in a PTY simulation library. The helper functions (`mask_pat_hint`, `validate_url`) are unit-tested; the end-to-end terminal UX is not.

---

### Gaps Summary

No implementation gaps found. All 8 automatically-verifiable truths are satisfied:

- The binary compiles, links, and produces a working `ccli` binary.
- All 80 unit tests pass, covering AppError display and chain walk, config path/load/save/env-var behavior, output TTY dispatch and column filtering, CLI flag parsing, API client construction and all HTTP status codes, and init helper functions.
- Every key link is wired: main.rs → cli → init → config + api; output mod → table + tsv; config mod → path + AppError.
- No forbidden dependencies. No stub patterns. No TODO markers.

The only pending item is the human-verify checkpoint for INIT-01's live interactive flow (Plan 05 Task 5.2). This was explicitly marked `autonomous: false` in the plan and designates a blocking human gate. All automated evidence strongly suggests the flow will work correctly.

---

_Verified: 2026-04-26T21:00:00Z_
_Verifier: Claude (gsd-verifier)_
