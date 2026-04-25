# Phase 1: Foundation - Research

**Researched:** 2026-04-24
**Domain:** Rust CLI — Confluence DC PAT auth, interactive init, config persistence, output formatting
**Confidence:** HIGH (core stack verified via crates.io + official docs; Confluence error behavior MEDIUM)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Use **dialoguer** for interactive prompts — colored labels, input validation inline, masked PAT field.
- **D-02:** On `ccli init`, **test the connection** before writing config. Show success/error. Do not save on failure.
- **D-03:** On re-run with existing config, **pre-fill prompts** — show existing URL, show masked PAT hint (e.g., `AT*****xyz`). Enter to keep.
- **D-04:** Error format is **two-line**: `Error: <message>` on stderr, then remediation hint line (e.g., `Run 'ccli init' to reconfigure.`).
- **D-05:** Debug verbosity via **RUST_LOG env var** (`tracing`/`env_logger` approach). No --debug flag in Phase 1.
- **D-06:** **TTY auto-detect**: TTY → aligned table; piped → tab-separated rows automatically. `--plain` still accepted explicitly.
- **D-07:** `--columns` takes **comma-separated column names** (e.g., `--columns key,name,status`).
- **D-08:** Use **tokio + reqwest (async)** from Phase 1.
- **D-09:** Config supports a **single profile** (one URL + one PAT). Flat schema.

### Claude's Discretion
- Table formatting library choice (comfy-table, tabled, prettytable-rs).
- Project structure (single crate vs. workspace, module layout for `api/`, `config/`, `output/`).
- Default columns per resource type.

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| INIT-01 | User can run `ccli init` to interactively configure URL and PAT on first run | dialoguer Input + Password patterns confirmed; wizard example matches flow |
| INIT-02 | Configuration persisted to `~/.config/ccli/config.toml` | toml crate + serde + home crate (avoid dirs::config_dir on macOS — see pitfalls) |
| INIT-03 | User can override config via `CCLI_URL` / `CCLI_TOKEN` env vars | std::env::var — no crate needed; handled at config-load time |
| INIT-04 | User sees clear error with remediation hint when credentials are missing/invalid | thiserror error types + two-line stderr format; reqwest status/connect error detection |
| OUT-01 | Non-interactive output formats as aligned table | comfy-table with TTY detection via is-terminal |
| OUT-02 | `--plain` outputs tab-separated values | Custom TSV writer (comfy-table doesn't support tab separator; see Don't Hand-Roll) |
| OUT-03 | `--no-headers` suppresses column header row | OutputConfig flag; conditional header emission in OutputFormatter |
| OUT-04 | `--columns` selects which columns to include | Comma-split at parse time; filter Row fields before rendering |
</phase_requirements>

---

## Summary

Phase 1 builds the Rust binary skeleton for `ccli`: a `#[tokio::main]` async CLI with clap v4 derive macros for subcommand dispatch, dialoguer for interactive `ccli init`, reqwest 0.13 for Confluence DC PAT authentication, toml 1.x + serde for config persistence, comfy-table for aligned table rendering, and `is-terminal` for TTY detection.

The crate ecosystem for all locked decisions is actively maintained, with one important API nuance: `dirs::config_dir()` changed behavior on macOS in version 6.0 (now returns `~/Library/Application Support` instead of `~/.config`). Since CONTEXT.md locks the config path to `~/.config/ccli/config.toml`, the implementation must use `home::home_dir()` directly and append `.config/ccli/config.toml` — not `dirs::config_dir()`.

For tab-separated output (OUT-02), comfy-table does not support tab-delimited output; the `NOTHING` preset removes all borders but still pads cells. The OutputFormatter must implement TSV rendering independently alongside the comfy-table path — this is trivially simple (join fields with `\t`, print rows) and is the correct pattern.

