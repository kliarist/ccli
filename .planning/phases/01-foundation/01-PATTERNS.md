# Phase 1: Foundation - Pattern Map

**Mapped:** 2026-04-24
**Files analyzed:** 11 (new files; greenfield project)
**Analogs found:** 0 / 11 — no existing source files; all patterns sourced from RESEARCH.md verified references

## Greenfield Note

The working directory has no `src/` directory and no `Cargo.toml`. There are zero existing source files to draw analogs from. This phase **establishes the canonical patterns** that all subsequent phases will inherit and extend.

All pattern excerpts below are drawn from:
- RESEARCH.md verified examples (sourced from official crates.io / docs.rs references)
- Locked decisions from CONTEXT.md (D-01 through D-09)

The planner should treat every excerpt in this document as the ground-truth pattern for the project.

---

## File Classification

| New File | Role | Data Flow | Closest Analog | Match Quality |
|----------|------|-----------|----------------|---------------|
| `Cargo.toml` | config | — | none | no analog |
| `src/main.rs` | entry-point | request-response | none | no analog |
| `src/cli/mod.rs` | cli-parser | request-response | none | no analog |
| `src/cli/init.rs` | command-handler | request-response | none | no analog |
| `src/api/mod.rs` | service | request-response | none | no analog |
| `src/api/error.rs` | utility | — | none | no analog |
| `src/config/mod.rs` | service | file-I/O | none | no analog |
| `src/config/path.rs` | utility | — | none | no analog |
| `src/output/mod.rs` | service | transform | none | no analog |
| `src/output/table.rs` | utility | transform | none | no analog |
| `src/output/tsv.rs` | utility | transform | none | no analog |

---

## Pattern Assignments

### `Cargo.toml` (config)

**Source:** RESEARCH.md Standard Stack, verified against crates.io 2026-04-24

**Dependency block:**
```toml
[package]
name = "ccli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "ccli"
path = "src/main.rs"

[dependencies]
clap = { version = "4.6", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.13", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "1"
anyhow = "1"
thiserror = "2"
dialoguer = { version = "0.12", features = ["password"] }
comfy-table = "7"
is-terminal = "0.4"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
home = "0.5"
```

**Critical notes:**
- Do NOT add `dirs` as a dependency — `dirs::config_dir()` returns `~/Library/Application Support` on macOS (dirs 6.0+), which contradicts the locked config path `~/.config/ccli/config.toml`. Use `home` instead.
- Do NOT add `atty` — has RUSTSEC-2021-0145; use `is-terminal`.
- Do NOT add `prettytable-rs` — has RUSTSEC-2022-0074; abandoned 2022.
- `reqwest` 0.13 defaults to `rustls` (not `native-tls`). Self-signed cert support requires a flag; document in README.

---

### `src/main.rs` (entry-point, request-response)

**Role:** `#[tokio::main]` async entry point. Initializes tracing, parses CLI args, dispatches to command handlers, and catches all errors for two-line stderr formatting.

**Imports pattern:**
```rust
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod api;
mod cli;
mod config;
mod output;

use cli::{Cli, Commands};
```

**Async main + tracing init pattern (RESEARCH.md Pattern 7, Code Examples):**
```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("warn"))
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            if let Err(e) = cli::init::run(&cli).await {
                handle_error(&e.downcast::<api::error::AppError>().unwrap_or_else(|e| {
                    api::error::AppError::Api(e.to_string()).into()
                }));
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
```

**Two-line error handler pattern (D-04, RESEARCH.md Pattern 6):**
```rust
fn handle_error(err: &api::error::AppError) {
    eprintln!("Error: {}", err);
    let hint = match err {
        api::error::AppError::Auth(_) => "Run 'ccli init' to reconfigure your credentials.",
        api::error::AppError::Network(_) => "Check that your Confluence instance is reachable.",
        api::error::AppError::Config(_) => "Run 'ccli init' to set up your configuration.",
        api::error::AppError::Api(_) => "Check your Confluence instance status.",
    };
    eprintln!("{}", hint);
}
```

