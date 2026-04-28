# Phase 2: TUI Shell & Spaces - Pattern Map

**Mapped:** 2026-04-27
**Files analyzed:** 8 new/modified files
**Analogs found:** 8 / 8

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/api/space.rs` | service | request-response (paginated CRUD) | `src/api/client.rs` | role-match |
| `src/cli/space.rs` | handler | request-response | `src/cli/init.rs` | role-match |
| `src/cli/mod.rs` | config | request-response | `src/cli/mod.rs` (modify) | exact |
| `src/main.rs` | entrypoint | request-response | `src/main.rs` (modify) | exact |
| `src/tui/mod.rs` | service | event-driven | `src/cli/init.rs` (async fn run pattern) | partial |
| `src/tui/app.rs` | model/store | event-driven | `src/cli/init.rs` (state ownership pattern) | partial |
| `src/tui/screens/spaces.rs` | component | event-driven | `src/output/table.rs` (pure render fn) | partial |
| `src/api/mod.rs` | config | — | `src/api/mod.rs` (modify) | exact |

---

## Pattern Assignments

### `src/api/space.rs` (service, paginated request-response)

**Analog:** `src/api/client.rs`

**Imports pattern** (`src/api/client.rs` lines 17-23):
```rust
use std::time::Duration;

use reqwest::{header, Client as ReqwestClient};
use tracing::{debug, instrument};

use crate::api::error::AppError;
use crate::config::Config;
```

For `space.rs`, imports follow the same convention — use `crate::api::error::AppError`, pull in `serde::Deserialize`, and reference `crate::api::client::Client`:
```rust
use serde::Deserialize;
use tracing::instrument;

use crate::api::client::Client;
use crate::api::error::AppError;
```

**Instrument/security pattern** (`src/api/client.rs` lines 96-100):
```rust
/// Probe `GET /rest/api/user/current` to validate the PAT.
#[instrument(skip(token))]
pub async fn test_connection(base_url: &str, token: &str) -> Result<String, AppError> {
    let trimmed = base_url.trim_end_matches('/');
    let url = format!("{}/rest/api/user/current", trimmed);
    debug!("PAT validation probe: GET {}", url); // never log the token value
```

For `space.rs`, always `#[instrument(skip(client))]` — the client holds the PAT and must never be logged. Log only the URL:
```rust
#[instrument(skip(client))]
pub async fn list_all_spaces(client: &Client) -> Result<Vec<Space>, AppError> {
    // ...
    debug!("Fetching spaces page: GET {}", url);
```

**Core HTTP call + error mapping pattern** (`src/api/client.rs` lines 103-134):
```rust
let resp = client
    .get(&url)
    .send()
    .await
    .map_err(|e| {
        if e.is_connect() || e.is_timeout() {
            AppError::Network(format!("Cannot reach {}: {}", trimmed, e))
        } else {
            AppError::Network(e.to_string())
        }
    })?;

match resp.status().as_u16() {
    200 => {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        // ...
    }
    401 => Err(AppError::Auth("Invalid or expired token. Check your PAT.".to_string())),
    403 => Err(AppError::Auth("Authenticated but access denied. Check your PAT permissions.".to_string())),
    status => Err(AppError::Api(format!("Unexpected HTTP {}", status))),
}
```

For `space.rs`, apply the same `.map_err` chains. Use `.json::<T>()` for typed deserialization, mapping parse errors to `AppError::Api`:
```rust
let resp: SpaceListResponse = client.inner()
    .get(&url)
    .send()
    .await
    .map_err(|e| AppError::Network(e.to_string()))?
    .json()
    .await
    .map_err(|e| AppError::Api(format!("Failed to parse space list: {}", e)))?;
```

**Test pattern** (`src/api/client.rs` lines 137-291) — use `httpmock::prelude::*`, one `#[tokio::test]` per status code variant:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn list_all_spaces_returns_sorted_vec_on_200() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/space");
            then.status(200).json_body(serde_json::json!({
                "results": [{"id": 1, "key": "DEV", "name": "Development", "_links": {}}],
                "start": 0, "limit": 25, "size": 1,
                "_links": {}
            }));
        });
        // ...
    }
}
```

**Client reuse rule** (`src/api/client.rs` lines 53-60): Always call `client.inner()` to get the underlying `reqwest::Client` — never instantiate a new one per request:
```rust
pub fn inner(&self) -> &ReqwestClient {
    &self.inner
}