**Primary recommendation:** Single binary crate (no workspace yet), four modules (`cli/`, `api/`, `config/`, `output/`), with `AppError` via thiserror covering auth/network/config failure categories.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| CLI argument parsing and subcommand dispatch | CLI binary | — | clap derive macro lives in `main.rs` / `cli/` module |
| Interactive `ccli init` prompts | CLI binary | — | dialoguer is sync; wrapped in `tokio::task::spawn_blocking` if needed |
| Config read/write (`~/.config/ccli/config.toml`) | Config module | — | Platform-independent fs operations; no network |
| Confluence PAT validation (connection test) | API module | — | Async reqwest call; HTTP layer only |
| Output formatting (table / TSV) | Output module | CLI binary | OutputFormatter called by command handlers |
| TTY detection | Output module | — | `is-terminal` checked at render time |
| Error presentation | CLI binary | Output module | Two-line format printed to stderr by main error handler |

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.6.1 | CLI argument parsing, subcommands, global flags | Industry-standard Rust CLI library; derive macro eliminates boilerplate |
| tokio | 1.52.1 | Async runtime | Required for reqwest async; needed for Phase 2 TUI anyway (D-08) |
| reqwest | 0.13.2 | HTTP client for Confluence REST API | Most popular Rust async HTTP client; ships json + rustls by default in 0.13 |
| serde | 1.0.228 | Serialization framework | Required by toml + serde_json |
| serde_json | 1.0.149 | JSON deserialization for API responses | Standard companion to serde |
| toml | 1.1.2 | Config file read/write | Official toml-rs crate; serde-compatible |
| anyhow | 1.0.102 | Error propagation in binary code | Opaque errors with context for main flow |
| thiserror | 2.0.18 | Typed error definitions for AppError | Enables `match` on error variants for user-facing messages |
| dialoguer | 0.12.0 | Interactive prompts with masking | Chosen (D-01); ColorfulTheme, Input, Password |
| comfy-table | 7.2.2 | Aligned terminal table rendering | 14.9M recent downloads; actively maintained; built-in TTY width detection |
| is-terminal | 0.4.17 | TTY detection for stdout | Replacement for `atty` (RUSTSEC-2021-0145 advisory); maintained |
| tracing | 0.1.44 | Structured logging framework | Chosen (D-05); tokio-integrated |
| tracing-subscriber | 0.3.23 | RUST_LOG subscriber for tracing | `tracing_subscriber::fmt::init()` reads RUST_LOG automatically |
| home | 0.5.12 | Resolve `$HOME` consistently | Needed to construct `~/.config/ccli/config.toml` reliably across platforms |

### Supporting (optional / phase-later)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| dirs | 6.0.0 | Platform config/data dirs | Only if XDG compliance desired — **NOT** for `~/.config` on macOS (see pitfalls) |
| crossterm | 0.29.0 | Terminal manipulation | Phase 2 (Ratatui); not needed in Phase 1 |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| comfy-table | tabled 0.20.0 | tabled has more styling options and active development (April 2026 commits) but 2.3x fewer downloads; both are fine choices |
| comfy-table | prettytable-rs | **Rejected**: RUSTSEC-2022-0074 unsoundness advisory; abandoned since 2022 |
| is-terminal | atty 0.2.14 | **Rejected**: RUSTSEC-2021-0145 unaligned read on Windows; atty unmaintained since 2020 |
| reqwest default (rustls) | reqwest + native-tls | native-tls uses OS TLS; useful if DC instance has self-signed cert (use `danger_accept_invalid_certs` or `add_root_certificate`) |
| home | dirs::home_dir | dirs 6.0 home_dir is fine but adds a dep just for one function; `home` crate is standalone |

**Installation:**

```toml
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

**Version verification:** All versions confirmed against crates.io registry on 2026-04-24. [VERIFIED: crates.io API]

---

## Architecture Patterns

### System Architecture Diagram

```
 CLI Entry Point (main.rs)
        │
        ▼
 clap::Parser::parse()
        │
        ├─── Commands::Init ──────────────────────────────────────────────┐
        │                                                                   │
        │    config::load() ──► Option<Config>                             │
        │           │                                                       │
        │    dialoguer prompts                                              │
        │    (Input URL, Password PAT)                                     │
        │           │                                                       │
        │    api::test_connection(url, pat) ──► reqwest ──► Confluence DC │
        │           │                              │                        │
        │           │                    HTTP 200 OK ──► config::save()   │
        │           │                    HTTP 401/403 ──► AppError::Auth  │
        │           │                    connect err ──► AppError::Network│
        │           ▼                                                       │
        │    stderr: "Error: <msg>\nRun 'ccli init' to reconfigure."       │
        │                                                                   ◄─┘
        │
        ├─── Commands::Space(SpaceCmd) ── (Phase 2)
        │           │
        │    config::load_or_error()     ← CCLI_URL / CCLI_TOKEN override
        │           │
        │    api::Client::new(config)
        │           │
        │    output::OutputFormatter::new(output_flags)
        │           │
        │    ┌──────┴────────────────────┐
        │    │  is_terminal(stdout)?     │
        │    │  YES ──► comfy-table      │
        │    │  NO  ──► TSV writer       │
        │    │  --plain ──► TSV writer   │
        │    └───────────────────────────┘
        │           │
        │    stdout (table or TSV)
        │
        └─── (future commands...)
