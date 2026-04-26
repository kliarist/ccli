---
phase: 01-foundation
plan: "05"
subsystem: cli-init
tags:
  - rust
  - cli
  - dialoguer
  - init
  - foundation
  - tdd

dependency_graph:
  requires:
    - "01-01: AppError enum (Auth/Network/Config/Api variants), tokio async entry point"
    - "01-02: config::load(), config::save(), Config struct"
    - "01-03: OutputConfig, Cli::output_config() helper"
    - "01-04: api::test_connection(base_url, token), Cli struct, Commands::Init"
  provides:
    - "src/cli/init.rs: run() handler for ccli init interactive flow"
    - "mask_pat_hint() visual hint helper (first2+*****+last3 for tokens >=6 chars)"
    - "validate_url() scheme validator (requires http:// or https://)"
    - "pub mod init; declaration in cli/mod.rs"
    - "Commands::Init dispatch in main.rs wired to cli::init::run(&cli).await"
  affects:
    - "INIT-01 closure (pending human-verify checkpoint Task 5.2)"
    - "Phase 2+ command handlers — ccli init is now the entry point for fresh installs"

tech_stack:
  added: []
  patterns:
    - "dialoguer 0.12 ColorfulTheme + Input::with_theme + Password::with_theme"
    - "with_initial_text for D-03 pre-fill on re-run"
    - "validate_with closure delegates to pub(crate) validate_url (testable without TTY)"
    - "allow_empty_password(true) + empty-string-fallback for Pitfall 4 handling"
    - "api::test_connection called BEFORE config::save (D-02 order enforced in code)"
    - "mask_pat_hint: token[..2] + ***** + token[len-3..] for tokens >=6; full-mask for shorter"

key_files:
  created:
    - src/cli/init.rs
  modified:
    - src/cli/mod.rs
    - src/main.rs

decisions:
  - "validate_url extracted as pub(crate) fn so it can be unit-tested without spinning up an interactive TTY (dialoguer requires a terminal for Input::interact_text)"
  - "mask_pat_hint uses char count for short-token detection but byte indexing for slicing — safe because Confluence PATs are ASCII"
  - "Empty password on first run returns explicit anyhow error before reaching test_connection — prevents testing empty string against API (T-05-07 mitigation)"
  - "run() handler takes _cli: &Cli (currently unused) for symmetry with future command handlers that need output flag access"

metrics:
  duration: "~17 minutes"
  completed: "2026-04-26T20:25:57Z"
  tasks_completed: 1
  tasks_pending_human_verify: 1
  files_created: 1
  files_modified: 2
  tests_added: 11
---

# Phase 1 Plan 05: ccli init Interactive Setup Flow Summary

**One-liner:** dialoguer-powered ccli init handler with pre-fill on re-run, masked PAT hint, URL scheme validation, connection test before save, and 11 unit tests covering mask_pat_hint and validate_url helpers.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 5.1 | Implement cli/init.rs, declare pub mod init, wire main.rs dispatch | d11a393 | src/cli/init.rs, src/cli/mod.rs, src/main.rs |

## Tasks Pending Human Verification

| Task | Name | Status |
|------|------|--------|
| 5.2 | Human-verify live ccli init flow against real Confluence DC instance | CHECKPOINT — awaiting human |

## Public API

### src/cli/init.rs

```rust
/// Top-level handler for `ccli init`.
pub async fn run(_cli: &Cli) -> anyhow::Result<()>;

/// Validate the user-entered Confluence URL (scheme check only).
/// Returns Ok(()) if starts with http:// or https://; Err(&str) otherwise.
pub(crate) fn validate_url(s: &str) -> Result<(), &'static str>;

/// Produce a non-revealing visual hint of an existing PAT.
/// Format: first2+*****+last3 for tokens >=6 chars; full mask for shorter.
pub(crate) fn mask_pat_hint(token: &str) -> String;
```

## ccli init State Machine

```
config::load()
  |
  v
URL prompt (Input::with_theme)
  - with_initial_text(existing.url or "")   [D-03 pre-fill]
  - validate_with(validate_url)              [T-05-04 + T-04-04]
  |
  v
PAT prompt (Password::with_theme)
  - prompt shows mask_pat_hint(existing.token) if re-run  [D-03 hint]
  - allow_empty_password(true)                            [Pitfall 4]
  |
  v
Empty PAT resolution:
  - empty + existing → keep existing token
  - empty + no existing → anyhow error (T-05-07)
  - non-empty → use entered value
  |
  v
api::test_connection(&url, &token).await    [D-02 pre-save validation]
  - failure → surface AppError through anyhow chain → main.rs two-line error
  |
  v
config::save(&Config { url, token })        [D-02 save only on success]
  |
  v
println!("Connected as {} — configuration saved.", display_name)
```

## mask_pat_hint Specification

| Input length | Behavior | Example |
|---|---|---|
| 0 | Return "" (empty) | "" → "" |
| 1-5 | Return N stars (full mask, no char revealed) | "short" → "*****" |
| 6+ | Return first2 + "*****" + last3 | "ABCDEF" → "AB*****DEF" |