---

### `src/cli/mod.rs` (cli-parser, request-response)

**Role:** Defines the top-level `Cli` struct with global output flags and the `Commands` enum. All output flags use `global = true` so they propagate to every subcommand.

**Source:** RESEARCH.md Pattern 1 (clap v4 Derive with Global Output Flags)

**Full module pattern:**
```rust
use clap::{Parser, Subcommand};

pub mod init;

#[derive(Parser)]
#[command(name = "ccli", about = "Confluence CLI", version)]
pub struct Cli {
    /// Output tab-separated values (no box-drawing characters)
    #[arg(long, global = true)]
    pub plain: bool,

    /// Suppress column headers
    #[arg(long, global = true)]
    pub no_headers: bool,

    /// Comma-separated columns to include (e.g. key,name,status)
    #[arg(long, global = true)]
    pub columns: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Configure Confluence instance URL and Personal Access Token
    Init,
    // Space(SpaceArgs),  -- Phase 2
    // Page(PageArgs),    -- Phase 3
}
```

**Key detail:** `global = true` on every output flag is mandatory — it allows `ccli page list --plain` to work even though `--plain` is defined on the root `Cli` struct, not on `PageArgs`. Access via `cli.plain` in the top-level dispatch.

---

### `src/cli/init.rs` (command-handler, request-response)

**Role:** Handles `ccli init`. Loads existing config, runs dialoguer prompts with pre-fill, calls `api::test_connection`, saves config on success, errors on failure. Does NOT save config if connection test fails (D-02).

**Source:** RESEARCH.md Pattern 2 (dialoguer ccli init Flow)

**Imports pattern:**
```rust
use anyhow::Context;
use dialoguer::{theme::ColorfulTheme, Input, Password};

use crate::api;
use crate::config::{self, Config};
use crate::cli::Cli;
```

**Core handler pattern:**
```rust
pub async fn run(_cli: &Cli) -> anyhow::Result<()> {
    let existing = config::load()?;
    let theme = ColorfulTheme::default();

    // URL prompt — pre-fill with existing value on re-run (D-03)
    let url: String = Input::with_theme(&theme)
        .with_prompt("Confluence URL")
        .with_initial_text(
            existing.as_ref().map(|c| c.url.as_str()).unwrap_or("")
        )
        .validate_with(|s: &String| {
            if s.starts_with("http://") || s.starts_with("https://") {
                Ok(())
            } else {
                Err("URL must start with http:// or https://")
            }
        })
        .interact_text()?;

    // PAT prompt — show masked hint on re-run; empty = keep existing (D-03, Pitfall 4)
    let pat_prompt = match existing.as_ref() {
        Some(c) => format!("Personal Access Token [current: {}]", mask_pat_hint(&c.token)),
        None => "Personal Access Token".to_string(),
    };

    let raw_pat: String = Password::with_theme(&theme)
        .with_prompt(&pat_prompt)
        .allow_empty_password(true)
        .interact()?;

    // If empty, keep existing token; error on first run with empty (Pitfall 4)
    let token = if raw_pat.is_empty() {
        existing.as_ref()
            .map(|c| c.token.clone())
            .ok_or_else(|| anyhow::anyhow!("Token is required on first run"))?
    } else {
        raw_pat
    };

    // Test connection before saving (D-02)
    let display_name = api::test_connection(&url, &token).await
        .context("Connection test failed")?;

    // Save only on success (D-02)
    let cfg = Config { url: url.clone(), token };
    config::save(&cfg)?;

    println!("Connected as {} — configuration saved.", display_name);
    Ok(())
}

fn mask_pat_hint(token: &str) -> String {
    if token.len() <= 5 {
        return "*".repeat(token.len());
    }
    let prefix = &token[..2];
    let suffix = &token[token.len() - 3..];
    format!("{}*****{}", prefix, suffix)
}
```

---

### `src/api/mod.rs` (service, request-response)

