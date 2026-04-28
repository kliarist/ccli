# Agents

**ccli** — Confluence Data Center CLI (Rust, async).

## Build & Test

```bash
cargo build          # debug build
cargo build --release
cargo test          # run all tests
cargo test -- --nocapture
cargo run -- init   # run the CLI
```

## Project Structure

```
src/
  main.rs              # Entry point, error dispatch, env tracing
  cli/                 # Command parser & handlers
    mod.rs             # Clap parser, global flags (--plain, --no-headers, --columns)
    init.rs            # Init command: interactive setup
  api/                 # Confluence API client & error types
    mod.rs             # Re-exports Client, AppError
    client.rs          # HTTP client for Confluence API
    error.rs           # AppError enum (Auth, Network, Config, Api)
  config/              # Config persistence & loading
    mod.rs             # Config struct, load/save, env var overlays
    path.rs            # Config file path resolution (~/.ccli/config.toml)
  output/              # Tabular output formatting
    mod.rs             # OutputConfig, table rendering
```

## Key Patterns

- **Error Handling (D-04)**: `AppError` wraps all domain errors; anyhow chains preserve category through `.context()` calls
- **Config Loading (W-06)**: Precedence: file + env overlay > env-only (when both `CCLI_URL`+`CCLI_TOKEN` set) > none
- **Config Permissions**: Unix mode 0o600 after save (atomic write via `.toml.tmp` rename)
- **Global Flags**: `--plain`, `--no-headers`, `--columns` auto-inherit to all subcommands via clap `global = true`
- **Output**: TSV when piped, pretty table when interactive (via `is-terminal` crate)

## Commands

- `ccli init` — Interactive setup: configure URL + PAT, validate connection, save config
- `ccli <cmd> --plain --no-headers --columns key,name` — Output formatting options (inherited)

## Environment Variables

- `CCLI_URL` — Confluence instance URL (overrides config file)
- `CCLI_TOKEN` — Personal Access Token (overrides config file)
- `RUST_LOG` — Tracing level (default: `warn`)

## Config File

Location: `~/.ccli/config.toml`

```toml
url = "https://confluence.example.com"
token = "AT-xxxxxxxxxxxx"
```

## Testing

- Unit tests embed `tempfile`, `httpmock` for isolated file/HTTP simulation
- Use `ENV_LOCK: Mutex<()>` + `EnvIsolation` guard to serialize env var mutations
- Tests verify: config serialization, env var override precedence, atomic saves, permission bits