pub fn base_url(&self) -> &str {
    &self.base_url
}
```

---

### `src/cli/space.rs` (handler, request-response)

**Analog:** `src/cli/init.rs`

**Imports pattern** (`src/cli/init.rs` lines 15-21):
```rust
use anyhow::Context;
// ...
use crate::api;
use crate::cli::Cli;
use crate::config::{self, Config};
```

For `space.rs`:
```rust
use anyhow::Context;

use crate::api::space::SpaceClient;
use crate::cli::Cli;
use crate::config;
use crate::output::OutputFormatter;
```

**Handler signature pattern** (`src/cli/init.rs` line 24):
```rust
pub async fn run(_cli: &Cli) -> anyhow::Result<()> {
```

For `space.rs`, the handler receives `&Cli` (for `output_config()`) and a `SpaceArgs` struct:
```rust
pub async fn run(cli: &Cli, _args: &SpaceArgs) -> anyhow::Result<()> {
```

**Config load + context pattern** (`src/cli/init.rs` line 25):
```rust
let existing = config::load().context("Failed to load existing config")?;
```

For `space.rs`, use `config::load_or_error()` (not `load()`) since absence is an error in this command:
```rust
let config = config::load_or_error()
    .map_err(anyhow::Error::from)
    .context("Failed to load config")?;
```

**OutputFormatter usage** (`src/output/mod.rs` lines 41-45):
```rust
pub fn print(&self, headers: &[&str], rows: &[Vec<String>]) {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let _ = self.print_to(&mut handle, headers, rows);
}
```

In `space.rs`, build headers + rows from `Vec<Space>`, then call `print()`:
```rust
let formatter = OutputFormatter::new(cli.output_config());
let headers = &["Key", "Name", "Type"];
let rows: Vec<Vec<String>> = spaces.iter()
    .map(|s| vec![
        s.key.clone(),
        s.name.clone(),
        s.space_type.clone().unwrap_or_default(),
    ])
    .collect();
formatter.print(headers, &rows);
```

---

### `src/cli/mod.rs` (modify existing — config, request-response)

**Analog:** `src/cli/mod.rs` (self)

**Current `Commands` enum** (lines 39-46):
```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Configure Confluence URL and Personal Access Token interactively.
    Init,
    // Phase 2 will add: Space(SpaceArgs)
}
```

**Modification required — add `Space` variant and make `command` optional** (per D-10):

Change line 36:
```rust
// BEFORE (line 36):
#[command(subcommand)]
pub command: Commands,

// AFTER (D-10):
#[command(subcommand)]
pub command: Option<Commands>,
```

Add to `Commands` enum:
```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Configure Confluence URL and Personal Access Token interactively.
    Init,
    /// List and browse Confluence spaces.
    Space(SpaceArgs),
}

#[derive(clap::Args, Debug)]
pub struct SpaceArgs {
    #[command(subcommand)]
    pub command: SpaceCommands,
}

#[derive(Subcommand, Debug)]
pub enum SpaceCommands {
    /// Print all accessible spaces as a table.
    List,
}
```

**Test to update** (lines 99-103) — `missing_subcommand_errors` must flip per Pitfall 2 from RESEARCH.md:
```rust
// BEFORE:
#[test]
fn missing_subcommand_errors() {
    let result = Cli::try_parse_from(["ccli"]);
    assert!(result.is_err(), "clap should require a subcommand");
}