**Role:** Owns the `reqwest::Client` (connection pool — create once, reuse). Exposes `test_connection()` for init validation and `Client` struct for all Phase 2+ commands.

**Source:** RESEARCH.md Pattern 4 (Confluence DC PAT Authentication), Code Examples (reqwest Client setup)

**Imports pattern:**
```rust
pub mod error;

use reqwest::{Client as ReqwestClient, header};
use std::time::Duration;
use tracing::{debug, instrument};

use crate::config::Config;
use error::AppError;
```

**Client struct pattern (anti-pattern prevention: create once, not per-request):**
```rust
pub struct Client {
    inner: ReqwestClient,
    base_url: String,
}

impl Client {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let mut headers = header::HeaderMap::new();
        let auth_value = header::HeaderValue::from_str(
            &format!("Bearer {}", config.token)
        )?;
        headers.insert(header::AUTHORIZATION, auth_value);

        let inner = ReqwestClient::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            inner,
            base_url: config.url.trim_end_matches('/').to_string(),
        })
    }
}
```

**Connection test pattern (use `/rest/api/user/current` — Pitfall 2):**
```rust
#[instrument(skip(token))]
pub async fn test_connection(base_url: &str, token: &str) -> Result<String, AppError> {
    let url = format!("{}/rest/api/user/current", base_url.trim_end_matches('/'));
    debug!("Testing connection: GET {}", url);  // Never log token value (Security)

    let resp = ReqwestClient::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                AppError::Network(format!("Cannot reach {}: {}", base_url, e))
            } else {
                AppError::Network(e.to_string())
            }
        })?;

    match resp.status().as_u16() {
        200 => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let name = body["displayName"].as_str().unwrap_or("unknown").to_string();
            Ok(name)
        }
        401 => Err(AppError::Auth("Invalid or expired token. Check your PAT.".to_string())),
        403 => Err(AppError::Auth(
            "Authenticated but access denied. Check your PAT permissions.".to_string()
        )),
        status => Err(AppError::Api(format!("Unexpected HTTP {}", status))),
    }
}
```

**Critical notes:**
- Do NOT use `/rest/api/content?limit=1` or similar as the validation probe — known Atlassian bug CONFSERVER-87540 returns 200 with empty body for invalid PAT on some endpoints.
- Never log the `Authorization` header value or the token string. Log only method + URL.
- `reqwest::Client` is a connection pool — the `Client` struct stores it and reuses across calls.

---

### `src/api/error.rs` (utility, —)

**Role:** Defines `AppError` enum with `thiserror` derive. Each variant maps to a user-facing error category. Main dispatch matches on these variants to produce remediation hints.

**Source:** RESEARCH.md Pattern 6 (AppError Two-Line Stderr)

**Full module:**
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("API error: {0}")]
    Api(String),
}
```

---

### `src/config/mod.rs` (service, file-I/O)

**Role:** Defines `Config` struct (serde Serialize/Deserialize). Implements `load()` (read TOML + env var overlay), `save()` (atomic write), and `load_or_error()` (error if no config). All file operations go through this module.

**Source:** RESEARCH.md Pattern 3 (Config Load with Env Var Override)

**Imports pattern:**
```rust
pub mod path;

use serde::{Deserialize, Serialize};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::api::error::AppError;
use path::config_path;
```

**Config struct:**
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub url: String,
    pub token: String,
}
```

**load() with env var override (INIT-03):**
```rust
pub fn load() -> anyhow::Result<Option<Config>> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)?;
    let mut config: Config = toml::from_str(&content)?;

    // Env var overrides (INIT-03)
    if let Ok(url) = std::env::var("CCLI_URL") {
        config.url = url;
    }
    if let Ok(token) = std::env::var("CCLI_TOKEN") {
        config.token = token;
    }
    Ok(Some(config))
}
```

