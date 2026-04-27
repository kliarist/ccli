# Phase 2: TUI Shell & Spaces - Context

**Gathered:** 2026-04-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the Ratatui TUI shell — async event loop, layout primitives, key-binding infrastructure, help bar, and panic-safe terminal restoration — and deliver the Spaces feature end-to-end through both surfaces:

- **Non-interactive:** `ccli space list` prints all accessible spaces as a table (or TSV when piped) using the existing Phase 1 `OutputFormatter`.
- **Interactive:** Bare `ccli` (no subcommand) launches the TUI on a Spaces browse view with list + preview pane, real-time `/` fuzzy filter, and `o` to open the highlighted space in the system browser.

Requirements covered: TUI-01, TUI-02, TUI-03, TUI-04, TUI-05, TUI-06, SPC-01, SPC-02, SPC-03, SPC-04.

Out of phase: page browsing/viewing/edit (Phase 3), blog posts (Phase 3), comments and attachments (Phase 4), CQL search (Phase 4), clipboard support, page list inside the space preview pane.

</domain>

<decisions>
## Implementation Decisions

### TUI Entry & Lifecycle

- **D-10:** **Bare `ccli` (no subcommand) launches the TUI** on the Spaces browse view. Phase 1 currently errors when no subcommand is given; that behavior flips here. Subcommands (`ccli space list`, `ccli init`, future `ccli page ...`) keep their existing non-interactive contract.
- **D-11:** **`ccli space list` always prints a table to stdout** — never opens the TUI. Stays consistent with the OUT-* contract from Phase 1 (TTY → table, piped → TSV, `--plain` forces TSV). Users who want to browse interactively run `ccli`.
- **D-12:** **`q` always quits the TUI** from any view. **`Esc` closes the active overlay** (filter input, `?` modal); only quits when at the top-level view with no overlay open. Matches jira-cli/vim convention; protects in-progress filter input.
- **D-13:** **Help UX = static bottom bar + `?` modal**. Always-visible bottom strip shows the most-used bindings for the current view (e.g., `/ filter  o open  ? help  q quit`). `?` opens a full modal listing every binding for the current view. Satisfies TUI-06 with both fast scanning and a complete reference.

### Space Fetch & Filter

- **D-14:** **Pre-fetch all spaces on launch.** The TUI walks every page of `/rest/api/space` (paginated, default 25/page) until exhausted, then holds the full list in memory. A spinner/progress indicator shows during the initial fetch. Filter operates entirely on the local list.
- **D-15:** **Real-time fuzzy filter** activates on `/`, re-filters the list on every keystroke, and matches against **both space key and name**. No submit-on-Enter. Pressing `Esc` closes the filter overlay (per D-12) and restores the full list.
- **D-16:** **Use the `nucleo` crate** for fuzzy matching. fzf-style ranking, Unicode-correct, used by the Helix editor; actively maintained.
- **D-17:** **`ccli space list` fetches every page and prints all rows.** Same pagination walk as D-14 — SPC-04 says "all accessible spaces" and the consistent behavior between TUI and CLI keeps the mental model simple. No `--limit` / `--page` flags in this phase.

### Preview Pane

- **D-18:** **Preview pane shows space metadata only:** key, name, type (global/personal/archived), description, homepage URL. No page list — that's Phase 3. Fields come from `/rest/api/space/{key}?expand=description.plain,homepage`.
- **D-19:** **Fetch on selection with debounce + session cache.** When the highlight settles on a space (~150ms debounce while arrowing), fetch its detail. Cache results in-memory for the session so re-visiting is instant and a fast j/k scroll doesn't trigger a request storm.
- **D-20:** **Layout split: fixed 40% list / 60% preview**, implemented with Ratatui `Constraint::Percentage`. Roomy enough for descriptions without truncating space names.
- **D-21:** **Empty/missing fields show fallbacks**: dim `—` for missing values, `No description` when description is absent. Layout structure stays stable across selections.

### Browser Open (`o`)

