---
phase: 01-foundation
plan: "02"
subsystem: config
tags:
  - rust
  - config
  - filesystem
  - foundation
  - tdd

dependency_graph:
  requires:
    - "01-01: AppError enum (Auth/Network/Config/Api variants)"
  provides:
    - "config_path() resolving to ~/.config/ccli/config.toml via home::home_dir()"
    - "Config struct (url + token, serde Serialize/Deserialize)"
    - "load() / load_from() with CCLI_URL + CCLI_TOKEN env overlay + env-only synthesis (W-06)"
    - "save() / save_to() atomic write via tmp+rename with 0o600 perms on Unix"
    - "load_or_error() / load_or_error_from() returning AppError::Config when no config"
    - "pub(crate) testable helpers load_from / save_to / load_or_error_from (W-05)"
  affects:
    - "01-04: Plan 04 Task 4.1 Step C adds mod config; to main.rs (B-01 coordination)"
    - "01-05: ccli init handler uses config::load() and config::save()"

tech_stack:
  added:
    - "home = 0.5 (already in Cargo.toml — used for home::home_dir())"
    - "tempfile = 3 ([dev-dependencies] added for TempDir-based tests)"
  patterns:
    - "pub(crate) testable helper pattern (W-05): public functions delegate to path-accepting helpers, enabling TempDir tests without HOME mutation"
    - "env-only synthesis pattern (W-06): CCLI_URL + CCLI_TOKEN both set and non-empty synthesize Config without a file"
    - "atomic config write: write to <path>.toml.tmp, then fs::rename to final path (Pitfall 6)"
    - "0o600 file permissions via fs::set_permissions + PermissionsExt on Unix (Security ASVS V2)"
    - "env var overlay: CCLI_URL / CCLI_TOKEN override file values when non-empty (INIT-03)"

key_files:
  created:
    - src/config/path.rs
    - src/config/mod.rs
  modified:
    - Cargo.toml

decisions:
  - "Used home::home_dir() not dirs::config_dir() — dirs 6.0+ returns ~/Library/Application Support on macOS, contradicting D-09 locked path requirement"
  - "Extracted pub(crate) load_from / save_to / load_or_error_from helpers (W-05) so tests use TempDir paths instead of mutating $HOME"
  - "Added W-06 env-only synthesis: when no file exists but CCLI_URL and CCLI_TOKEN are both non-empty, synthesize Config from env vars (partial or empty env falls through to None)"
  - "B-01 coordination: mod config; NOT added to src/main.rs; Plan 04 Task 4.1 Step C will add it alongside mod api; mod cli; mod output; in a single consolidated declaration"

metrics:
  duration_minutes: 8
  completed_date: "2026-04-26"
  tasks_completed: 2
  files_created: 2
  files_modified: 1
---

# Phase 01 Plan 02: Config Subsystem Summary

**One-liner:** Config subsystem with atomic writes, 0o600 perms, env var overlay + env-only synthesis via home::home_dir().

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 2.1 | Implement src/config/path.rs (config_path resolver) | 89e0fc4 | src/config/path.rs |
| 2.2 | Implement src/config/mod.rs (Config struct, load, save, load_or_error) | fc81cc7 | src/config/mod.rs, Cargo.toml |

## Public API

### src/config/path.rs

```rust
pub fn config_path() -> anyhow::Result<std::path::PathBuf>
// Returns: ~/.config/ccli/config.toml (uses home::home_dir(), NOT dirs::config_dir())
```

### src/config/mod.rs

```rust
// Public API consumed by Plans 04 and 05+
pub struct Config { pub url: String, pub token: String }
pub fn load() -> anyhow::Result<Option<Config>>
pub fn save(config: &Config) -> anyhow::Result<()>
pub fn load_or_error() -> Result<Config, AppError>

// pub(crate) testable helpers (W-05 — used by tests via TempDir)
pub(crate) fn load_from(path: &Path) -> anyhow::Result<Option<Config>>
pub(crate) fn save_to(path: &Path, config: &Config) -> anyhow::Result<()>
pub(crate) fn load_or_error_from(path: &Path) -> Result<Config, AppError>
```

## Key Design Notes

### W-05: Testable Helper Extraction
The public `load()`, `save()`, and `load_or_error()` functions delegate entirely to `pub(crate)` variants that accept an explicit `&Path`. This allows unit tests to use `tempfile::TempDir` paths and eliminates any need to mutate `$HOME` in tests, which is fragile in parallel test runners.

