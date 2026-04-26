---
phase: 01-foundation
plan: "01"
subsystem: core
tags:
  - rust
  - cli
  - foundation
  - error-handling
  - tdd

dependency_graph:
  requires: []
  provides:
    - AppError enum (Auth/Network/Config/Api variants with Display + Clone)
    - hint_for() pure mapper function
    - handle_error() two-line stderr handler
    - tokio async entry point
    - RUST_LOG-driven tracing initialization
    - Cargo.toml with 14 pinned Phase 1 dependencies
  affects:
    - All downstream Phase 1 plans (01-02 through 01-05) — import AppError and Cargo.toml

tech_stack:
  added:
    - tokio 1 (async runtime, full features)
    - clap 4.6 (CLI arg parsing, derive feature)
    - reqwest 0.13 (HTTP client, json feature)
    - serde 1 + serde_json 1 (serialization)
    - toml 1 (config file format)
    - anyhow 1 (error propagation)
    - thiserror 2 (typed error definitions)
    - dialoguer 0.12 (interactive prompts, password feature)
    - comfy-table 7 (terminal table rendering)
    - is-terminal 0.4 (TTY detection, RUSTSEC-safe atty replacement)
    - tracing 0.1 + tracing-subscriber 0.3 (structured logging, env-filter)
    - home 0.5 (home directory resolution)
  patterns:
    - thiserror derive for typed AppError enum
    - anyhow chain walk for error category recovery through .context() wrappers
    - Pure hint_for() mapper enabling unit-testable category dispatch
    - RUST_LOG env-driven tracing with warn default

key_files:
  created:
    - Cargo.toml
    - Cargo.lock
    - .gitignore
    - src/main.rs
    - src/api/error.rs
    - src/api/mod.rs
  modified: []

decisions:
  - "Use thiserror 2.x derive for AppError — enables match dispatch and unit-testable Display strings"
  - "Derive Clone on AppError — required for .cloned() on borrowed &AppError from anyhow chain walk (B-02 fix)"
  - "hint_for() extracted as pure function — enables unit testing of category dispatch without stderr capture (B-03 fix)"
  - "anyhow chain walk (err.chain().find_map()) instead of direct downcast — prevents .context() wrappers from defeating category dispatch (B-02 fix)"
  - "Binary exits 0 with no-op run() for Plan 01 skeleton — Plan 04 wires Cli::parse(), Plan 05 dispatches to init handler"

metrics:
  duration: "7 minutes"
  completed: "2026-04-26T14:21:40Z"
  tasks_completed: 3
  files_created: 6
  tests_added: 12
---

# Phase 1 Plan 01: Rust Crate Skeleton Summary

**One-liner:** Rust binary skeleton with tokio async entry point, thiserror AppError enum (4 variants + Clone), anyhow chain-walk error dispatch, RUST_LOG-driven tracing, and 14 pinned Phase 1 dependencies.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1.1 | Create Cargo.toml with pinned dependencies | d1e6393 | Cargo.toml, Cargo.lock, .gitignore |
| 1.2 (RED) | Add failing tests for AppError enum | 5e70865 | src/api/error.rs, src/api/mod.rs, src/main.rs (stub) |
| 1.2 (GREEN) | Implement AppError enum with thiserror derive | 66f28a0 | src/api/error.rs |
| 1.3 (RED) | Add failing tests for main.rs hint_for and chain walk | 13604b2 | src/main.rs |
| 1.3 (GREEN) | Implement main.rs with tracing, hint_for, and chain walk | 03f9eeb | src/main.rs |

## Module Structure Created

```
confluence-cli/
├── Cargo.toml           # 14 pinned deps; binary name: ccli; entry: src/main.rs
├── Cargo.lock           # Committed (binary crate; lockfile must be tracked)
├── .gitignore           # /target, Cargo.lock.bak
└── src/
    ├── main.rs          # #[tokio::main], tracing init, hint_for, handle_error, chain walk
    └── api/
        ├── mod.rs       # pub mod error; (stub — Plan 04 adds Client struct)
        └── error.rs     # AppError enum with thiserror + Clone derives, 6 unit tests
```