**save() with atomic write + permissions (Pitfall 6 + Security):**
```rust
pub fn save(config: &Config) -> anyhow::Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;

    // Atomic write: temp file + rename (Pitfall 6)
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, &content)?;
    fs::rename(&tmp, &path)?;

    // Restrict permissions to owner only (Security: ASVS V2)
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;

    Ok(())
}
```

**load_or_error() for commands requiring credentials:**
```rust
pub fn load_or_error() -> Result<Config, AppError> {
    load()
        .map_err(|e| AppError::Config(e.to_string()))?
        .ok_or_else(|| AppError::Config(
            "No configuration found. Run 'ccli init' to configure.".to_string()
        ))
}
```

---

### `src/config/path.rs` (utility, —)

**Role:** Single function `config_path()` that returns the canonical `~/.config/ccli/config.toml` path. Isolated so other modules import from here — no inline path construction elsewhere.

**Source:** RESEARCH.md Pattern 3 (config_path using home::home_dir)

**Full module:**
```rust
use std::path::PathBuf;

/// Returns the canonical config file path: ~/.config/ccli/config.toml
///
/// IMPORTANT: Uses `home::home_dir()`, NOT `dirs::config_dir()`.
/// dirs 6.0+ returns ~/Library/Application Support on macOS, which contradicts
/// the locked path requirement (CONTEXT.md INIT-02).
pub fn config_path() -> anyhow::Result<PathBuf> {
    let home = home::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".config").join("ccli").join("config.toml"))
}
```

---

### `src/output/mod.rs` (service, transform)

**Role:** Owns `OutputConfig` (parsed from CLI flags) and `OutputFormatter`. The formatter dispatches to `table.rs` or `tsv.rs` based on TTY state and `--plain` flag. All command handlers call `OutputFormatter::print()` — never format output directly.

**Source:** RESEARCH.md Pattern 5 (Output Formatter with TTY Detection)

**Imports pattern:**
```rust
pub mod table;
pub mod tsv;

use is_terminal::IsTerminal;
```

**OutputConfig and OutputFormatter:**
```rust
pub struct OutputConfig {
    pub plain: bool,
    pub no_headers: bool,
    pub columns: Option<Vec<String>>,  // parsed from comma-separated string at call site
}

pub struct OutputFormatter {
    config: OutputConfig,
    is_tty: bool,
}

impl OutputFormatter {
    pub fn new(config: OutputConfig) -> Self {
        let is_tty = std::io::stdout().is_terminal();
        Self { config, is_tty }
    }

    /// Print headers + rows, applying column filter, TTY detection, and --plain.
    pub fn print(&self, headers: &[&str], rows: &[Vec<String>]) {
        let (active_headers, col_indices) = self.resolve_columns(headers);

        if self.is_tty && !self.config.plain {
            // Aligned table (OUT-01)
            table::render(&active_headers, &col_indices, rows, self.config.no_headers);
        } else {
            // TSV output (OUT-02) — plain flag or piped stdout
            tsv::render(&active_headers, &col_indices, rows, self.config.no_headers);
        }
    }

    /// Build (active_header_strings, column_index_list) from optional --columns filter (D-07).
    fn resolve_columns(&self, headers: &[&str]) -> (Vec<String>, Vec<usize>) {
        match &self.config.columns {
            None => {
                let indices: Vec<usize> = (0..headers.len()).collect();
                let names: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
                (names, indices)
            }
            Some(selected) => {
                let lower_headers: Vec<String> = headers.iter()
                    .map(|s| s.to_lowercase())
                    .collect();
                let mut names = Vec::new();
                let mut indices = Vec::new();
                for col in selected {
                    let col_lower = col.to_lowercase();
                    if let Some(i) = lower_headers.iter().position(|h| h == &col_lower) {
                        names.push(headers[i].to_string());
                        indices.push(i);
                    }
                }
                (names, indices)
            }
        }
    }
}
```

**Columns parsing helper (D-07 — comma-separated, call at CLI dispatch time):**
```rust
/// Parse --columns "key,name,status" into Vec<String>.
/// Called in main.rs dispatch when building OutputConfig.
pub fn parse_columns(s: &str) -> Vec<String> {
    s.split(',')
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect()
}
```