```

### Recommended Project Structure

```
confluence-cli/
├── Cargo.toml
├── Cargo.lock
└── src/
    ├── main.rs              # #[tokio::main], clap::Parser::parse(), error handler, stderr format
    ├── cli/
    │   ├── mod.rs           # Cli struct, Commands enum, OutputArgs struct (global flags)
    │   └── init.rs          # ccli init command handler
    ├── api/
    │   ├── mod.rs           # Client struct, test_connection()
    │   └── error.rs         # ApiError → AppError conversion
    ├── config/
    │   ├── mod.rs           # Config struct, load(), save(), load_or_error()
    │   └── path.rs          # config_path() using home::home_dir()
    └── output/
        ├── mod.rs           # OutputFormatter, OutputConfig (plain/no_headers/columns)
        ├── table.rs         # comfy-table rendering
        └── tsv.rs           # TSV rendering (trivial join with \t)
```

**Note:** Single crate is correct for Phase 1 (and likely all of v1). A workspace makes sense only if `ccli` gains a library crate that other tools consume. Do not over-engineer.

### Pattern 1: clap v4 Derive with Global Output Flags

**What:** Global flags defined on the top-level `Cli` struct propagate to all subcommands without redefinition.
**When to use:** All output flags (`--plain`, `--no-headers`, `--columns`) belong here.

```rust
// Source: https://docs.rs/clap/4.6.1/clap/_derive/index.html [VERIFIED: docs.rs]
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ccli", about = "Confluence CLI")]
struct Cli {
    /// Output tab-separated values (no box-drawing characters)
    #[arg(long, global = true)]
    plain: bool,

    /// Suppress column headers
    #[arg(long, global = true)]
    no_headers: bool,

    /// Comma-separated columns to include (e.g. key,name,status)
    #[arg(long, global = true)]
    columns: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure instance URL and PAT
    Init,
    // Space(SpaceArgs),  -- Phase 2
}
```

**Key detail:** `global = true` makes the flag available from any subcommand. Access via `cli.plain` in the top-level handler.

### Pattern 2: dialoguer ccli init Flow

**What:** Multi-step interactive prompts with pre-fill on re-run and masked PAT input.
**When to use:** `ccli init` command handler.

```rust
// Source: https://docs.rs/dialoguer/0.12.0/dialoguer/ [VERIFIED: docs.rs + examples]
use dialoguer::{theme::ColorfulTheme, Input, Password};

let theme = ColorfulTheme::default();

// URL: pre-fill with existing value if re-run
let url: String = Input::with_theme(&theme)
    .with_prompt("Confluence URL")
    .with_initial_text(existing_config.as_ref().map(|c| c.url.as_str()).unwrap_or(""))
    .validate_with(|s: &String| {
        if s.starts_with("http://") || s.starts_with("https://") {
            Ok(())
        } else {
            Err("URL must start with http:// or https://")
        }
    })
    .interact_text()?;

// PAT: masked hint on re-run — use prompt text to show hint, not pre-fill
// (Password struct has no with_initial_text; hint is baked into the prompt label)
let pat_prompt = match existing_config {
    Some(ref c) => {
        let hint = mask_pat_hint(&c.token); // e.g. "AT*****xyz"
        format!("Personal Access Token [current: {}]", hint)
    }
    None => "Personal Access Token".to_string(),
};

let raw_pat: String = Password::with_theme(&theme)
    .with_prompt(&pat_prompt)
    .allow_empty_password(true)   // allow Enter to keep existing
    .interact()?;

