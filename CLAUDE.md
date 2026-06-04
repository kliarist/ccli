# CLAUDE.md

**ccli** — Confluence Data Center / Cloud CLI (Rust, async). Bare `ccli` launches a TUI; subcommands are non-interactive CLI.

## CI requirements — must pass before every commit

**Always run `cargo fmt --all` before committing.** CI runs `cargo fmt --all -- --check` and fails on any diff. `cargo build` and `cargo test` do not enforce formatting — the check is CI-only, so it is easy to miss locally.

```bash
# Full pre-commit checklist (run all three, in order)
cargo fmt --all
cargo clippy -- -D warnings
cargo test
```

## Common commands

```bash
# Build
cargo build
cargo build --release

# Run (bare = TUI; subcommand = CLI)
cargo run
cargo run -- space list
cargo run -- page --space KEY

# Tests
cargo test
cargo test <test_name>          # run a single test by name (substring match)
cargo test -- --nocapture       # show println!/eprintln! output

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt --all

# Coverage (requires cargo-llvm-cov)
cargo llvm-cov                              # terminal summary
cargo llvm-cov --html && open target/llvm-cov/html/index.html  # HTML report
cargo llvm-cov -- --test-threads=1 --nocapture  # debug a hanging test
```

Config lives at `~/.config/ccli/config.toml`. Override with env vars `CCLI_URL` and `CCLI_TOKEN`. Enable debug logging with `RUST_LOG=debug`.

## Module layout

```
src/
  main.rs          — entry point, error formatting, CLI dispatch
  edit.rs          — pandoc-backed Markdown editor session (shared by CLI and TUI)
  config/          — load/save ~/.config/ccli/config.toml
  api/
    client.rs      — Arc<reqwest::Client> wrapper, auth headers, base URL
    error.rs       — AppError enum (Auth/Network/Config/Api)
    space.rs       — Space, SpaceDetail structs + fetch functions
    page.rs        — Page, PageDetail + fetch/update
    comment.rs     — Comment + fetch
    attachment.rs  — Attachment + upload
  cli/
    mod.rs         — clap Cli struct, Commands/SpaceCommands enums
    init.rs        — `ccli init` wizard (dialoguer)
    space.rs       — `ccli space list` (comfy-table output)
    page.rs        — `ccli page` (list/view/create/edit/search)
    blog.rs        — `ccli blog` (delegates to page.rs with ContentType::BlogPost)
    comment.rs     — `ccli comment`
    attachment.rs  — `ccli attachment`
  output/          — shared table/TSV/XML formatters + strip_storage_xml
  tui/
    mod.rs         — `tui::run()`: event loop, async tasks, crossterm setup/teardown
    app.rs         — App struct + AppState enum + all key handlers (no terminal I/O)
    screens/
      spaces.rs    — render_spaces() and helpers
      pages.rs     — render_pages() and helpers
      comments.rs  — render_comments() and helpers
```

## Architecture

### TUI

`tui::run()` owns the async runtime side: spawns tokio tasks that fetch data, sends results over `mpsc` channels, and drives a 100ms poll loop calling `terminal.draw(|f| render(f, &mut app))`.

`tui/app.rs` is **terminal-I/O-free** — all state is plain Rust. Key handlers return `KeyAction` (Quit / PushScreen / PopScreen / None); the event loop in `tui/mod.rs` acts on the return value. This makes app logic unit-testable without a real terminal (`ratatui::TestBackend`).

Screen navigation uses a **stack**: `App::screen_stack: Vec<Screen>`. Empty stack = Spaces view. `Enter` pushes `Screen::PagesBrowse { .. }`; `Esc` pops. Comments are pushed from Pages.

Each screen has its own browse-state sub-struct (`PagesBrowseState`, `CommentsBrowseState`) held inside `Screen` enum variants.

### Error handling

`AppError` (thiserror) has four variants: `Auth`, `Network`, `Config`, `Api`. `anyhow::Context` wraps errors in propagation. `main()` walks the full anyhow chain (`err.chain().find_map(downcast_ref::<AppError>)`) to recover the original variant for category-specific remediation hints — this survives `.context("…")` wrapping.

### Filtering

Real-time fuzzy filter uses `nucleo-matcher` on the haystack `"{key} {name}"`. Selection changes trigger a 150ms debounce before spawning a preview detail fetch; fetched details are cached in `App::preview_cache: HashMap<String, SpaceDetail>` for the session.

### API client

`api/client.rs` wraps `Arc<reqwest::Client>` so it is cheap to clone across async tasks. All API functions take `&Client` and return `Result<T, AppError>`. HTTP 401 maps to `AppError::Auth`; connection errors to `AppError::Network`.

### Page editing (`edit.rs`)

When `pandoc` is on `$PATH`, `page edit` / `blog edit` converts Confluence storage XML → Markdown before opening `$EDITOR` and converts back on save. Falls back to raw XML with a tip when pandoc is absent. CLI has a `--raw` flag to bypass pandoc explicitly.

## Key patterns

- **Output**: TSV when piped, pretty table when interactive (via `is-terminal` crate). `--plain` forces TSV even on a TTY.
- **Config precedence**: file + env overlay > env-only (when both `CCLI_URL` + `CCLI_TOKEN` set) > none.
- **Config permissions**: Unix mode 0o600 after save (atomic write via `.toml.tmp` rename).
- **Security**: `$EDITOR` is always launched via `Command::new(editor).arg(path)` — never through a shell.
- **Pagination**: uses `_links.next` absence + `size < limit` to detect last page (Confluence quirk).

## Environment variables

- `CCLI_URL` — Confluence instance URL (overrides config file)
- `CCLI_TOKEN` — Personal Access Token (overrides config file)
- `RUST_LOG` — Tracing level (default: `warn`)

## Config file

Location: `~/.config/ccli/config.toml`

```toml
url = "https://confluence.example.com"
token = "AT-xxxxxxxxxxxx"
```

## Testing

- `httpmock` for isolated HTTP simulation; `tempfile` for filesystem tests
- Tests in `tui/app.rs` use `ratatui::TestBackend` — no real terminal needed
- Env var tests serialize via `Mutex` guard to avoid cross-test interference