---

### `src/output/table.rs` (utility, transform)

**Role:** Renders an aligned terminal table using comfy-table. Called only when stdout is a TTY and `--plain` is not set. Always use `UTF8_FULL_CONDENSED` preset with `ContentArrangement::Dynamic`.

**Source:** RESEARCH.md Code Examples (comfy-table aligned table)

**Full module:**
```rust
use comfy_table::{ContentArrangement, Table};
use comfy_table::presets::UTF8_FULL_CONDENSED;

pub fn render(
    headers: &[String],
    col_indices: &[usize],
    rows: &[Vec<String>],
    no_headers: bool,
) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);

    if !no_headers {
        table.set_header(headers.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    }
    for row in rows {
        let filtered: Vec<&str> = col_indices.iter()
            .map(|&i| row[i].as_str())
            .collect();
        table.add_row(filtered);
    }
    println!("{table}");
}
```

---

### `src/output/tsv.rs` (utility, transform)

**Role:** Renders tab-separated values. Used when stdout is piped (D-06 TTY auto-detect) or `--plain` is set. Must NOT use comfy-table — even the `NOTHING` preset pads cells with spaces (Pitfall 5).

**Source:** RESEARCH.md Pattern 5 (TSV rendering), Pitfall 5

**Full module:**
```rust
/// Render rows as tab-separated values.
///
/// IMPORTANT: Do NOT use comfy-table's NOTHING preset here.
/// It pads cells with spaces to align columns — output is not true TSV
/// and breaks `awk -F'\t'`, `cut -f2`, and `sort -t$'\t'` pipelines.
/// Simple `fields.join("\t")` is the correct approach (Pitfall 5).
pub fn render(
    headers: &[String],
    col_indices: &[usize],
    rows: &[Vec<String>],
    no_headers: bool,
) {
    if !no_headers {
        println!("{}", headers.join("\t"));
    }
    for row in rows {
        let filtered: Vec<&str> = col_indices.iter()
            .map(|&i| row[i].as_str())
            .collect();
        println!("{}", filtered.join("\t"));
    }
}
```

---

## Shared Patterns

### Tracing / Debug Logging (D-05)

**Apply to:** `src/main.rs` (init), `src/api/mod.rs` (per-request tracing)

**Pattern:**
```rust
// In main.rs — initialize once before any async work
tracing_subscriber::fmt()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"))
    )
    .init();

// In api/mod.rs — instrument async functions; never log token value
#[tracing::instrument(skip(token))]
pub async fn test_connection(base_url: &str, token: &str) -> Result<String, AppError> {
    tracing::debug!("GET {}/rest/api/user/current", base_url);
    // ...
}
```

**Usage:** `RUST_LOG=debug ccli init` surfaces HTTP details. `skip(token)` ensures the PAT never appears in logs.

---

### Two-Line Error Format (D-04)

**Apply to:** `src/main.rs` error handler only (single error exit point)

**Pattern:**
```rust
fn handle_error(err: &AppError) {
    eprintln!("Error: {}", err);   // line 1: message
    eprintln!("{}", hint_for(err)); // line 2: remediation
    // Exit code 1 set by caller: std::process::exit(1)
}
```

All errors propagate upward to `main()` via `anyhow::Result`. Command handlers return `anyhow::Result<()>`. The single error handler in `main()` formats them. No `eprintln!` in command handlers or service modules.

---

### OutputConfig Construction (D-06, D-07)

**Apply to:** Every command handler dispatch in `src/main.rs` (Phase 2+)

**Pattern (established now, used from Phase 2 onward):**
```rust
use crate::output::{OutputConfig, OutputFormatter, parse_columns};

let output_config = OutputConfig {
    plain: cli.plain,
    no_headers: cli.no_headers,
    columns: cli.columns.as_deref().map(parse_columns),
};
let formatter = OutputFormatter::new(output_config);
// then: formatter.print(&headers, &rows);
```