Property invariant: `mask_pat_hint(token) != token` for any non-empty token (enforced by test).

## Pitfall 4 Handling

`Password::interact()` returns an empty string when the user presses Enter without typing anything. The handler resolves empty PAT as follows:

- **Re-run (existing config present):** Empty input → keep `existing.token.clone()`. The user pressing Enter confirms they want to reuse their saved token without re-entering it.
- **First run (no existing config):** Empty input → `anyhow::anyhow!("A Personal Access Token is required on first run.")` — surfaces through the two-line error handler in main.rs as `AppError::Api`.

## Test Coverage

| Test | Behavior |
|------|----------|
| mask_pat_hint_long_token_first2_stars_last3 | "ATATT3xFfGF0abc123" → "AT*****123" |
| mask_pat_hint_short_token_full_mask | "short" → "*****" |
| mask_pat_hint_empty_input_empty_output | "" → "" |
| mask_pat_hint_two_chars_two_stars | "AB" → "**" |
| mask_pat_hint_six_chars_first2_stars_last3 | "ABCDEF" → "AB*****DEF" |
| mask_pat_hint_never_contains_full_token | property test across 7 inputs |
| validate_url_https_ok | https://example.com → Ok(()) |
| validate_url_http_ok | http://example.com → Ok(()) |
| validate_url_ftp_rejected | ftp://example.com → Err (contains "http://") |
| validate_url_no_scheme_rejected | example.com → Err |
| validate_url_empty_rejected | "" → Err |

**Total: 11 unit tests.** The run() handler is NOT unit-tested — dialoguer requires a TTY.

**Cumulative suite: 80 tests pass (69 prior + 11 new), 0 failed, 0 regressions.**

NOTE: The full `run()` handler is verified by the human-verify checkpoint (Task 5.2) which must be completed against a real Confluence DC instance.

## Linkage to Other Plans

| Dependency | Plan | What's Used |
|---|---|---|
| config::load() | Plan 02 | Pre-fill source for URL + PAT on re-run |
| config::save(&Config) | Plan 02 | Atomic write + 0o600 perms after connection success |
| api::test_connection() | Plan 04 | PAT validation before save (D-02) |
| AppError chain walk | Plan 01 + Plan 04 | Two-line error in main.rs (D-04) |
| Cli struct | Plan 04 | Handler signature `_cli: &Cli` |

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None. All implemented functions are fully wired. The run() handler is complete; it cannot be unit-tested (requires a TTY) but is verified end-to-end via the human-verify checkpoint.

## Threat Surface Scan

All mitigations from the plan's threat_model are implemented:

| Threat ID | Mitigation | Status |
|---|---|---|
| T-05-01 | mask_pat_hint: only first2+last3 exposed; ≤5 char tokens fully masked; property test | Implemented |
| T-05-02 | Password::with_theme().interact() uses dialoguer echo-off masking | Implemented |
| T-05-03 | println! uses display_name (from API), not token | Implemented |
| T-05-04 | validate_with(validate_url) enforces http:// or https:// at prompt layer | Implemented |
| T-05-05 | config::save called only after test_connection succeeds (code order enforced) | Implemented |
| T-05-06 | No tracing calls in cli::init; test_connection is #[instrument(skip(token))] (Plan 04) | Inherited |
| T-05-07 | Empty PAT on first run returns error before test_connection("") | Implemented |
| T-05-08 | config::save uses atomic write (Plan 02 delegates) | Inherited |
| T-05-09 | RUST_LOG=debug: displayName may appear (acceptable); token does not (skip attribute) | Accepted |

No new threat surface introduced beyond what was in the plan's threat model.

## Human-Verify Checkpoint (Task 5.2) — PENDING

INIT-01 closure requires a human to run the six-phase verification protocol against a real Confluence DC 9.x instance:

- **Phase A:** First-run happy path — URL prompt no pre-fill, PAT prompt no hint, `Connected as <name>` success, config file at `~/.config/ccli/config.toml` with `-rw-------` permissions
- **Phase B:** Re-run with pre-fill — URL pre-filled, PAT hint shows masked form, Enter keeps token
- **Phase C:** Invalid PAT failure path — two-line stderr error, config NOT overwritten
- **Phase D:** Invalid URL failure path — inline rejection of `not-a-url`, two-line network error
- **Phase E:** `RUST_LOG=debug` — PAT string does NOT appear in debug output
- **Phase F:** Env var override — `CCLI_TOKEN=wrong ./ccli init` uses override and shows Auth error

Binary: `cargo build --release && ./target/release/ccli init`

## Self-Check: PASSED

Files exist:
- FOUND: src/cli/init.rs
- FOUND: src/cli/mod.rs (modified — pub mod init added)
- FOUND: src/main.rs (modified — Commands::Init wired to cli::init::run)

Commits exist:
- FOUND: d11a393 (feat(01-05): implement ccli init interactive setup flow)

Test results:
- cli::init::tests: 11 passed, 0 failed
- Full suite: 80 passed, 0 failed, 0 regressions