// AFTER (D-10 behavior):
#[test]
fn no_subcommand_launches_tui() {
    let result = Cli::try_parse_from(["ccli"]);
    assert!(result.is_ok(), "bare ccli should parse successfully (D-10)");
    assert!(result.unwrap().command.is_none());
}
```

---

### `src/main.rs` (modify existing — entrypoint, request-response)

**Analog:** `src/main.rs` (self)

**Current dispatch** (lines 33-39):
```rust
async fn run() -> anyhow::Result<()> {
    let cli = <Cli as clap::Parser>::parse();

    match cli.command {
        Commands::Init => cli::init::run(&cli).await,
    }
}
```

**Modification required — add `tui` module reference + `Option<Commands>` dispatch** (per D-10):
```rust
mod tui;

// ...

async fn run() -> anyhow::Result<()> {
    let cli = <Cli as clap::Parser>::parse();

    match cli.command {
        Some(Commands::Init) => cli::init::run(&cli).await,
        Some(Commands::Space(args)) => match args.command {
            cli::SpaceCommands::List => cli::space::run(&cli, &args).await,
        },
        None => tui::run().await,  // D-10: bare ccli launches TUI
    }
}
```

**Error handling pattern stays unchanged** (lines 25-29) — `hint_for()` and the anyhow chain walk need no modification since no new `AppError` variants are added:
```rust
let app_err = err.chain().find_map(|e| e.downcast_ref::<AppError>())
    .cloned()
    .unwrap_or_else(|| AppError::Api(err.to_string()));
handle_error(&app_err);
```

---

### `src/tui/mod.rs` (service, event-driven)

**Analog:** `src/cli/init.rs` (closest for `pub async fn run()` entry pattern)

**Module-level doc comment pattern** (`src/cli/init.rs` lines 1-13):
```rust
//! `ccli init` interactive setup flow.
//!
//! Locked decisions implemented:
//! - D-01 dialoguer for prompts
//! - D-02 connection test BEFORE save
//! ...
//!
//! Security:
//! - Token value never appears in stdout/stderr or tracing output.
```

For `tui/mod.rs`, follow the same header style:
```rust
//! TUI shell — terminal lifecycle, event loop, and top-level dispatch.
//!
//! Locked decisions implemented:
//! - D-10: bare `ccli` launches this TUI on the Spaces browse view
//! - D-12: `q` always quits; `Esc` closes overlays
//! - D-13: static help bar + `?` modal
//!
//! Terminal lifecycle:
//! - `ratatui::init()` handles raw mode, alternate screen, AND panic hook.
//! - `ratatui::restore()` undoes all of it — always called, even on error.
```

**Public async `run()` entry convention** (`src/cli/init.rs` line 24):
```rust
pub async fn run(_cli: &Cli) -> anyhow::Result<()> {
```

For `tui/mod.rs`:
```rust
pub async fn run() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal).await;
    ratatui::restore();
    result
}
```

**Config loading pattern** (`src/cli/init.rs` lines 25-26):
```rust
let existing = config::load().context("Failed to load existing config")?;
```

TUI must load config at startup before spawning fetch tasks:
```rust
let config = config::load_or_error()
    .map_err(anyhow::Error::from)
    .context("Run 'ccli init' to configure before using the TUI")?;
let client = Client::new(&config)?;
```

---

### `src/tui/app.rs` (model/store, event-driven)

**Analog:** No direct existing analog — this is a new pattern for this codebase. Use RESEARCH.md Pattern 6 as the template.

The closest structural analog is `src/output/mod.rs`'s `OutputFormatter` struct, which demonstrates the project's idiom of pairing a struct with owned state and an `impl` block containing behavior methods.

**Struct + impl pattern** (`src/output/mod.rs` lines 22-45):
```rust
pub struct OutputFormatter {
    config: OutputConfig,
    is_tty: bool,
}

impl OutputFormatter {
    pub fn new(config: OutputConfig) -> Self {
        let is_tty = std::io::stdout().is_terminal();
        Self { config, is_tty }
    }

