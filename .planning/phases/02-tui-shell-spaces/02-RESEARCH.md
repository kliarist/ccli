# Phase 2: TUI Shell & Spaces - Research

**Researched:** 2026-04-27
**Domain:** Ratatui TUI framework, Crossterm terminal backend, Confluence DC REST API (spaces), async event loop with Tokio, nucleo fuzzy matching
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**TUI Entry & Lifecycle**
- D-10: Bare `ccli` (no subcommand) launches the TUI on the Spaces browse view. Phase 1 errors on no subcommand; this flips. Subcommands keep their existing non-interactive contract.
- D-11: `ccli space list` always prints a table — never opens the TUI. TTY → table, piped → TSV, `--plain` forces TSV (OUT-* contract from Phase 1).
- D-12: `q` always quits from any view. `Esc` closes the active overlay (filter input, `?` modal); only quits when at top-level view with no overlay open.
- D-13: Help UX = static bottom bar + `?` modal. Always-visible bottom strip shows most-used bindings; `?` opens a full modal for the current view.

**Space Fetch & Filter**
- D-14: Pre-fetch all spaces on launch. Walk every page of `/rest/api/space` (25/page) until exhausted, hold full list in memory. Spinner during initial fetch.
- D-15: Real-time fuzzy filter activates on `/`, re-filters on every keystroke, matches both space key and name. No submit-on-Enter. `Esc` closes filter and restores full list.
- D-16: Use the `nucleo` crate for fuzzy matching.
- D-17: `ccli space list` fetches every page and prints all rows. Same pagination walk as D-14.

**Preview Pane**
- D-18: Preview pane shows space metadata only: key, name, type, description, homepage URL. No page list.
- D-19: Fetch on selection with 150ms debounce + session cache (`HashMap<SpaceKey, SpaceDetail>`).
- D-20: Layout split: fixed 40% list / 60% preview, `Constraint::Percentage`.
- D-21: Empty/missing fields show fallbacks: dim `—` for missing values, `No description` when description is absent.