- **D-22:** **Use the `open` crate** (Sebastian Thiel's, used by `cargo doc --open`). Cross-platform (`xdg-open` / `open` / `start`), minimal API surface, fewer deps than `webbrowser`.
- **D-23:** **Headless/SSH fallback is reactive, not predictive.** Always attempt `open::that()` first. On error: temporarily suspend the TUI (restore terminal), print `Browser unavailable — open this URL manually:\n  <url>` plus `Press any key to continue.` to stderr, wait for keypress, restore the TUI. We do NOT pre-sniff `$SSH_CONNECTION` / `$DISPLAY` — actual failure is more reliable than environment guessing.
- **D-24:** **Resolve the URL from the API's `_links.webui`** (or `homepage._links.webui`) joined onto the configured base URL. Authoritative across instances with custom routing or context paths; never hand-build `<base>/display/<KEY>`.
- **D-25:** **No clipboard / copy-URL keybind in Phase 2.** `c` and `y` stay unbound here. The print-URL fallback in D-23 already covers the SSH copy-paste flow. Re-evaluate when Phase 3 lands and clipboard becomes broadly useful (yanking page IDs, etc.).

### Claude's Discretion

- **TUI architecture:** module layout (`src/tui/`, `src/tui/app.rs`, `src/tui/screens/spaces.rs`, etc.), event-loop pattern (crossterm event reader + tokio `select!` over a fetch channel), tick rate. Standard Ratatui idioms apply.
- **Panic-safe terminal restoration:** install a panic hook that calls `disable_raw_mode()` and `LeaveAlternateScreen` before the default panic prints; identical pattern across Ratatui apps. Required for not-leaving-the-user-in-raw-mode safety; planner should encode it explicitly even though it's discretion-level.
- **Default sort order in space list:** sort by `key` ascending. Matches jira-cli and Confluence's own UI defaults.
- **Which space types to include:** include all types the API returns by default (global, personal, archived). No filtering flag in Phase 2 — that's a v2 idea (`--type=global`).
- **Spinner / loading state:** simple text indicator in the status bar during the initial page-walk (e.g., `Loading spaces… 75/200`). No fancy progress bar.
- **TUI default columns** for the space list (key, name) — the preview pane carries everything else.
- **TUI color theme:** monochrome with a single accent color for the selected row and the filter cursor. No theming infrastructure in v1.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project & Phase Inputs
- `.planning/REQUIREMENTS.md` — TUI-01..06 and SPC-01..04 (Phase 2 requirements with acceptance criteria)
- `.planning/PROJECT.md` — Key decisions (Rust, jira-cli UX, raw-XML editing, PAT-only auth), constraints, out-of-scope items
- `.planning/ROADMAP.md` §"Phase 2: TUI Shell & Spaces" — goal, success criteria, requirement mapping
- `.planning/phases/01-foundation/01-CONTEXT.md` — Phase 1 locked decisions (D-01..D-09); D-08 (tokio + reqwest) is the load-bearing one for this phase

### Reference Implementation
- `https://github.com/ankitpokhrel/jira-cli` — UX model. Reuse for: bottom-bar key hint pattern, `/` filter overlay, `o` to open in browser, `q`/`Esc` semantics, list+preview pane layout. NOT a Rust codebase — patterns only, no code to port.

### Target API (Confluence DC 9.2.19)
- Confluence DC REST API v1: `https://docs.atlassian.com/ConfluenceServer/rest/latest/`
  - `GET /rest/api/space` — paginated list of accessible spaces (used for both TUI launch and `ccli space list`)
  - `GET /rest/api/space/{key}?expand=description.plain,homepage` — single-space detail for the preview pane
- Confluence DC REST API v2: available alongside v1 in DC 9.2.19; v1 is used here for parity with jira-cli's API style and broader documentation coverage

### Phase 1 Code (already implemented — reuse, don't re-build)
- `src/api/client.rs` — `Client::new`, `Client::inner()`, `Client::base_url()`, `CCLI_INSECURE=1` escape hatch
- `src/api/error.rs` — `AppError` taxonomy (Auth / Network / Config / Api). Phase 2 must classify `/rest/api/space` errors into these variants
- `src/output/mod.rs` — `OutputFormatter`, `OutputConfig`, `parse_columns()`. `ccli space list` must use `OutputFormatter::print()` so `--plain`, `--no-headers`, `--columns` all work
- `src/cli/mod.rs` — global flags + `Commands` enum. Add `Space(SpaceArgs)` here; the existing comment `// Phase 2 will add: Space(SpaceArgs)` confirms the intended location
- `src/main.rs` — `hint_for()` and the anyhow chain-walk pattern. New AppError mappings (none expected — variants stay the same) plug in here for free

### Key Crates (already in `Cargo.toml`)
- `tokio = "1"` (full features) — async runtime
- `reqwest = "0.13"` — HTTP client
- `clap = "4.6"` — CLI parser (add a `Space` subcommand)
- `comfy-table = "7"` — non-interactive table
- `tracing` / `tracing-subscriber` — RUST_LOG-driven debug

### Key Crates to Add in Phase 2
- `ratatui` (current ≥ 0.29) — TUI framework (D-10..D-13, D-18..D-21)
- `crossterm` (≥ 0.28) — terminal backend for ratatui (also handles raw-mode + alt-screen + key events)
- `nucleo` (current ≥ 0.5) — fuzzy matcher (D-16)
- `open` (≥ 5) — system browser launcher (D-22)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Client` (`src/api/client.rs`) — preconfigured reqwest client with PAT auth header, 30s timeout, optional `danger_accept_invalid_certs(true)`. Phase 2 builds `space.rs` API module on top via `Client::inner()` + `Client::base_url()`.
- `AppError` (`src/api/error.rs`) — taxonomy is sufficient as-is. 401/403 → `Auth`, network/timeout → `Network`, 4xx/5xx other → `Api`. No new variants needed.
- `OutputFormatter` (`src/output/mod.rs`) — TTY-aware dispatcher. `ccli space list` builds `headers + rows`, calls `print()`, and gets `--plain` / `--no-headers` / `--columns` for free.
- `dialoguer` pattern (`src/cli/init.rs`) — interactive prompt style. Probably not needed in Phase 2 (TUI replaces dialoguer for the browse flow), but worth knowing about for any one-shot prompts (e.g., reauth on 401).
- `tracing` + `RUST_LOG` — debug verbosity is already wired; new modules just `use tracing::{debug, instrument};` and inherit.

### Established Patterns
- Async-first via `#[tokio::main]` — every command handler is `async`. The TUI event loop will live inside `async fn` and `tokio::select!` over crossterm events + fetch channels.
- `#[instrument(skip(token))]` and never logging the PAT — applied in Phase 1's `client.rs`. Same discipline applies to any new API module.
- Plan-letter decision IDs (D-01, D-02, …) — Phase 1 used `D-NN` markers in code comments to tie back to CONTEXT.md. Phase 2 continues from D-10.
- Module layout: feature-flat (`api/client.rs`, `cli/init.rs`, `output/table.rs`). Phase 2 follows: `api/space.rs`, `cli/space.rs`, `tui/{app.rs, screens/spaces.rs, widgets/*}`.
- Test pattern: behavioral unit tests inline (`#[cfg(test)] mod tests`) using `httpmock` for HTTP and pure-function helpers for logic. The output formatter's `print_to<W: Write>` testability pattern is the model — TUI screens should expose a similarly testable seam (state-only, no terminal).

### Integration Points
- `src/cli/mod.rs` `Commands` enum — `Space(SpaceArgs)` added next to `Init`. Existing comment marks the spot.
- `src/main.rs` — when no subcommand is given today, clap errors with "missing subcommand"; D-10 changes that to launch the TUI. Need to make the subcommand `Option<Commands>` and dispatch `None → tui::run()`.
- `~/.config/ccli/config.toml` — already loaded by Phase 1 helpers (`config::load`); TUI reuses the same loader.
- `CCLI_URL` / `CCLI_TOKEN` env overrides — handled in Phase 1 config layer; transparent to Phase 2.
- `OUT-01..04` flags — `ccli space list` inherits via the global flags on `Cli` struct; nothing new to wire.

</code_context>

<specifics>
## Specific Ideas

- **Bare `ccli` opens the TUI** — short, memorable, mirrors `lazygit` / `k9s`. Daily users want the smallest possible mode-switch.
- **jira-cli is the explicit UX reference** for help bar style, `/` filter, `o` browser open, `q`/`Esc` semantics, and the list+preview layout.
- **Space URL must come from `_links.webui`**, not a hand-built path. This avoids breakage on instances with custom context paths.
- **Reactive SSH/headless detection** — try first, fall back on actual error. Don't pre-sniff env vars (too brittle on remote-desktop / mosh / VS Code Remote / headless-with-Browser setups).
- **Fixed 40/60 split** for list/preview — explicit number so the planner doesn't waste cycles on responsive layout this phase.
- **TUI fetch is single-shot, blocking with a spinner** — no lazy-load complexity in v1. We can revisit if a real Confluence DC user reports a 5k+ space instance feeling slow.

</specifics>

<deferred>
## Deferred Ideas

- **Clipboard / copy-URL (`c` or `y`)** — needs `arboard` and a Linux clipboard daemon; not worth it before Phase 3 page IDs make yanking compelling.
- **Page list teaser inside the space preview pane** — naturally lands when Phase 3 adds page browsing. Don't pre-build it here.
- **Space-type filtering flag** (`--type=global` / `--type=personal`) — v2 idea once users tell us they want it.
- **`--limit` / `--page` flags on `ccli space list`** — only matters if real instances feel slow with full pagination. Defer until reported.
- **Lazy-load space pagination in the TUI** — same trigger; pre-fetch is fine for the typical Confluence DC instance size.
- **TUI color themes** — out of scope for v1. Single accent color is enough.
- **Dynamic responsive layout for narrow terminals** — fixed 40/60 wins for Phase 2; revisit if user feedback says otherwise.

</deferred>

---

*Phase: 02-tui-shell-spaces*
*Context gathered: 2026-04-27*