// If user pressed Enter (empty), keep existing token
let token = if raw_pat.is_empty() {
    existing_config.as_ref().map(|c| c.token.clone())
        .ok_or_else(|| anyhow::anyhow!("Token is required on first run"))?
} else {
    raw_pat
};
```

**PAT masking helper:**
```rust
fn mask_pat_hint(token: &str) -> String {
    // Show first 2 chars + ***** + last 3 chars
    if token.len() <= 5 {
        return "*".repeat(token.len());
    }
    let prefix = &token[..2];
    let suffix = &token[token.len() - 3..];
    format!("{}*****{}", prefix, suffix)
}
```

### Pattern 3: Config Load with Env Var Override

**What:** Load config from TOML file, then overlay `CCLI_URL` / `CCLI_TOKEN` env vars.
**When to use:** Every command that needs Confluence credentials.

```rust
// Source: toml 1.x serde pattern [VERIFIED: crates.io docs.rs]
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub url: String,
    pub token: String,
}

pub fn load() -> anyhow::Result<Option<Config>> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
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

pub fn save(config: &Config) -> anyhow::Result<()> {
    let path = config_path()?;
    // Create parent dirs if missing
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    // Atomic write: write to temp file, then rename
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

// CRITICAL: use home::home_dir(), NOT dirs::config_dir() (see pitfalls)
pub fn config_path() -> anyhow::Result<std::path::PathBuf> {
    let home = home::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".config").join("ccli").join("config.toml"))
}
```

### Pattern 4: Confluence DC PAT Authentication and Validation

**What:** HTTP Bearer token header; `GET /rest/api/user/current` as validation probe.
**When to use:** `ccli init` after prompts, before saving config.

```rust
// Source: Atlassian PAT documentation [CITED: https://confluence.atlassian.com/enterprise/using-personal-access-tokens-1026032365.html]
use reqwest::Client;

pub async fn test_connection(base_url: &str, token: &str) -> Result<String, AppError> {
    let url = format!("{}/rest/api/user/current", base_url.trim_end_matches('/'));
    let resp = Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
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
            // Parse display_name for success message
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let name = body["displayName"].as_str().unwrap_or("unknown").to_string();
            Ok(name)
        }
        401 => Err(AppError::Auth("Invalid or expired token. Check your PAT.".to_string())),
        403 => Err(AppError::Auth("Authenticated but access denied. Check your PAT permissions.".to_string())),
        status => Err(AppError::Api(format!("Unexpected HTTP {}", status))),
    }
}
```

**Note on Confluence DC error behavior:** There is a known Atlassian bug (CONFSERVER-87540) where using an invalid PAT against some endpoints returns HTTP 200 with empty results or HTTP 404 instead of 401. `GET /rest/api/user/current` is the most reliable validation endpoint because it is user-scoped and returns the authenticated user's details on success or 401/403 on auth failure. [CITED: https://jira.atlassian.com/browse/CONFSERVER-87540]

### Pattern 5: Output Formatter with TTY Detection

**What:** OutputFormatter dispatches to comfy-table or TSV based on TTY state and flags.
**When to use:** All command handlers that output tabular data.

```rust
// Source: is-terminal crate [VERIFIED: crates.io]; comfy-table presets [VERIFIED: GitHub source]
use comfy_table::{Table, ContentArrangement};
use comfy_table::presets::UTF8_FULL_CONDENSED;
use is_terminal::IsTerminal;

pub struct OutputConfig {
    pub plain: bool,
    pub no_headers: bool,
    pub columns: Option<Vec<String>>,
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

