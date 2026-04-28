---
phase: "02"
plan: "02"
subsystem: cli
tags: [cli, dispatch, spaces, tui-stub, clap, optional-subcommand]
dependency_graph:
  requires: [src/api/space.rs, src/output/mod.rs, src/cli/mod.rs, src/main.rs]
  provides: [src/cli/space.rs, src/tui/mod.rs]
  affects: [src/cli/mod.rs, src/main.rs]
tech_stack:
  added: []
  patterns: [optional-subcommand-clap, anyhow-context-wrapping, output-formatter-usage]
key_files:
  created: [src/cli/space.rs, src/tui/mod.rs]
  modified: [src/cli/mod.rs, src/main.rs]
decisions:
  - "D-10: bare ccli (no subcommand) dispatches to tui::run() via None => tui::run().await"
  - "D-11: ccli space list always prints table — never opens TUI"
  - "D-17: list_all_spaces used by both TUI pre-fetch and ccli space list"
metrics:
  duration_minutes: 4
  completed_date: "2026-04-28"
  tasks_completed: 2
  tasks_total: 2
  files_created: 2
  files_modified: 2
  tests_added: 2
  tests_total: 91
---

# Phase 2 Plan 02: CLI Dispatch and Space List Handler Summary

**One-liner:** Wired `ccli space list` via SpaceArgs/SpaceCommands clap parser, OutputFormatter table output, and D-10 optional-subcommand dispatch making bare `ccli` launch the TUI stub.

## What Was Built

**Task 1: src/cli/mod.rs updated (D-10, SpaceArgs, SpaceCommands)**
- `pub command: Commands` changed to `pub command: Option<Commands>` — bare `ccli` now parses as `None` instead of erroring
- `Space(SpaceArgs)` variant added to `Commands` enum
- `SpaceArgs` struct and `SpaceCommands::List` subcommand declared
- `pub mod space;` declaration added
- `missing_subcommand_errors` test renamed to `no_subcommand_launches_tui` (D-10 flip: asserts `is_ok()` and `command.is_none()`)
- New `space_list_subcommand_parses` test added
- Existing `parses_init_subcommand` updated to match `Some(Commands::Init)`

**Task 2: src/cli/space.rs implemented + src/main.rs updated + src/tui/mod.rs stub created**
- `src/cli/space.rs`: `pub async fn run(cli: &Cli, _args: &SpaceArgs)` loads config via `config::load_or_error()`, builds `Client`, calls `list_all_spaces(&client)`, maps spaces to `Vec<Vec<String>>` rows, and calls `OutputFormatter::print()` with `["Key", "Name", "Type"]` headers (SPC-04, D-11, D-17)
- `src/tui/mod.rs`: minimal stub `pub async fn run()` that bails with a descriptive error message — placeholder for Plan 05
- `src/main.rs`: `mod tui;` declared; import changed to `use cli::{Cli, Commands, SpaceCommands};`; dispatch updated to `match cli.command` with `Some(Commands::Init)`, `Some(Commands::Space(ref args))` → `SpaceCommands::List`, and `None => tui::run().await` (D-10)

## Tests

91 total tests pass (was 90 after Plan 01, +1 net: renamed test + 1 new).

| Test | What it covers |
|------|---------------|
| `no_subcommand_launches_tui` (renamed) | D-10: bare ccli parses as Ok with command = None |
| `space_list_subcommand_parses` (new) | `ccli space list` parsed as Some(Commands::Space(_)) |

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 | ed20f1c | feat(02-02): add SpaceArgs/SpaceCommands to cli/mod.rs; make command optional (D-10) |
| Task 2 | bfcf4ad | feat(02-02): implement cli/space.rs handler; update main.rs dispatch; add tui stub |

## Deviations from Plan

None — plan executed exactly as written.

Note: `src/output/table.rs` and `src/output/tsv.rs` had pre-existing uncommitted modifications (bounds-safe row access via `.get(i)` and TSV cell escaping) visible in git status at plan start. These were not part of this plan and were not committed here — they remain as unstaged changes for a separate commit.

## Threat Model Coverage

| Threat | Mitigation | Status |
|--------|------------|--------|
| T-02-05: Token in error messages | `anyhow::Error::from` on AppError; main.rs chain-walk surfaces Auth/Network errors with hint_for(); token not included in error text | Implemented |
| T-02-06: Space list output sensitivity | Space Key, Name, Type are public metadata; not credential-sensitive | Accepted |
| T-02-07: ccli space list spoofing | Non-interactive table; no auth state in rows; API-sourced data only | Accepted |
| T-02-08: TUI stub tampering | Stub exits with error message; no credentials processed; replaced in Plan 05 | Accepted |

## Known Stubs

- `src/tui/mod.rs`: `tui::run()` is a stub that bails with an error message. This is intentional — Plan 05 (02-05-PLAN.md) will replace it with the full Ratatui TUI implementation. The stub exists solely so `mod tui;` compiles and `None => tui::run().await` in main.rs dispatches correctly.

## Self-Check: PASSED

- [x] `src/cli/space.rs` exists
- [x] `src/cli/space.rs` contains `pub async fn run(cli: &Cli`
- [x] `src/cli/space.rs` contains `list_all_spaces(&client)`
- [x] `src/cli/space.rs` contains `formatter.print(headers, &rows)`
- [x] `src/cli/mod.rs` contains `pub command: Option<Commands>`
- [x] `src/cli/mod.rs` contains `Space(SpaceArgs),` in Commands enum
- [x] `src/cli/mod.rs` contains `pub mod space;`
- [x] `src/main.rs` contains `mod tui;`
- [x] `src/main.rs` contains `None => tui::run().await`
- [x] `src/main.rs` contains `SpaceCommands::List => cli::space::run`
- [x] `src/tui/mod.rs` exists (stub)
- [x] Commit ed20f1c exists (Task 1 - cli/mod.rs)
- [x] Commit bfcf4ad exists (Task 2 - space.rs, tui stub, main.rs)
- [x] `cargo build --release` exits 0
- [x] 91 tests pass, 0 fail