**Browser Open (`o`)**
- D-22: Use the `open` crate (Sebastian Thiel's).
- D-23: Headless/SSH fallback is reactive — always attempt `open::that()` first. On error: suspend TUI, print URL to stderr with "Press any key to continue.", wait for keypress, restore TUI.
- D-24: Resolve the URL from the API's `_links.webui` joined onto the configured base URL.
- D-25: No clipboard / copy-URL keybind in Phase 2.

### Claude's Discretion

- TUI architecture: module layout (`src/tui/`, `src/tui/app.rs`, `src/tui/screens/spaces.rs`, etc.), event-loop pattern (crossterm event reader + tokio `select!` over a fetch channel), tick rate.
- Panic-safe terminal restoration: install a panic hook calling `disable_raw_mode()` and `LeaveAlternateScreen` before the default panic prints.
- Default sort order in space list: sort by `key` ascending.
- Which space types to include: all types the API returns by default (global, personal, archived).
- Spinner / loading state: simple text indicator in status bar during page-walk.
- TUI default columns for space list (key, name).
- TUI color theme: monochrome with a single accent color (`Color::Cyan`).

### Deferred Ideas (OUT OF SCOPE)

- Clipboard / copy-URL (`c` or `y`) — needs `arboard`; deferred to Phase 3.
- Page list teaser inside space preview pane — Phase 3.
- Space-type filtering flag (`--type=global`) — v2.
- `--limit` / `--page` flags on `ccli space list` — deferred.
- Lazy-load space pagination in the TUI — deferred.
- TUI color themes — out of scope for v1.
- Dynamic responsive layout for narrow terminals — deferred.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TUI-01 | TUI list pane supports arrow-key navigation and `j`/`k` vim bindings | Ratatui `List` + `ListState` stateful widget handles selection tracking; `j`/`k` handled in event loop by calling `list_state.select_next()` / `select_previous()` |
| TUI-02 | TUI supports `/` to activate fuzzy search/filter within a list | `nucleo-matcher` `Pattern::parse` + `match_list` for real-time filtering; filter state managed in `AppState::Filter { query }` |
| TUI-03 | TUI displays a content preview pane alongside the list pane (split view) | Ratatui `Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])` + `Paragraph` widget for preview content |
| TUI-04 | TUI supports `o` keybinding to open the selected item in the system browser | `open` crate `open::that(url)` handles cross-platform browser launch; URL from `_links.webui` (D-24) |
| TUI-05 | TUI supports `q` or `Esc` to quit or go back | State machine: `q` → exit always; `Esc` → close overlay or exit at top-level (D-12) |
| TUI-06 | TUI displays a key-binding help bar at the bottom of the screen | Static `Paragraph` row with alternating styled `Span`s; `?` opens full modal using `Clear` + `Block` overlay |
| SPC-01 | User can list all accessible Confluence spaces in a TUI list view | Pre-fetch all pages of `GET /rest/api/space` into `Vec<Space>` on TUI launch |
| SPC-02 | User can fuzzy-filter the space list by space key or name | nucleo-matcher applied to `"{key} {name}"` combined haystack on every keystroke |
| SPC-03 | User can press `o` on a space to open it in the system browser | `open::that(url)` with URL from `space._links.webui` prefixed by `base_url` |
| SPC-04 | User can run `ccli space list` to output space list as a formatted table | Non-interactive handler in `cli/space.rs` using existing `OutputFormatter::print()` |
</phase_requirements>

---

## Summary

Phase 2 adds a full Ratatui TUI shell to `ccli` and delivers the Spaces feature through two surfaces: the interactive TUI browse view and the non-interactive `ccli space list` command. The foundation (Tokio, reqwest, clap, OutputFormatter) is already in place from Phase 1; this phase adds four new crates and three new source modules.

The TUI is built on Ratatui 0.30 with the Crossterm backend. Ratatui 0.30 ships a `ratatui::init()` convenience function that handles raw mode, alternate screen, and panic hook installation automatically — this replaces the manual setup pattern used in older tutorials. The event loop uses `crossterm::event::poll(Duration)` with a 100ms timeout for the spinner ticker, dispatching keyboard events through a state machine (`Loading` / `Browse` / `Filter` / `Modal`).

The Spaces API layer adds `src/api/space.rs` on top of the existing `Client` from Phase 1. The `GET /rest/api/space` endpoint returns a paginated `results[]` array with `start`, `limit`, `size`, and `_links.next` fields. All pages are pre-fetched before the TUI renders. Preview detail fetches `GET /rest/api/space/{key}?expand=description.plain,homepage` with a 150ms debounce and an in-memory session cache to avoid hammering the API during fast navigation. The URL used for `o` / browser open is resolved from `_links.webui` in the space API response — never hand-built.

The non-interactive `ccli space list` path mirrors the same pagination walk but writes directly to `OutputFormatter`, inheriting `--plain`, `--no-headers`, and `--columns` for free. The CLI entry point (`src/main.rs`) changes `command: Commands` to `command: Option<Commands>` and dispatches `None → tui::run()`.

**Primary recommendation:** Use `ratatui::init()` / `ratatui::restore()` for terminal lifecycle management (panic hook, raw mode, alternate screen), implement the event loop with `crossterm::event::poll(Duration::from_millis(100))` for the spinner ticker, and delegate fuzzy matching to `nucleo-matcher`'s `Pattern::parse` + `match_list` which handles the space list size comfortably on the main thread.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Terminal I/O, raw mode, alt screen | CLI binary (TUI layer) | — | Crossterm operates directly on the process's stdio |
| Event loop (keypress, tick) | CLI binary (TUI layer) | — | Synchronous crossterm::event::poll with 100ms timeout |
| Space list fetch (all pages) | API tier (`api/space.rs`) | — | reqwest + tokio; reuses Phase 1 `Client` |
| Preview detail fetch + cache | API tier (`api/space.rs`) | TUI layer (debounce) | HTTP in api module; 150ms debounce timer in TUI app |
| Fuzzy filter logic | TUI layer (`tui/app.rs`) | — | nucleo-matcher runs on main thread; list is small enough |
| Non-interactive table output | CLI tier (`cli/space.rs`) | Output tier | Reuses `OutputFormatter` from Phase 1 |
| Browser launch | TUI layer (`tui/app.rs`) | — | `open::that()` called directly; error handling in TUI layer |
| Render / draw | TUI layer (`tui/screens/spaces.rs`) | — | Pure function: `render_spaces(f, app, area)` |

---

## Standard Stack

### Core (New in Phase 2)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.30.0 | TUI framework: layout, widgets, rendering | Current stable; ships `init()`/`restore()` convenience; most active Rust TUI library [VERIFIED: cargo search] |
| crossterm | 0.29.0 | Terminal backend: raw mode, alt screen, events | Re-exported by ratatui-crossterm; cross-platform (Win/Mac/Linux/SSH) [VERIFIED: cargo search] |
| nucleo | 0.5.0 | Fuzzy matcher (fzf-style) | Used by Helix editor; Unicode-correct; locked by D-16 [VERIFIED: cargo search] |
| open | 5.3.4 | System browser launcher | Cross-platform (xdg-open/open/start); locked by D-22 [VERIFIED: cargo search] |

### Supporting (Already in Cargo.toml)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | 1 | Async runtime | Already wired; TUI event loop runs inside `#[tokio::main]` |
| reqwest | 0.13 | HTTP client | Space API calls via `Client::inner()` |
| serde / serde_json | 1 | JSON deserialization | Space API response structs |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| ratatui 0.30 | ratatui 0.29 | 0.30 ships `init()`/`restore()` which handles panic hook automatically; 0.29 requires manual setup — use 0.30 |
| nucleo | fuzzy-matcher (crate) | fuzzy-matcher is simpler but lower quality ranking; nucleo is the locked decision (D-16) |
| open crate | webbrowser crate | webbrowser has more deps; `open` is used by `cargo doc --open`; locked by D-22 |
| crossterm::event::poll | tokio::select! + crossterm EventStream | EventStream needs `crossterm/event-stream` feature; poll() is simpler and sufficient for 100ms tick rate |

**Installation:**

```bash
cargo add ratatui@0.30 crossterm@0.29 nucleo@0.5 open@5
```

**Version verification (confirmed 2026-04-27):**
- `ratatui` = 0.30.0 [VERIFIED: cargo search]
- `crossterm` = 0.29.0 [VERIFIED: cargo search]
- `nucleo` = 0.5.0 [VERIFIED: cargo search]
- `open` = 5.3.4 [VERIFIED: cargo search]

> Note: ratatui 0.30 re-exports crossterm via `ratatui-crossterm`. To avoid version conflicts, use the re-exported `crossterm` types through ratatui's re-export path when doing direct crossterm operations inside the TUI module. [CITED: https://github.com/ratatui/ratatui/blob/main/ratatui-crossterm/README.md]

---

## Architecture Patterns

### System Architecture Diagram

```
  ccli (no args)              ccli space list
       │                            │
       ▼                            ▼
  main.rs                    cli/space.rs
  Option<Commands>=None       SpaceHandler::run()
       │                            │
       ▼                            │
  tui::run()                        │
       │                            │
       ├── ratatui::init()           │
       ├── panic hook               │
       ├── AppState::Loading        │
       │                            │
       │         api/space.rs ◄─────┘
       │         SpaceClient
       │         │  list_all_spaces() ──► GET /rest/api/space?start=N&limit=25
       │         │  (paginate all pages)  (loop until no _links.next)
       │         │
       │         │  get_space_detail() ──► GET /rest/api/space/{key}
       │         │                         ?expand=description.plain,homepage
       │         │
       │    ┌────┴──────┐
       │    │  Vec<Space>│  (pre-fetched, sorted by key)
       │    │  Cache<Key,│  (session cache for detail)
       │    │  Detail>   │
       │    └─────┬──────┘
       │          │
       ▼          ▼
  Event Loop (crossterm::event::poll 100ms)
       │
       ├── KeyEvent ──► state machine dispatch
       │                  │
       │     AppState::Loading   → ignore keys (except q)
       │     AppState::Browse    → j/k/arrows, /, o, ?, q, Esc
       │     AppState::Filter{}  → chars append to query, Esc closes
       │     AppState::Modal     → Esc closes
       │
       ├── Tick (100ms) ──► advance spinner frame
       │
       └── terminal.draw() ──► tui/screens/spaces.rs
                                render_spaces(f, &app, area)
                                  │
                                  ├── Title Block
                                  ├── Layout::horizontal [40% | 60%]
                                  │     ├── List pane (ListState)
                                  │     └── Preview pane (Paragraph)
                                  ├── Status bar (1 row)
                                  └── Help bar (1 row)
                                  [overlay: Filter input OR Help modal]

  o key ──► open::that(url) ──► system browser
              │ Err ──► suspend TUI, print URL to stderr, await keypress, restore
```

### Recommended Project Structure

```
src/
├── tui/
│   ├── mod.rs          — pub async fn run(); panic hook, event loop, terminal lifecycle
│   ├── app.rs          — App struct: AppState enum, Vec<Space>, ListState, filter query, cache
│   └── screens/
│       └── spaces.rs   — render_spaces(f: &mut Frame, app: &App, area: Rect) — pure render fn
├── api/
│   └── space.rs        — SpaceClient: list_all_spaces(), get_space_detail(); Space / SpaceDetail structs
└── cli/
    └── space.rs        — handle_space_list(cli: &Cli) async fn — non-interactive ccli space list
```

### Pattern 1: Ratatui Terminal Lifecycle (0.30 style)

**What:** `ratatui::init()` handles raw mode, alternate screen, AND panic hook in one call. `ratatui::restore()` undoes all of it.

**When to use:** Always. Do not call `enable_raw_mode()` / `EnterAlternateScreen` manually — `ratatui::init()` does all three atomically and installs the panic hook.

```rust
// Source: https://docs.rs/ratatui/latest/ratatui/init/fn.init.html
// tui/mod.rs
pub async fn run() -> anyhow::Result<()> {
    // ratatui::init() enables raw mode, enters alternate screen,
    // AND installs a panic hook that calls restore() before panicking.
    let mut terminal = ratatui::init();

    let result = run_app(&mut terminal).await;

    // Always restore — even on error. ratatui::restore() is idempotent.
    ratatui::restore();

    result
}
```

> Note: If manual panic hook control is needed (e.g., to add extra cleanup), `ratatui::init()` still installs it. The UI-SPEC's manual panic hook pattern in the `Panic Safety Contract` is equivalent — both call `restore()` before the original hook. Using `ratatui::init()` is simpler and preferred. [CITED: https://docs.rs/ratatui/latest/ratatui/init/fn.init.html]

### Pattern 2: Event Loop with Spinner Tick

**What:** `crossterm::event::poll(Duration::from_millis(100))` drives both key event handling and the spinner tick. Returns `true` if an event is ready within the timeout; `false` on timeout (use timeout = tick advance).

**When to use:** This is the standard Ratatui + Crossterm event loop pattern when a ticker is needed without Tokio's `select!` complexity.

```rust
// Source: https://docs.rs/crossterm/latest/crossterm/event/fn.poll.html
// tui/mod.rs — event loop core
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

async fn run_app(terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
    let mut app = App::new();
    app.start_fetch().await; // spawn tokio task to fetch spaces

    loop {
        terminal.draw(|f| screens::spaces::render_spaces(f, &app, f.area()))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.handle_key(key.code).await? {
                        break; // quit signal
                    }
                }
            }
        } else {
            // Timeout — advance spinner, check for completed fetch
            app.tick();
        }
    }
    Ok(())
}
```

### Pattern 3: Clap Optional Subcommand (D-10)

**What:** Change `command: Commands` to `command: Option<Commands>` in the `Cli` struct to allow bare `ccli` to launch the TUI.

**When to use:** Required by D-10.

```rust
// Source: Phase 1 src/cli/mod.rs — needs this modification
#[derive(Parser, Debug)]
pub struct Cli {
    // ... existing output flags ...

    #[command(subcommand)]
    pub command: Option<Commands>,  // was: Commands
}

// src/main.rs dispatch
match cli.command {
    Some(Commands::Init) => cli::init::run(&cli).await,
    Some(Commands::Space(args)) => cli::space::run(&cli, &args).await,
    None => tui::run().await,  // D-10: bare ccli launches TUI
}
```

> Note: Making `command` optional means `clap` will no longer error on missing subcommand. The existing test `missing_subcommand_errors` in `cli/mod.rs` must be updated to assert `result.is_ok()` and that `.command` is `None`. [VERIFIED: codebase read — test at line 100 of src/cli/mod.rs]

### Pattern 4: Ratatui Stateful List with ListState

**What:** `List` widget with `ListState` for selection tracking. `render_stateful_widget` rather than `render_widget`.

**When to use:** Always for navigable lists.

```rust
// Source: https://docs.rs/ratatui/latest/ratatui/widgets/struct.List.html
use ratatui::widgets::{List, ListState, HighlightSpacing};
use ratatui::style::{Style, Color, Modifier};

fn render_list(f: &mut Frame, items: &[String], state: &mut ListState, area: Rect) {
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|s| ListItem::new(s.as_str()))
        .collect();

    let list = List::new(list_items)
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ")
        // HighlightSpacing::Always reserves the 2-char gutter even when nothing
        // is selected — prevents layout shift on navigation (from UI-SPEC)
        .highlight_spacing(HighlightSpacing::Always);

    f.render_stateful_widget(list, area, state);
}
```

### Pattern 5: Confluence Space API Pagination

**What:** Walk all pages of `GET /rest/api/space` using `start` offset. The response is a JSON object with `results[]`, `start`, `limit`, `size`, and `_links` (including `next` when more pages exist).

**When to use:** Both TUI pre-fetch (D-14) and `ccli space list` (D-17).

```rust
// Source: https://docs.atlassian.com/atlassian-confluence/REST/6.6.0/index
// api/space.rs
#[derive(serde::Deserialize)]
pub struct SpaceListResponse {
    pub results: Vec<Space>,
    pub start: u32,
    pub limit: u32,
    pub size: u32,       // count of results in THIS page
    #[serde(rename = "_links")]
    pub links: SpaceListLinks,
}

#[derive(serde::Deserialize)]
pub struct SpaceListLinks {
    pub next: Option<String>,   // present when more pages exist
    pub base: Option<String>,   // base URL e.g. "http://myhost:8080/confluence"
}

#[derive(serde::Deserialize, Clone)]
pub struct Space {
    pub id: u64,
    pub key: String,
    pub name: String,
    #[serde(rename = "type")]
    pub space_type: Option<String>,   // "global", "personal"
    #[serde(rename = "_links")]
    pub links: SpaceLinks,
}

#[derive(serde::Deserialize, Clone)]
pub struct SpaceLinks {
    pub webui: Option<String>,  // e.g. "/display/KEY" — relative path
    #[serde(rename = "self")]
    pub self_url: Option<String>,
}

pub async fn list_all_spaces(client: &Client) -> Result<Vec<Space>, AppError> {
    let mut all = Vec::new();
    let mut start = 0u32;
    let limit = 25u32;

    loop {
        let url = format!(
            "{}/rest/api/space?start={}&limit={}",
            client.base_url(), start, limit
        );
        let resp: SpaceListResponse = client.inner()
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?
            .json()
            .await
            .map_err(|e| AppError::Api(format!("Failed to parse space list: {}", e)))?;

        let fetched = resp.results.len() as u32;
        all.extend(resp.results);

        if resp.links.next.is_none() || fetched < limit {
            break;
        }
        start += limit;
    }

    // D-sort by key ascending (discretion)
    all.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(all)
}
```

### Pattern 6: Preview Detail Fetch with Debounce and Session Cache

**What:** On highlight change, wait 150ms then fetch detail. Cache in `HashMap<String, SpaceDetail>` for the session. Cancel pending fetch if selection changes again within 150ms.

**When to use:** Preview pane fetch (D-19).

```rust
// tui/app.rs — debounce via last_selection_change timestamp
use std::time::{Instant, Duration};
use std::collections::HashMap;

pub struct App {
    pub spaces: Vec<Space>,
    pub list_state: ListState,
    pub filtered_indices: Vec<usize>,  // indices into self.spaces for current filter
    pub state: AppState,
    pub preview_cache: HashMap<String, SpaceDetail>,
    pub pending_preview_key: Option<String>,
    pub last_selection_change: Option<Instant>,
    pub spinner_frame: usize,
    // ...
}

// In tick():
pub fn tick(&mut self) {
    self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();

    // Debounce: if selection settled for 150ms, request preview fetch
    if let Some(t) = self.last_selection_change {
        if t.elapsed() >= Duration::from_millis(150) {
            self.last_selection_change = None;
            // signal: preview fetch needed for currently selected key
            if let Some(key) = self.selected_key() {
                if !self.preview_cache.contains_key(&key) {
                    self.pending_preview_key = Some(key);
                }
            }
        }
    }
}
```

### Pattern 7: nucleo-matcher Fuzzy Filter

**What:** `nucleo-matcher` `Pattern::parse` + `match_list` for real-time list filtering. Small enough list (typical Confluence DC has <500 spaces) to run synchronously on the event loop thread.

**When to use:** D-15 / D-16 filter activation.

```rust
// Source: https://docs.rs/nucleo-matcher/0.3.1/nucleo_matcher/pattern/struct.Pattern.html
// tui/app.rs
use nucleo_matcher::{Matcher, Config};
use nucleo_matcher::pattern::{Pattern, CaseMatching, Normalization};

pub fn apply_filter(&mut self, query: &str) {
    if query.is_empty() {
        self.filtered_indices = (0..self.spaces.len()).collect();
        return;
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

    // Build haystack strings: "{key} {name}" for each space
    let haystacks: Vec<String> = self.spaces.iter()
        .map(|s| format!("{} {}", s.key, s.name))
        .collect();

    // match_list returns (item, score) sorted by score desc
    let matched: Vec<(String, u32)> = pattern.match_list(haystacks.iter().map(|s| s.as_str()), &mut matcher);

    // Recover original indices from matched strings
    self.filtered_indices = matched.iter().filter_map(|(haystack, _)| {
        haystacks.iter().position(|h| h == haystack)
    }).collect();
}
```

### Pattern 8: Help Modal with Clear Overlay

**What:** `Clear` widget erases the region before rendering the modal `Block`. Use `Rect::centered()` or manual centering.

**When to use:** `?` key opens the help modal (D-13).

```rust
// Source: https://docs.rs/ratatui/latest/ratatui/widgets/struct.Clear.html
// tui/screens/spaces.rs
use ratatui::widgets::{Clear, Block, Borders, BorderType, Paragraph};
use ratatui::layout::{Constraint, Rect};

fn render_help_modal(f: &mut Frame, area: Rect) {
    // Fixed 50x14 centered (from UI-SPEC)
    let modal_area = area.centered(
        Constraint::Length(50),
        Constraint::Length(14),
    );

    // Clear the area first so content below doesn't bleed through
    f.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Key Bindings ");

    f.render_widget(block, modal_area);
    // render help text Paragraph inside block.inner(modal_area)
}
```

### Pattern 9: Filter Overlay (Inline, not Floating)

**What:** Filter input is an inline block overlaid at the top of the list pane — 3 rows tall, same width as list pane. Does NOT shift the list layout.

**When to use:** `/` activates filter (D-15, UI-SPEC Filter Input Overlay).

```rust
// tui/screens/spaces.rs
// The overlay is rendered after the list, on top of the same Rect area.
// Compute a 3-row overlay at the top of the list pane:
let overlay_area = Rect {
    x: list_area.x,
    y: list_area.y,
    width: list_area.width,
    height: 3,  // border + input row + border
};

if matches!(app.state, AppState::Filter { .. }) {
    let query = app.filter_query();
    let display = format!("{}_", query);  // trailing _ as cursor (UI-SPEC uses █)
    let cursor_display = format!("{}█", query);

    let filter_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled("Filter", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));

    f.render_widget(Clear, overlay_area);
    f.render_widget(
        Paragraph::new(cursor_display).block(filter_block),
        overlay_area,
    );
}
```

### Anti-Patterns to Avoid

- **Manual raw mode setup:** Do not call `enable_raw_mode()` + `EnterAlternateScreen` manually in Phase 2. Use `ratatui::init()` — it does all three (raw mode, alt screen, panic hook) atomically. [CITED: docs.rs/ratatui]
- **Building space URL by hand:** Never construct `<base_url>/display/<KEY>`. Always use `space._links.webui` from the API response, which is correct for instances with custom context paths (D-24). [ASSUMED — `_links.webui` is documented in older Confluence DC API refs; exact field name confirmed by CONTEXT.md D-24]
- **Creating reqwest::Client per request:** Reuse the Phase 1 `Client` struct which holds the connection pool. Do NOT instantiate a new `reqwest::Client` per API call in `api/space.rs`.
- **Logging the Authorization header:** `#[instrument(skip(client))]` or only log URL, never log `Authorization` headers. Pattern established in Phase 1.
- **Using `comfy-table` NOTHING preset for TSV in `ccli space list`:** The existing `tsv.rs` is correct — do not use comfy-table for the non-interactive path (Phase 1 Pitfall 5).
- **Leaving terminal in raw mode on panic:** If not using `ratatui::init()`, must manually install a panic hook. The UI-SPEC's Panic Safety Contract pattern is correct; using `ratatui::init()` is equivalent and simpler.
- **`ListItem` layout shift on selection:** Use `HighlightSpacing::Always` to reserve the 2-char gutter unconditionally, preventing the list from jumping when the selection highlight appears. [CITED: docs.rs/ratatui/widgets/struct.List.html]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Fuzzy string matching | Custom substring or edit-distance filter | `nucleo-matcher` `Pattern::parse` + `match_list` | Unicode-correct, fzf-style ranking, locked by D-16 |
| Cross-platform browser open | Shell exec of `xdg-open` / `open` / `start` per OS | `open` crate `open::that(url)` | Handles all platforms; error handling included; locked by D-22 |
| Terminal raw mode / alt screen lifecycle | Manual `enable_raw_mode` + execute! macros | `ratatui::init()` / `ratatui::restore()` | Panic hook, raw mode, alt screen in one atomic call; idempotent restore |
| List scroll + selection tracking | Manual index management | `ListState` + `render_stateful_widget` | Handles scroll offset automatically when selection moves out of visible area |
| Modal overlay background clearing | Z-order tricks | `Clear` widget before rendering modal | Without `Clear`, the terminal background shows through the modal |

**Key insight:** In Ratatui, state management (selection, scroll offset) is the only real complexity — the widgets handle all rendering math. Keep `App` struct clean and stateless render functions pure.

---

## Common Pitfalls

### Pitfall 1: ratatui 0.29 vs 0.30 API Differences

**What goes wrong:** Tutorial code from ratatui 0.29 and earlier uses `Terminal::new(CrosstermBackend::new(stdout()))` + manual `enable_raw_mode()`. ratatui 0.30 ships `ratatui::init()` which supersedes this.
**Why it happens:** Most online tutorials and the ratatui Book were written for ≤0.29.
**How to avoid:** Use `ratatui::init()` → `ratatui::restore()`. The 0.30 re-export of crossterm avoids version conflicts — use `use ratatui::crossterm::...` if both crates are in scope.
**Warning signs:** Code that calls `enable_raw_mode()` directly or creates `CrosstermBackend` manually where `ratatui::init()` would suffice.

### Pitfall 2: Clap test breakage when making command optional

**What goes wrong:** Phase 1 has a test `missing_subcommand_errors` that asserts `Cli::try_parse_from(["ccli"]).is_err()`. Making `command: Option<Commands>` causes `ccli` with no args to parse successfully, breaking this test.
**Why it happens:** The test was written to encode Phase 1 behavior that D-10 explicitly reverses.
**How to avoid:** Update the test to `assert!(result.is_ok())` and `assert!(cli.command.is_none())`. This test now documents D-10 behavior.
**Warning signs:** Compile succeeds but `cargo test` fails on `cli/mod.rs` tests.

### Pitfall 3: `_links.webui` is a relative path, not an absolute URL

**What goes wrong:** Constructing the browser URL as `_links.webui` directly yields a relative path like `/display/DEV` — not a full URL.
**Why it happens:** The Confluence DC API returns relative paths in `_links.webui` (the `base` field provides the origin).
**How to avoid:** Join `_links.webui` onto `client.base_url()` — `format!("{}{}", client.base_url(), space.links.webui.unwrap_or_default())`.
**Warning signs:** `open::that()` fails with "not a URL" or browser opens `file:///display/DEV`.

### Pitfall 4: Spinner hangs if event loop blocks on `event::read()`

**What goes wrong:** Using blocking `event::read()` instead of `event::poll(timeout) + event::read()` means the spinner never advances when no key is pressed.
**Why it happens:** `event::read()` blocks indefinitely.
**How to avoid:** Always use `event::poll(Duration::from_millis(100))` → check → `event::read()`. The 100ms poll timeout drives spinner advancement on the `else` (timeout) branch.
**Warning signs:** Spinner frame never advances until user presses a key.

### Pitfall 5: Filter query clears ListState selection unexpectedly

**What goes wrong:** When the filter changes `filtered_indices`, the previously selected index may no longer be valid, causing a panic on `items[state.selected()]`.
**Why it happens:** `ListState` holds an absolute index; filtering changes the item count.
**How to avoid:** When `filtered_indices` changes, reset `ListState` to `select(Some(0))` if the list is non-empty, or `select(None)` if empty.
**Warning signs:** Panic "index out of bounds" in the render function when typing a filter.

### Pitfall 6: tokio async + crossterm event loop — running blocking poll in async context

**What goes wrong:** `crossterm::event::poll()` and `event::read()` are synchronous / blocking. Calling them directly in `async fn` without `tokio::task::spawn_blocking` can block the tokio executor thread.
**Why it happens:** The tokio single-threaded executor is blocked while `poll()` waits.
**How to avoid:** For this phase's scale (100ms tick, single user), this is acceptable — `ccli` is a single-user CLI tool with one async task for fetching. The event loop occupies the main thread; API fetches run in a `tokio::spawn` task. Alternatively, use `crossterm`'s `EventStream` (async) if stricter async compliance is needed in the future.
**Warning signs:** API fetch tasks appear to stall or spinner is sluggish.

### Pitfall 7: Confluence space list API endpoint may return `size` field differently than expected

**What goes wrong:** The `size` field in `GET /rest/api/space` response refers to the count of results on THIS page (not total spaces). Using it as total space count for the spinner display will be wrong.
**Why it happens:** Confluence DC pagination follows the HATEOAS pattern where `size` = current page size and total is not directly given.
**How to avoid:** Show "Loading spaces… N fetched" (count of spaces collected so far) rather than "N/total". Only show a denominator if the first API response includes a known total — some DC versions include a `totalSize` field but this is not guaranteed. [ASSUMED — Confluence DC 9.x behavior; the API docs show `size` as current-page count only]
**Warning signs:** The spinner shows "75/25" or similar nonsensical fractions.

---

## Code Examples

### Verified patterns from official sources

#### ratatui::init() / ratatui::restore() (0.30)

```rust
// Source: https://docs.rs/ratatui/latest/ratatui/init/fn.init.html
fn main() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();  // raw mode + alt screen + panic hook
    let result = run_app(&mut terminal);
    ratatui::restore();                  // always restore, even on error
    result
}
```

#### crossterm event::poll with timeout ticker

```rust
// Source: https://docs.rs/crossterm/latest/crossterm/event/fn.poll.html
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

loop {
    terminal.draw(|f| render(f, &app))?;

    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') | KeyCode::Down => app.select_next(),
                    KeyCode::Char('k') | KeyCode::Up => app.select_prev(),
                    KeyCode::Char('/') => app.enter_filter_mode(),
                    _ => {}
                }
            }
        }
    } else {
        app.tick(); // advance spinner, check debounce
    }
}
```

#### Ratatui Layout (vertical + horizontal split)

```rust
// Source: https://docs.rs/ratatui/latest/ratatui/layout/index.html
use ratatui::layout::{Constraint, Layout};

let vertical = Layout::vertical([
    Constraint::Length(3),   // title block
    Constraint::Min(0),      // main body
    Constraint::Length(1),   // status bar
    Constraint::Length(1),   // help bar
]);
let [title_area, body_area, status_area, help_area] = vertical.areas(frame.area());

let horizontal = Layout::horizontal([
    Constraint::Percentage(40),  // list pane
    Constraint::Percentage(60),  // preview pane
]);
let [list_area, preview_area] = horizontal.areas(body_area);
```

#### nucleo-matcher usage

```rust
// Source: https://docs.rs/nucleo-matcher/0.3.1/nucleo_matcher/pattern/struct.Pattern.html
use nucleo_matcher::{Matcher, Config};
use nucleo_matcher::pattern::{Pattern, CaseMatching, Normalization};

fn filter_spaces(spaces: &[Space], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..spaces.len()).collect();
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let haystacks: Vec<String> = spaces.iter()
        .map(|s| format!("{} {}", s.key, s.name))
        .collect();
    let results = pattern.match_list(haystacks.iter().map(|s| s.as_str()), &mut matcher);
    // Recover original indices, preserving score-sorted order
    results.iter().filter_map(|(h, _)| haystacks.iter().position(|x| x == h)).collect()
}
```

#### open crate (browser launch)

```rust
// Source: https://docs.rs/open/latest/open/
use open;

fn open_in_browser(url: &str) -> Result<(), String> {
    open::that(url).map_err(|e| e.to_string())
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual `enable_raw_mode()` + `Terminal::new(CrosstermBackend::new(stdout()))` | `ratatui::init()` | ratatui 0.28/0.29 | Single-call setup; panic hook included |
| Manual panic hook (`std::panic::set_hook`) | Included in `ratatui::init()` | ratatui 0.28 | No longer needs explicit setup |
| `frame.size()` for terminal area | `frame.area()` | ratatui 0.29 | `size()` deprecated; `area()` is current |
| `ListItem::new(text)` with `Text` | `ListItem::new(impl Into<Text>)` | ratatui 0.26+ | Accepts `&str`, `String`, `Span`, `Text` directly |

**Deprecated/outdated:**
- `frame.size()`: Deprecated in favour of `frame.area()` in ratatui 0.29+. Use `frame.area()`.
- `ListItem` requiring explicit `Text::from()` wrapping: now accepts `&str` directly.
- `atty` crate: RUSTSEC-2021-0145; use `is-terminal` (already in Phase 1 Cargo.toml).

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `_links.webui` field is present on space objects in Confluence DC 9.2.19 REST API v1 response | Common Pitfalls / Pitfall 3 | Browser open URL construction breaks; need to fall back to hand-built `/display/{KEY}` path |
| A2 | `GET /rest/api/space` paginated response includes `_links.next` to indicate more pages | Standard Stack / Pattern 5 | Pagination walk loop logic needs to use `size < limit` comparison instead of `_links.next` |
| A3 | `GET /rest/api/space/{key}?expand=description.plain,homepage` returns homepage as an object with `_links.webui` | Code Examples | Preview pane homepage URL resolution fails; may need different expand param |
| A4 | Confluence DC 9.2.19 does not impose a server-side pagination cap below 25 items | Common Pitfalls / Pitfall 7 | May need to detect actual page size and adjust loop termination |
| A5 | nucleo-matcher 0.5.0 is the version in the `nucleo` umbrella crate (nucleo-matcher is the low-level sub-crate) | Standard Stack | Dependency declared as `nucleo` but the usage API is from `nucleo-matcher`; need to check which to declare in Cargo.toml |

> A5 clarification: `nucleo` is the high-level crate (multithreaded, for large datasets). `nucleo-matcher` is the low-level single-threaded crate. For the space list (small dataset, main thread acceptable), `nucleo-matcher` is sufficient and lighter. D-16 says "use the `nucleo` crate" — either works, but `nucleo-matcher` is simpler. The plan should declare `nucleo-matcher` as the Cargo.toml dependency.

---

## Open Questions

1. **`_links.webui` field presence in Confluence DC 9.2.19**
   - What we know: Older Confluence DC API docs (6.6.0) show `_links.self`, `_links.collection`, `_links.base`, `_links.context` on space objects — but NOT `_links.webui` explicitly. The CONTEXT.md D-24 and the jira-cli UX model suggest `webui` exists.
   - What's unclear: Whether `_links.webui` is present in the list endpoint response or only in the detail endpoint (`GET /rest/api/space/{key}`), or whether it requires an `expand` parameter.
   - Recommendation: In `api/space.rs`, define `webui` as `Option<String>`. In the browser open handler, if `webui` is `None` on the list response, fall back to fetching the detail (which is already cached after selection).

2. **`nucleo` vs `nucleo-matcher` Cargo dependency**
   - What we know: D-16 says "use the `nucleo` crate". `cargo search` confirmed `nucleo = "0.5.0"`. `nucleo-matcher = "0.3.1"` is the lower-level sub-crate. The code examples use `nucleo-matcher` types.
   - What's unclear: The `nucleo` umbrella re-exports `nucleo-matcher`; declaring `nucleo` in Cargo.toml gives access to both.
   - Recommendation: Declare `nucleo-matcher = "0.3.1"` in Cargo.toml for the lightest dependency. The plan must specify this explicitly.

3. **Tokio async + blocking crossterm::event::poll**
   - What we know: `crossterm::event::poll` is synchronous and occupies the thread for up to 100ms. In a `#[tokio::main]` context this blocks the async executor.
   - What's unclear: Whether the single-threaded Tokio executor (default for CLI) is adequate, or whether `spawn_blocking` is needed.
   - Recommendation: Use `tokio::task::spawn_blocking` to run the event loop in a dedicated thread, communicating fetch results via `tokio::sync::mpsc` channels. This is the correct pattern for non-trivial Ratatui + Tokio apps. The plan task for `tui/mod.rs` should implement this.

---

## Environment Availability

Step 2.6: SKIPPED — This phase introduces only source code and Cargo dependencies. No external services, databases, or OS tools beyond the Rust toolchain (already verified working in Phase 1).

---

## Validation Architecture

`nyquist_validation` is `false` in `.planning/config.json` — section skipped per configuration.

---

## Security Domain

`security_enforcement` is not set in `.planning/config.json` (key absent = enabled).

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (PAT handling unchanged from Phase 1) | Phase 1 `Client` struct — no new auth code |
| V3 Session Management | no (CLI tool, no session state persisted) | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes (filter query input from user keystrokes) | nucleo-matcher handles arbitrary Unicode input safely — no injection surface |
| V6 Cryptography | no | — |

### Known Threat Patterns for TUI + Confluence DC REST API

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| PAT token in tracing logs | Information Disclosure | `#[instrument(skip(client))]` — established in Phase 1, apply to `SpaceClient` methods |
| URL crafted from API response opening unintended protocol | Tampering | `open::that()` passes URL to OS browser; validate `_links.webui` starts with `/` or known base URL before construction |
| Filter query causing excessive CPU (ReDoS-like) | Denial of Service | Not applicable — nucleo-matcher does not use regexes; bounded by space list size |

---

## Sources

### Primary (HIGH confidence)
- `/ratatui/ratatui` (Context7) — event loop, ListState, Layout, Clear, HighlightSpacing
- `/websites/rs_ratatui` (Context7) — `ratatui::init()`, `ratatui::restore()`, panic hook
- `/websites/rs_nucleo-matcher_0_3_1` (Context7) — `Pattern::parse`, `match_list`, `Matcher`
- `https://docs.rs/crossterm/latest/crossterm/event/fn.poll.html` — `event::poll` signature and usage
- `/websites/atlassian_atlassian-confluence_rest_6_6_0` (Context7) — `GET /rest/api/space` parameters and response shape
- `cargo search` (verified 2026-04-27) — ratatui 0.30.0, crossterm 0.29.0, nucleo 0.5.0, open 5.3.4

### Secondary (MEDIUM confidence)
- `https://github.com/ratatui/ratatui/blob/main/BREAKING-CHANGES.md` — 0.29→0.30 breaking changes, `ratatui-core` migration
- `https://github.com/ratatui/ratatui/blob/main/ratatui-crossterm/README.md` — crossterm re-export pattern
- Phase 1 codebase (`src/api/client.rs`, `src/cli/mod.rs`, `src/output/mod.rs`) — integration points verified by direct read

### Tertiary (LOW confidence — flagged as ASSUMED)
- `_links.webui` field presence in Confluence DC 9.2.19 space list response (A1)
- `_links.next` pagination sentinel presence (A2)
- Homepage field in expand response shape (A3)

---

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — all crate versions verified via `cargo search` 2026-04-27
- Architecture: HIGH — patterns verified against Context7/docs.rs; module layout matches UI-SPEC
- Confluence API shape: MEDIUM — paginated response shape confirmed; `_links.webui` assumed based on CONTEXT.md D-24
- Pitfalls: HIGH — ratatui version differences verified against BREAKING-CHANGES.md; others from direct code analysis

**Research date:** 2026-04-27
**Valid until:** 2026-05-27 (stable crates — 30 day window)