    pub fn print(&self, headers: &[&str], rows: &[Vec<String>]) {
        // Determine active columns
        let (active_headers, col_indices) = self.resolve_columns(headers);

        if self.is_tty && !self.config.plain {
            // Aligned table (OUT-01)
            let mut table = Table::new();
            table.load_preset(UTF8_FULL_CONDENSED)
                 .set_content_arrangement(ContentArrangement::Dynamic);
            if !self.config.no_headers {
                table.set_header(active_headers.iter().map(|s| s.as_str()).collect::<Vec<_>>());
            }
            for row in rows {
                let filtered: Vec<_> = col_indices.iter().map(|&i| &row[i]).collect();
                table.add_row(filtered);
            }
            println!("{table}");
        } else {
            // TSV output (OUT-02) — simple, no comfy-table dependency
            if !self.config.no_headers {
                println!("{}", active_headers.join("\t"));
            }
            for row in rows {
                let filtered: Vec<_> = col_indices.iter().map(|&i| row[i].as_str()).collect();
                println!("{}", filtered.join("\t"));
            }
        }
    }
}
```

### Pattern 6: AppError Two-Line Stderr (D-04)

```rust
// Source: thiserror 2.x derive pattern [VERIFIED: crates.io]
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

// In main.rs — print two-line error format (D-04)
fn handle_error(err: &AppError) {
    eprintln!("Error: {}", err);
    let hint = match err {
        AppError::Auth(_) => "Run 'ccli init' to reconfigure your credentials.",
        AppError::Network(_) => "Check that your Confluence instance is reachable.",
        AppError::Config(_) => "Run 'ccli init' to set up your configuration.",
        AppError::Api(_) => "Check your Confluence instance status.",
    };
    eprintln!("{}", hint);
}
```

### Pattern 7: tracing Setup for RUST_LOG

```rust
// Source: tracing-subscriber docs [VERIFIED: docs.rs]
// In main.rs, before any async work:
tracing_subscriber::fmt::init();
// Reads RUST_LOG automatically. Usage: RUST_LOG=debug ccli ...
// Or with default level: 
tracing_subscriber::fmt()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"))
    )
    .init();