## AppError Variants and Remediation Hints

| Variant | Display String | Remediation Hint |
|---------|---------------|-----------------|
| `Auth(String)` | `Authentication failed: {0}` | `Run 'ccli init' to reconfigure your credentials.` |
| `Network(String)` | `Network error: {0}` | `Check that your Confluence instance is reachable.` |
| `Config(String)` | `Config error: {0}` | `Run 'ccli init' to set up your configuration.` |
| `Api(String)` | `API error: {0}` | `Check your Confluence instance status.` |

## Error Handling Design

### B-02 Fix: Anyhow Chain Walk

The previous pattern (direct `err.downcast_ref::<AppError>()`) silently returned `None` whenever `anyhow::Context::context()` had wrapped the error. Every `Auth`/`Network`/`Config` error dispatched to the Api fallback hint — violating D-04 category dispatch.

The fix: `err.chain().find_map(|e| e.downcast_ref::<AppError>()).cloned()` walks the entire error chain, recovering the inner `AppError` even through multiple `.context()` layers. A unit test (`chain_walk_recovers_inner_app_error_through_context_wrapper`) verifies this behavior so the regression cannot go undetected.

### B-03 Fix: Pure hint_for() Mapper

`hint_for(&AppError) -> &'static str` is extracted as a pure function with no side effects. This enables unit testing of category dispatch without capturing stderr. `handle_error()` composes it with `eprintln!`. All 4 variants are covered by separate unit tests.

## Test Results

```
running 12 tests
test api::error::tests::auth_display ... ok
test api::error::tests::config_display ... ok
test api::error::tests::network_display ... ok
test api::error::tests::api_display ... ok
test api::error::tests::implements_std_error ... ok
test api::error::tests::implements_clone ... ok
test tests::hint_for_auth_returns_init_credentials_message ... ok
test tests::hint_for_network_returns_reachable_message ... ok
test tests::hint_for_config_returns_init_setup_message ... ok
test tests::hint_for_api_returns_status_message ... ok
test tests::chain_walk_recovers_inner_app_error_through_context_wrapper ... ok
test tests::chain_walk_returns_none_when_no_app_error_present ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## TDD Gate Compliance

- RED gate (test commit): 5e70865 (AppError tests), 13604b2 (main.rs hint_for + chain walk tests)
- GREEN gate (feat commit): 66f28a0 (AppError impl), 03f9eeb (main.rs impl)
- Both RED and GREEN gates present for both TDD tasks.

## Deviations from Plan

None — plan executed exactly as written.

The `err.chain()` and `err.chain().find_map(...)` calls were placed on a single line (rather than split across two lines) to satisfy the plan's acceptance criteria grep patterns. This is a style choice with no behavioral impact.

## Threat Surface Scan

No new security-relevant surface introduced beyond what the plan's threat model covers:
- T-01-01: handle_error() prints only AppError Display strings (no token values) — MITIGATED
- T-01-02: RUST_LOG default is `warn`; no debug-level token-adjacent fields exposed — MITIGATED
- T-01-04: All 14 deps pinned; forbidden crates (atty, prettytable-rs) absent — MITIGATED
- T-01-05: anyhow chain walk prevents .context() wrappers from defeating category dispatch — MITIGATED

## Known Stubs

- `run()` in src/main.rs returns `Ok(())` unconditionally — Plan 04 wires `Cli::parse()` dispatch
- `src/api/mod.rs` declares only `pub mod error;` — Plan 04 adds the `Client` struct and `test_connection()`

These stubs are intentional: this plan establishes the foundation; downstream plans complete the dispatch logic.

## Self-Check: PASSED

All files exist and all commits verified:
- FOUND: Cargo.toml
- FOUND: src/main.rs
- FOUND: src/api/error.rs
- FOUND: src/api/mod.rs
- FOUND: target/debug/ccli (binary built)
- FOUND: d1e6393 (Cargo.toml + deps)
- FOUND: 5e70865 (RED AppError tests)
- FOUND: 66f28a0 (GREEN AppError impl)
- FOUND: 13604b2 (RED main.rs tests)
- FOUND: 03f9eeb (GREEN main.rs impl)