---

### Config Load at Command Entry (INIT-03)

**Apply to:** Every command handler that requires Confluence credentials (Phase 2+)

**Pattern (established now, used from Phase 2 onward):**
```rust
use crate::config;

let config = config::load_or_error()
    .map_err(|e| anyhow::anyhow!(e))?;
// CCLI_URL / CCLI_TOKEN env vars are applied inside load_or_error()
let client = api::Client::new(&config)?;
```

---

### Atomic Config Write + File Permissions

**Apply to:** `src/config/mod.rs` `save()` — already embedded in the pattern above

**Summary of rules:**
1. Write to `config.toml.tmp` first, then `std::fs::rename` to `config.toml` (Pitfall 6 — atomic on POSIX).
2. After rename, call `fs::set_permissions(path, Permissions::from_mode(0o600))` on Unix (Security: ASVS V2).
3. Never write config directly with `std::fs::write(path, ...)`.

---

## No Analog Found

All 11 files have no existing codebase analog — this is a greenfield project.

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `Cargo.toml` | config | — | Greenfield project |
| `src/main.rs` | entry-point | request-response | Greenfield project |
| `src/cli/mod.rs` | cli-parser | request-response | Greenfield project |
| `src/cli/init.rs` | command-handler | request-response | Greenfield project |
| `src/api/mod.rs` | service | request-response | Greenfield project |
| `src/api/error.rs` | utility | — | Greenfield project |
| `src/config/mod.rs` | service | file-I/O | Greenfield project |
| `src/config/path.rs` | utility | — | Greenfield project |
| `src/output/mod.rs` | service | transform | Greenfield project |
| `src/output/table.rs` | utility | transform | Greenfield project |
| `src/output/tsv.rs` | utility | transform | Greenfield project |

The planner must use the patterns documented in this file (sourced from RESEARCH.md verified references) as the canonical implementation guide for all files.

---

## Critical Implementation Constraints

These are hard constraints derived from locked decisions and research pitfalls. Planner must include them in plan actions.

| Constraint | File(s) Affected | Source |
|------------|-----------------|--------|
| Use `home::home_dir()`, NOT `dirs::config_dir()` | `src/config/path.rs` | RESEARCH.md Pitfall 1, D-09 |
| Use `is-terminal`, NOT `atty` | `src/output/mod.rs` | RESEARCH.md State of the Art |
| Use `/rest/api/user/current` as PAT validation endpoint | `src/api/mod.rs` | RESEARCH.md Pitfall 2 |
| Do NOT use comfy-table NOTHING preset for TSV | `src/output/tsv.rs` | RESEARCH.md Pitfall 5 |
| Atomic config write (tmp + rename) | `src/config/mod.rs` | RESEARCH.md Pitfall 6 |
| Set file permissions 0o600 after config write (Unix) | `src/config/mod.rs` | RESEARCH.md Security |
| Never log Authorization header or token value | `src/api/mod.rs` | RESEARCH.md Security |
| `reqwest::Client` created once, reused across calls | `src/api/mod.rs` | RESEARCH.md Anti-Patterns |
| Do NOT save config if connection test fails | `src/cli/init.rs` | CONTEXT.md D-02 |
| Empty PAT on re-run = keep existing token | `src/cli/init.rs` | RESEARCH.md Pitfall 4, D-03 |
| `--columns` parses comma-separated at call site | `src/output/mod.rs` | CONTEXT.md D-07 |
| TTY auto-detect: piped = TSV without --plain flag | `src/output/mod.rs` | CONTEXT.md D-06 |

---

## Metadata

**Analog search scope:** No source files exist; greenfield project confirmed (no `src/`, no `Cargo.toml`)
**Files scanned:** 0 source files (only CONTEXT.md, RESEARCH.md, REQUIREMENTS.md read)
**Pattern sources:** RESEARCH.md verified references (crates.io / docs.rs, 2026-04-24)
**Pattern extraction date:** 2026-04-24