    #[cfg(test)]
    pub(crate) fn with_tty(config: OutputConfig, is_tty: bool) -> Self {
        Self { config, is_tty }
    }
```

For `App` in `tui/app.rs`, follow the same `new()` + optional test constructor pattern:
```rust
pub struct App {
    pub spaces: Vec<Space>,
    pub list_state: ListState,
    pub filtered_indices: Vec<usize>,
    pub state: AppState,
    pub preview_cache: HashMap<String, SpaceDetail>,
    pub pending_preview_key: Option<String>,
    pub last_selection_change: Option<Instant>,
    pub spinner_frame: usize,
    pub fetch_result: Option<Result<Vec<Space>, AppError>>,
}

impl App {
    pub fn new() -> Self { ... }

    /// Test constructor with pre-loaded spaces (no network fetch needed).
    #[cfg(test)]
    pub fn with_spaces(spaces: Vec<Space>) -> Self { ... }
}
```

**Decision-ID comment style** (throughout `src/api/client.rs`):
```rust
// D-08 tokio + reqwest async
// D-14: pre-fetch all spaces on launch
```
Continue this pattern in `app.rs` comments, e.g. `// D-15: real-time fuzzy filter`, `// D-19: 150ms debounce`.

---

### `src/tui/screens/spaces.rs` (component, event-driven)

**Analog:** `src/output/table.rs` (pure render function pattern)

**Pure render function pattern** (`src/output/table.rs` lines 21-42):
```rust
/// Testable variant of `render` returning the rendered table as a String.
pub(crate) fn render_string(
    headers: &[String],
    col_indices: &[usize],
    rows: &[Vec<String>],
    no_headers: bool,
) -> String {
    let mut table = Table::new();
    // ... pure computation, no side effects ...
    table.to_string()
}
```

For `spaces.rs`, the render function must be pure (no side effects, no I/O) and accept immutable borrows:
```rust
/// Pure render function — no I/O, no state mutation.
/// Called on every draw cycle from the event loop in tui/mod.rs.
pub fn render_spaces(f: &mut Frame, app: &App, area: Rect) {
    // ... layout + widget rendering only ...
}
```

**Module-level doc comment pattern** (`src/output/table.rs` lines 1-6):
```rust
//! Aligned terminal table renderer using comfy-table.
//!
//! Used when stdout is a TTY and `--plain` is not set. Always uses
//! `UTF8_FULL_CONDENSED` preset with `ContentArrangement::Dynamic` for
//! responsive width handling (RESEARCH.md Pattern 5 + Code Examples).
```

For `screens/spaces.rs`:
```rust
//! Spaces browse view renderer — pure drawing functions for the TUI.
//!
//! Requirements covered: TUI-01..06, SPC-01..SPC-03.
//! All functions are pure: they take `&App` and `&mut Frame` and produce no side effects.
//! This makes them testable without a real terminal (state-only unit tests).
```

**Testable seam pattern** (`src/output/mod.rs` lines 50-56) — the `print_to<W: Write>` seam makes the output formatter testable without stdout:
```rust
pub(crate) fn print_to<W: std::io::Write>(
    &self,
    writer: &mut W,
    headers: &[&str],
    rows: &[Vec<String>],
) -> std::io::Result<()> {
```

For `screens/spaces.rs`, expose a testable seam via pure state assertions — the `App` struct's fields are the observable state, and tests can verify `app.list_state.selected()`, `app.filtered_indices`, etc. without needing a terminal.

---

## Shared Patterns

### Tracing + Token Safety
**Source:** `src/api/client.rs` lines 7-16, 96-100
**Apply to:** `src/api/space.rs`, `src/tui/mod.rs`
```rust
// Module-level security note:
// Security:
// - The token NEVER appears in tracing logs. `#[instrument(skip(client))]` is
//   applied to all public async functions. The client holds the PAT in the
//   Authorization header; that header value is not logged.

#[instrument(skip(client))]
pub async fn list_all_spaces(client: &Client) -> Result<Vec<Space>, AppError> {
    debug!("Fetching spaces page: GET {}", url); // URL only, never token
```

### AppError Mapping
**Source:** `src/api/error.rs` lines 1-16, `src/api/client.rs` lines 108-134
**Apply to:** `src/api/space.rs`, `src/tui/mod.rs`, `src/tui/app.rs`
```rust
// Error classification rules (established in Phase 1):
// - TCP connect failure / timeout → AppError::Network
// - HTTP 401 / 403 → AppError::Auth
// - JSON parse failure → AppError::Api
// - HTTP 4xx/5xx other → AppError::Api
// - Missing config → AppError::Config (from config::load_or_error)

.map_err(|e| {
    if e.is_connect() || e.is_timeout() {
        AppError::Network(format!("Cannot reach server: {}", e))
    } else {
        AppError::Network(e.to_string())
    }
})?
```

### Anyhow `.context()` Wrapping
**Source:** `src/cli/init.rs` lines 36-38, `src/main.rs` lines 25-27
**Apply to:** `src/cli/space.rs`, `src/tui/mod.rs`
```rust
// All fallible operations use .context() to add caller-level meaning.
// main.rs's chain-walk pattern recovers the inner AppError through wrappers.
let display_name = api::test_connection(&url, &token)
    .await
    .context("Connection test failed")?;
```

### Decision-ID Comment Convention
**Source:** `src/api/client.rs` lines 1-16, `src/cli/init.rs` lines 1-13
**Apply to:** All new Phase 2 files
```rust
//! Locked decisions implemented:
//! - D-14: pre-fetch all spaces on TUI launch
//! - D-15: real-time fuzzy filter on `/`
```
Phase 2 decisions run D-10 through D-25. Every implementation choice that corresponds to a CONTEXT.md decision must cite the `D-NN` marker in a module doc comment or inline comment.

### Testable Seam Convention
**Source:** `src/output/mod.rs` lines 36-56, `src/output/table.rs` lines 21-42
**Apply to:** `src/api/space.rs`, `src/tui/screens/spaces.rs`
```rust
// Production function: calls internal testable variant.
pub fn print(&self, headers: &[&str], rows: &[Vec<String>]) { ... }

// Testable variant: accepts explicit writer / state, no stdout/terminal needed.
pub(crate) fn print_to<W: std::io::Write>(&self, writer: &mut W, ...) { ... }
```
For the TUI, the equivalent is: public `run()` handles terminal lifecycle; testable `App` methods accept pre-loaded state and are exercised without a terminal.

### httpmock Test Pattern
**Source:** `src/api/client.rs` lines 137-145, 185-193
**Apply to:** `src/api/space.rs` tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn test_name() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/space");
            then.status(200).json_body(serde_json::json!({ ... }));
        });
        let client = /* build Client pointing at server.base_url() */;
        let result = list_all_spaces(&client).await;
        // assert ...
    }
}
```

---

## No Analog Found

All files have analogs in the existing codebase. The TUI-specific files (`tui/mod.rs`, `tui/app.rs`, `tui/screens/spaces.rs`) are genuinely new patterns for this project, but their structural conventions (public `run()` entry, pure render functions, testable seams, doc comment style, decision-ID annotations) are all derived directly from existing Phase 1 code.

| File | Notes |
|------|-------|
| `src/tui/mod.rs` | Event loop is new pattern; entry/teardown conventions copied from `cli/init.rs` |
| `src/tui/app.rs` | State machine is new; struct+impl+test-constructor pattern from `output/mod.rs` |
| `src/tui/screens/spaces.rs` | Ratatui rendering is new; pure-function pattern from `output/table.rs` |

For the Ratatui-specific APIs (Layout, List, ListState, Paragraph, Clear, Block), use the code examples in RESEARCH.md Patterns 1–9 directly — those are verified against docs.rs/ratatui 0.30.

---

## Metadata

**Analog search scope:** `src/api/`, `src/cli/`, `src/output/`, `src/config/`, `src/main.rs`
**Files scanned:** 11
**Pattern extraction date:** 2026-04-27
