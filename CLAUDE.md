# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## CI requirements — must pass before committing

**Always run `cargo fmt --all` before committing.** CI runs `cargo fmt --all -- --check` and will fail if formatting is off. `cargo build` and `cargo test` do not enforce formatting — the check is CI-only, so it's easy to miss locally.

```bash
# Full pre-commit check (run all three)
cargo fmt --all
cargo clippy -- -D warnings
cargo test
```

## Common commands

```bash
# Build
cargo build

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
cargo fmt
```

Config lives at `~/.config/ccli/config.toml`. Override with env vars `CCLI_URL` and `CCLI_TOKEN`. Enable debug logging with `RUST_LOG=debug`.

## Architecture

### Module layout

```
src/
  main.rs          — entry point, error formatting, CLI dispatch
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
    page.rs        — `ccli page`
    blog.rs        — `ccli blog`
    comment.rs     — `ccli comment`
    attachment.rs  — `ccli attachment`
  output/          — shared table/json formatters
  tui/
    mod.rs         — `tui::run()`: event loop, async tasks, crossterm setup/teardown
    app.rs         — App struct + AppState enum + all key handlers (no terminal I/O)
    screens/
      spaces.rs    — render_spaces() and helpers
      pages.rs     — render_pages() and helpers
      comments.rs  — render_comments() and helpers
```

### TUI architecture

`tui::run()` owns the async runtime side: spawns a tokio task that fetches spaces, sends results over an `mpsc` channel, and drives a 100ms poll loop that calls `terminal.draw(|f| render(f, &mut app))`.

`tui/app.rs` is **terminal-I/O-free** — all state is plain Rust. Key handlers return `KeyAction` (Quit / PushScreen / PopScreen / None); the event loop in `tui/mod.rs` acts on the return value. This makes app logic unit-testable without a real terminal (`ratatui::TestBackend`).

Screen navigation uses a **stack**: `App::screen_stack: Vec<Screen>`. Empty stack = Spaces view. `Enter` pushes `Screen::PagesBrowse { .. }`; `Esc` pops. Comments are pushed from Pages.

Each screen has its own browse-state sub-struct (`PagesBrowseState`, `CommentsBrowseState`) held inside `Screen` enum variants, mirroring the top-level `AppState` pattern.

### Error handling

`AppError` (thiserror) has four variants. `anyhow::Context` wraps errors in propagation. `main()` walks the full anyhow chain (`err.chain().find_map(downcast_ref::<AppError>)`) to recover the original variant for category-specific remediation hints — this survives `.context("…")` wrapping.

### Filtering

Real-time fuzzy filter uses `nucleo-matcher` on the haystack `"{key} {name}"`. Selection changes trigger a 150ms debounce before spawning a preview detail fetch; fetched details are cached in `App::preview_cache: HashMap<String, SpaceDetail>` for the session.

### API client

`api/client.rs` wraps `Arc<reqwest::Client>` so it's cheap to clone across async tasks. All API functions take `&Client` and return `Result<T, AppError>`. HTTP 401 maps to `AppError::Auth`; connection errors to `AppError::Network`.