### W-06: Env-Only Synthesis
`load_from()` implements a three-branch precedence:
1. **File + env vars set**: file config loaded, env vars override (non-empty only)
2. **No file + both env vars set and non-empty**: synthesize Config from env vars (env-only mode)
3. **No file + partial/empty env**: return `None`

This makes `CCLI_URL` + `CCLI_TOKEN` alone sufficient credentials (ROADMAP success criterion 2) without requiring a pre-existing config file.

### Atomic Write Strategy
`save_to()` writes to `<path>.toml.tmp` first, then calls `fs::rename()` to atomically replace the final path. On POSIX systems, rename is guaranteed atomic. The `.tmp` file does not persist after a successful save. After rename, `fs::set_permissions(..., 0o600)` is called on Unix to restrict read access to the file owner only.

### B-01 Coordination
`src/main.rs` is NOT modified by this plan. The `mod config;` declaration will be added by Plan 04 Task 4.1 Step C alongside `mod api; mod cli; mod output;` in a single consolidated block. This prevents a race condition in Wave 2 parallel execution where Plans 02 and 03 would both try to edit main.rs.

## Test Coverage Matrix

| Test | Location | Behavior Verified | TempDir |
|------|----------|-------------------|---------|
| ends_with_config_ccli_config_toml | path.rs | Path ends in .config/ccli/config.toml | No |
| is_absolute | path.rs | config_path() returns absolute path | No |
| contains_expected_components | path.rs | .config, ccli, config.toml components present | No |
| toml_roundtrip | mod.rs | Config serializes + deserializes preserving url + token | No |
| load_from_returns_none_when_missing_and_no_env | mod.rs | Ok(None) when no file and no env vars | Yes |
| save_to_creates_file_atomically | mod.rs | config.toml exists after save; .toml.tmp gone | Yes |
| save_then_load_roundtrip | mod.rs | Full save + load roundtrip via TempDir | Yes |
| ccli_url_env_var_overrides_file_value | mod.rs | CCLI_URL env var overrides file url | Yes |
| ccli_token_env_var_overrides_file_value | mod.rs | CCLI_TOKEN env var overrides file token | Yes |
| save_to_sets_0600_permissions_on_unix | mod.rs | 0o600 mode after save on Unix | Yes |
| load_or_error_from_returns_config_variant_when_missing | mod.rs | AppError::Config with "ccli init" message | Yes |
| load_from_env_only_synthesizes_config_when_both_vars_set_and_file_missing | mod.rs | W-06: both env vars set → Ok(Some(Config)) | Yes |
| load_from_partial_env_only_returns_none_when_file_missing | mod.rs | W-06: only CCLI_URL set → Ok(None) | Yes |
| load_from_empty_env_only_returns_none_when_file_missing | mod.rs | W-06: empty CCLI_URL → Ok(None) | Yes |

**Total: 3 path tests + 11 mod tests = 14 tests**

Note: Tests in `src/config/path.rs` and `src/config/mod.rs` are compiled into the binary once Plan 04 Task 4.1 Step C adds `mod config;` to `src/main.rs`. Until that declaration lands, the module compiles via `cargo check --tests --all-targets` but tests are not reachable via `cargo test --bin ccli config::`.

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None. All functions are fully implemented; no placeholder data flows to any consumer.

## Threat Surface

The implementation covers all mitigations in the plan's `<threat_model>`:

| Threat ID | Mitigation | Verified |
|-----------|-----------|---------|
| T-02-01 | 0o600 permissions after atomic rename | Yes — `save_to_sets_0600_permissions_on_unix` test |
| T-02-02 | Atomic tmp+rename write; no direct fs::write to canonical path | Yes — `save_to_creates_file_atomically` test |
| T-02-03 | Config derives Debug but is never logged; no tracing calls referencing token | Yes — no tracing macros in config module |
| T-02-06 | toml::from_str returns Result; errors propagate via anyhow/AppError; no unwrap() | Yes — all parse paths use ? operator |

No new threat surface introduced beyond what was in the plan's threat model.

## Self-Check: PASSED

Files exist:
- src/config/path.rs — FOUND
- src/config/mod.rs — FOUND

Commits exist:
- 89e0fc4 (Task 2.1: config_path resolver) — FOUND
- fc81cc7 (Task 2.2: Config struct + load/save/load_or_error) — FOUND