```

### Anti-Patterns to Avoid

- **Using `dirs::config_dir()` for config path:** On macOS (dirs 6.0+), returns `~/Library/Application Support`, not `~/.config`. The CONTEXT locks the path to `~/.config/ccli/config.toml`. Use `home::home_dir()` instead.
- **Using `atty` crate:** Has RUSTSEC-2021-0145 (unaligned read on Windows); use `is-terminal` instead.
- **Using `prettytable-rs`:** Has RUSTSEC-2022-0074 unsoundness advisory; abandoned since 2022.
- **Writing config with `std::fs::write` non-atomically:** On crash mid-write, config file is corrupt. Use write-to-tmp + rename.
- **Using blocking file I/O inside async tokio tasks directly:** Fine for CLI where init is the only blocking call, but wrap in `tokio::task::spawn_blocking` if it becomes frequent.
- **Assuming 401 from Confluence DC for bad PAT:** Known bug (CONFSERVER-87540) — some endpoints return 200/404. Use `/rest/api/user/current` as validation endpoint for consistent 401/200 behavior.
- **Calling `reqwest::Client::new()` per request:** Client is a connection pool; create once and reuse (via `api::Client` struct wrapping `reqwest::Client`).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CLI arg parsing + help text | Custom argv parser | clap v4 derive | Auto-generates --help, --version, type checking, completion |
| Interactive prompts with masking | readline loop with echo off | dialoguer | Handles terminal state, masking, re-read, cross-platform |
| TOML serialization/deserialization | String manipulation | toml + serde | Handles quoting, escaping, multiline, type mapping |
| Aligned table rendering | Custom column-width calculation | comfy-table | Unicode width, wrapping, terminal-width detection |
| TTY detection | ioctl/syscall | is-terminal | RUSTSEC-safe; handles pipes, redirects, all platforms |
| Async runtime | Thread pool + channels | tokio | Event loop, task scheduling, timer — enormous surface area |
| HTTP connection pooling | Thread-per-request | reqwest::Client | TLS handshake reuse, keep-alive, redirect handling |

**Key insight:** Every item above has multi-platform edge cases (Windows, macOS, Linux) that require hundreds of test cases. The crates have accumulated this coverage; custom solutions in a v1 CLI will not.

---

## Common Pitfalls

### Pitfall 1: dirs::config_dir() Breaks Explicit Config Path on macOS

**What goes wrong:** `dirs::config_dir()` in dirs 6.0 returns `~/Library/Application Support` on macOS. Code using `dirs::config_dir().join("ccli/config.toml")` puts config at the wrong path — not `~/.config/ccli/config.toml`.

**Why it happens:** dirs 6.0 changed macOS behavior to follow Apple guidelines. Breaking change introduced 2025-01-12. [VERIFIED: crates.io dirs changelog]

**How to avoid:** Use `home::home_dir()` and append `.config/ccli/config.toml` explicitly. This matches the locked requirement in CONTEXT.md regardless of platform.

**Warning signs:** Config file not found on macOS after `ccli init` appears to succeed.

---

### Pitfall 2: Confluence DC Returns 200 (not 401) for Invalid PAT on Some Endpoints

**What goes wrong:** Using `/rest/api/content?limit=1` or similar as the validation probe — an invalid PAT may return HTTP 200 with empty results, causing `ccli init` to save a broken config.

**Why it happens:** Known Atlassian regression (CONFSERVER-87540) where PAT validation is inconsistently applied. [CITED: https://jira.atlassian.com/browse/CONFSERVER-87540]

**How to avoid:** Use `GET /rest/api/user/current` as the validation endpoint. It is user-identity-bound and returns 200+body for valid PAT, 401 for invalid/expired PAT.

**Warning signs:** `ccli init` reports success but subsequent commands fail with auth errors.

---

### Pitfall 3: reqwest 0.13 Uses rustls by Default (TLS for Self-Signed Certs)

**What goes wrong:** Many Confluence Data Center instances use self-signed or internal-CA certificates. reqwest 0.13 changed the TLS default from `native-tls` to `rustls`, which does not use the OS certificate store by default. Connections to such instances will fail with a TLS error.

**Why it happens:** reqwest 0.13 breaking change: rustls with `rustls-platform-verifier` is now the default. [VERIFIED: reqwest CHANGELOG on GitHub]

**How to avoid:** For Phase 1, document the issue and accept it (most test environments have valid certs). For production-ready support, consider adding `danger_accept_invalid_certs(true)` behind a `--insecure` flag, or use `native-tls` feature for OS trust store.

**Warning signs:** TLS handshake failures on HTTPS connections to internal Confluence instances.

---

### Pitfall 4: Password::interact() Returns Empty String on Enter-Without-Input

**What goes wrong:** On re-run (`ccli init` with existing config), the PAT prompt shows a hint like `[current: AT*****xyz]`. If the user presses Enter to keep the existing token, `Password::interact()` returns an empty `String`. Code that passes this directly to the auth call will fail with a 401.

**Why it happens:** `dialoguer::Password` has no `allow_empty_password(true).default()` combination that works like `Input::default()`. Empty password is treated as "user wants to keep existing".

**How to avoid:** After `Password::interact()`, check `if raw_pat.is_empty()` and fall back to the loaded config's token. See Pattern 2 above.

**Warning signs:** `ccli init` re-run always reports auth failure even with valid existing credentials.

---

### Pitfall 5: comfy-table NOTHING Preset Pads Cells (Not True TSV)

**What goes wrong:** Using `comfy_table::presets::NOTHING` for tab-separated output — the preset removes borders but still pads cells with spaces to align columns. Output is space-padded, not tab-delimited, breaking `awk`, `cut`, `sort -t$'\t'` pipelines.

**Why it happens:** `NOTHING` is a "no cosmetics" aligned display, not a raw delimiter format.

**How to avoid:** For TSV output (--plain / piped), bypass comfy-table entirely and use a simple `fields.join("\t")` per row. See Pattern 5 (tsv.rs module).

**Warning signs:** `ccli space list --plain | cut -f2` returns nothing because fields are space-padded.

---

### Pitfall 6: Atomic Config Write Required

**What goes wrong:** Using `std::fs::write(path, content)` directly — if the process is killed mid-write, the config file is truncated and corrupt. Next run of `ccli` fails to parse it.

**Why it happens:** `write()` truncates the file before writing new content.

**How to avoid:** Write to `config.toml.tmp`, then `std::fs::rename()` to `config.toml`. Rename is atomic on POSIX systems. On Windows, this requires both files to be on the same volume (they will be, since both are under `~/.config/ccli/`).

---

## Code Examples

Verified patterns from official sources:

### Tokio async main with clap

```rust
// Source: tokio docs + clap 4.6 derive [VERIFIED: crates.io + docs.rs]
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"))
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            if let Err(e) = run_init(&cli).await {
                handle_error(&e.downcast::<AppError>().unwrap_or_else(|e| {
                    AppError::Api(e.to_string()).into()
                }));
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
```

### reqwest Client setup with timeout

```rust
// Source: reqwest 0.13 docs [VERIFIED: crates.io]
use reqwest::{Client, header};
use std::time::Duration;

pub fn build_client(token: &str) -> anyhow::Result<Client> {
    let mut headers = header::HeaderMap::new();
    let auth_value = header::HeaderValue::from_str(&format!("Bearer {}", token))?;
    headers.insert(header::AUTHORIZATION, auth_value);

    Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(Into::into)
}
```

### TTY detection

```rust
// Source: is-terminal 0.4 [VERIFIED: crates.io]
use is_terminal::IsTerminal;
let is_tty: bool = std::io::stdout().is_terminal();
```

### comfy-table aligned table

```rust
// Source: comfy-table 7.2 README [VERIFIED: GitHub]
use comfy_table::{Table, ContentArrangement};
use comfy_table::presets::UTF8_FULL_CONDENSED;

let mut table = Table::new();
table
    .load_preset(UTF8_FULL_CONDENSED)
    .set_content_arrangement(ContentArrangement::Dynamic)
    .set_header(vec!["Key", "Name", "Type"])
    .add_row(vec!["DEV", "Development", "global"]);

println!("{table}");
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `atty` for TTY detection | `is-terminal` | 2021 (RUSTSEC advisory) | Drop-in API replacement; no behavior change |
| `reqwest` default native-tls | `reqwest` default rustls | reqwest 0.13.0 (Dec 2025) | Self-signed cert handling differs; see pitfall 3 |
| `thiserror` 1.x | `thiserror` 2.x | 2024 | Major version bump; derive macro improved; semver-compatible for most patterns |
| `dirs::config_dir()` on macOS returns `~/.config` | Returns `~/Library/Application Support` | dirs 6.0 (Jan 2025) | Must use `home::home_dir()` for explicit `~/.config` path |
| `prettytable-rs` for tables | `comfy-table` or `tabled` | 2022 (RUSTSEC advisory) | prettytable-rs has unsoundness advisory; abandoned |

**Deprecated/outdated:**
- `atty`: RUSTSEC-2021-0145; unmaintained since 2020 — use `is-terminal`.
- `prettytable-rs`: RUSTSEC-2022-0074 unsoundness; abandoned 2022 — use `comfy-table` or `tabled`.
- `reqwest::ClientBuilder::use_rustls_tls()`: renamed to `tls_backend_rustls()` in 0.13 (soft deprecation, old name still works).

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `GET /rest/api/user/current` returns 401 for invalid PAT reliably (vs. 200 or 404) | Pattern 4, Pitfall 2 | Connection test gives false success; save broken config. Mitigation: also check response body for `username` field |
| A2 | Confluence DC 9.2.19 supports `Authorization: Bearer <token>` header (not just Basic auth) | Pattern 4 | Auth fails entirely; would need Basic auth fallback |
| A3 | PAT format for Confluence DC 9.2.19 starts with `AT` prefix (used in mask hint) | Pattern 2 | Masking hint looks wrong; UX issue only, not functional |

**A2 is LOW risk** — official Atlassian docs explicitly document Bearer token format. [CITED: confluence.atlassian.com/enterprise/using-personal-access-tokens-1026032365.html]

---

## Open Questions

1. **Self-signed certificates on Confluence DC instances**
   - What we know: reqwest 0.13 defaults to rustls which does not use the OS trust store
   - What's unclear: Whether target DC instances use valid certs or self-signed
   - Recommendation: Add `--insecure` / `CCLI_INSECURE=1` flag in Phase 1 to add `danger_accept_invalid_certs(true)` on the reqwest client; document the security tradeoff

2. **PAT token format prefix**
   - What we know: Atlassian DC PATs historically start with `AT` or similar prefix depending on version
   - What's unclear: Exact prefix for DC 9.2.19 (could vary)
   - Recommendation: Make mask_pat_hint generic (first 2 chars + *****  + last 3 chars) rather than checking for `AT` prefix

3. **Response timeout appropriate for slow DC instances**
   - What we know: 30-second timeout is common for CLIs
   - What's unclear: Whether enterprise DC instances behind VPNs are slower
   - Recommendation: 30s connect + 30s read is reasonable for v1; make configurable in v2

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/Cargo | Build | Available | 1.95.0 (2026-04-14) | — |
| rustc | Build | Available | 1.95.0 | — |
| Internet/crates.io | cargo fetch | Assumed | — | Vendor deps |
| Confluence DC instance | Integration test | Unknown | 9.2.19 (target) | Use mock server for unit tests |

**Missing dependencies with no fallback:**
- Live Confluence DC 9.2.19 instance for PAT auth validation testing — the connection test in `ccli init` cannot be meaningfully integration-tested without a real or Docker-based instance.

**Missing dependencies with fallback:**
- None for Phase 1 build/run.

---

## Validation Architecture

> `workflow.nyquist_validation` is `false` in config.json — this section is SKIPPED per config.

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | PAT stored in config.toml (file permissions 0600 recommended) |
| V3 Session Management | no | CLI is stateless; no sessions |
| V4 Access Control | no | Single-user CLI; no multi-user access |
| V5 Input Validation | yes | URL validation in dialoguer prompt; token non-empty check |
| V6 Cryptography | no | No encryption in Phase 1; PAT stored in plaintext |

### Known Threat Patterns for Rust CLI / Confluence PAT

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| PAT exposed in process list (env vars) | Information Disclosure | CCLI_TOKEN env var is user-scoped; document not to use in CI without secrets management |
| Config file readable by other users | Information Disclosure | `std::fs::set_permissions(path, Permissions::from_mode(0o600))` after writing |
| PAT logged in debug output | Information Disclosure | Never log Authorization header value; log only method + URL in tracing calls |
| URL injection in API calls | Tampering | Validate URL scheme (http/https only) in dialoguer prompt; use reqwest URL builder not string concat |

**Config file permissions note:** After `config::save()`, set file permissions to `0600` (owner read/write only) on Unix. On Windows, the config dir's ACL is user-scoped by default.

---

## Sources

### Primary (HIGH confidence)
- crates.io API — versions confirmed for all crates (2026-04-24)
- docs.rs/clap/4.6.1 — `#[arg(global = true)]` syntax confirmed
- docs.rs/dialoguer/0.12.0 — `Input::with_initial_text()`, `Password::allow_empty_password()` confirmed
- docs.rs/dirs/6.0.0 — `config_dir()` macOS behavior confirmed (Application Support, not ~/.config)
- GitHub/Nukesor/comfy-table presets.rs — `NOTHING` preset definition confirmed; `tty` feature confirmed
- GitHub/Nukesor/comfy-table commits — last commit March 2026; actively maintained
- GitHub/zhiburt/tabled commits — last commit April 2026; actively maintained
- reqwest CHANGELOG — 0.13 TLS rustls-by-default breaking change confirmed
- RustSec advisory DB — RUSTSEC-2021-0145 (atty) and RUSTSEC-2022-0074 (prettytable-rs) confirmed

### Secondary (MEDIUM confidence)
- Atlassian documentation (confluence.atlassian.com) — PAT Bearer token header format confirmed
- Atlassian Jira (CONFSERVER-87540) — Confluence DC inconsistent 401 behavior for invalid PAT documented
- docs.atlassian.com/atlassian-confluence/REST/6.6.0 — `/rest/api/user/current` response fields (username, displayName, userKey, 200/403 status codes)

### Tertiary (LOW confidence)
- Community reports — Confluence DC PAT prefix format (AT****) and exact 401 response JSON shape for 9.2.19 not directly verified

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions verified against crates.io on 2026-04-24
- Architecture: HIGH — patterns drawn from official docs and crate examples
- Confluence API behavior: MEDIUM — official docs for user/current endpoint; known bug verified via Atlassian issue tracker
- PAT error response body format: LOW — exact JSON for invalid PAT in DC 9.2.19 not directly verified (live instance required)

**Research date:** 2026-04-24
**Valid until:** 2026-07-24 (90 days — stable crates, but reqwest/clap release frequently)
