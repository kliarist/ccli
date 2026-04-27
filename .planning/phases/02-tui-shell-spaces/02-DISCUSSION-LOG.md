# Phase 2: TUI Shell & Spaces - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-27
**Phase:** 02-tui-shell-spaces
**Areas discussed:** TUI entry & lifecycle, Space fetch & filter behavior, Preview pane content for Phase 2, Browser-open + SSH fallback

---

## TUI entry & lifecycle

### What does `ccli space list` do when run in a terminal (TTY)?

| Option | Description | Selected |
|--------|-------------|----------|
| Always table | `ccli space list` always prints a non-interactive table; TUI launched via separate command. Cleanest separation, works for piping/scripting, matches OUT-* contract from Phase 1. | ✓ |
| TUI when TTY, table when piped | Strict jira-cli pattern: TTY → TUI, piped → table; `--plain` forces table. One command, two modes. | |
| TUI when TTY + explicit --tui flag | Default to table; `--tui` flag opens TUI. Predictable but doesn't match jira-cli defaults. | |

**User's choice:** Always table.
**Notes:** Locks D-11 — TUI lives behind a different entry point so the `ccli * list` family stays scriptable.

### How does the user launch the interactive TUI?

| Option | Description | Selected |
|--------|-------------|----------|
| Bare `ccli` | Running `ccli` with no subcommand opens the TUI. Mirrors lazygit / k9s. Phase 1 currently errors here; we flip that. | ✓ |
| `ccli tui` subcommand | Explicit `ccli tui`. More discoverable in `--help`; harder to fat-finger; daily friction. | |
| `ccli browse` subcommand | Verb-named entry. Reads naturally; doesn't match jira-cli's bare-binary pattern. | |

**User's choice:** Bare `ccli`.
**Notes:** Locks D-10. Subcommand becomes `Option<Commands>`; `None` dispatches to TUI runner.

### What are Esc and q semantics inside the TUI?

| Option | Description | Selected |
|--------|-------------|----------|
| q quits, Esc closes overlays | `q` quits from any view; `Esc` closes filter / modal first, only quits at top level. Matches jira-cli/vim. | ✓ |
| q quits, Esc always quits | Both immediately quit. Simplest; risks losing in-progress filter input. | |
| Esc only closes overlays, q only quits | Strict separation; can be brittle when `q` is captured by an input. | |

**User's choice:** q quits, Esc closes overlays.
**Notes:** Locks D-12.

### How should keybindings be exposed to the user (TUI-06)?

| Option | Description | Selected |
|--------|-------------|----------|
| Static bottom bar + ? modal | Always-visible strip + `?` modal for full reference. Best of both. | ✓ |
| Static bottom bar only | Just the strip. Struggles when binding count grows (Phase 3+). | |
| ? modal only | Maximizes screen space; risky for new-user discoverability. | |

**User's choice:** Static bottom bar + `?` modal.
**Notes:** Locks D-13. Bottom strip text is per-view (e.g., `/ filter  o open  ? help  q quit`).

---

## Space fetch & filter behavior

### How should the TUI fetch space data on launch?

| Option | Description | Selected |
|--------|-------------|----------|
| Pre-fetch all, paginate internally | Walk all pages of `/rest/api/space` at launch, hold full list in memory. Simplest filter UX. Linear startup time on big instances. | ✓ |
| Lazy-load on scroll | First page only, fetch more on scroll; filter has to hit server for unloaded spaces. | |
| Pre-fetch first page, background-load rest | Fast first paint + background hydration; more state to manage. | |

**User's choice:** Pre-fetch all, paginate internally.
**Notes:** Locks D-14. Spinner / status-bar progress during the walk.

### How does the `/` fuzzy filter behave?

| Option | Description | Selected |
|--------|-------------|----------|
| Real-time on every keystroke | Re-filter as user types, match against key + name. Snappy, jira-cli-style. | ✓ |
| Submit on Enter | Type query, press Enter. Predictable but feels slow. | |
| Real-time, key-only or name-only toggle | Tab cycles match-target. More precision; more keys to remember. | |

**User's choice:** Real-time on every keystroke (key + name).
**Notes:** Locks D-15.

### Which fuzzy-matcher crate?

| Option | Description | Selected |
|--------|-------------|----------|
| nucleo | fzf-style, used by Helix, Unicode-correct, actively maintained. | ✓ |
| fuzzy-matcher | Mature; SkimMatcherV2; less actively developed. | |
| Plain substring | `to_lowercase().contains()`. No fuzzy magic; too primitive. | |

**User's choice:** nucleo.
**Notes:** Locks D-16.

### How should `ccli space list` handle pagination?

| Option | Description | Selected |
|--------|-------------|----------|
| Fetch all, print all | Same walk as TUI launch. Matches SPC-04 "all accessible spaces". | ✓ |
| First page only + --limit/--page flags | Default to 25; flags for more. Fast but doesn't match SPC-04 by default. | |
| Fetch all + --limit cap | Default to all; `--limit` truncates server-side. Compromise. | |

**User's choice:** Fetch all, print all.
**Notes:** Locks D-17. No `--limit`/`--page` flags this phase.

---

## Preview pane content for Phase 2

### What goes in the preview pane when a space is highlighted?

| Option | Description | Selected |
|--------|-------------|----------|
| Space metadata only | Key, name, type, description, homepage URL via `/rest/api/space/{key}?expand=description`. No extra-phase scope creep. | ✓ |
| Metadata + recent pages teaser | Adds a `/rest/api/space/{key}/content/page?limit=5` request per selection. Stretches into Phase 3. | |
| Empty placeholder | "Pages coming in Phase 3" or just key/name. Doesn't earn the preview pane. | |

**User's choice:** Space metadata only.
**Notes:** Locks D-18.

### When is the preview pane data fetched?

| Option | Description | Selected |
|--------|-------------|----------|
| On selection, with debounce + cache | ~150ms debounce while arrowing; in-memory session cache. Fast, minimal wasted requests. | ✓ |
| Eager: hydrate all spaces on launch | Always-instant preview; large startup cost. | |
| On selection, no debounce | Simple but request-storm risk on fast j/k. | |

**User's choice:** On selection, with debounce + cache.
**Notes:** Locks D-19.

### What's the layout split between list and preview?

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed 40/60 list/preview | Roomy preview without truncating space names. Simple `Constraint::Percentage`. | ✓ |
| Fixed 50/50 | Even; descriptions wrap more aggressively. | |
| Dynamic based on terminal width | Stack on narrow terminals; more layout code. | |

**User's choice:** Fixed 40/60.
**Notes:** Locks D-20.

### How should we handle a space with no description / missing fields?

| Option | Description | Selected |
|--------|-------------|----------|
| Show fields with 'No description' / '—' fallbacks | Stable layout across selections; predictable. | ✓ |
| Hide missing fields | Less noise; inconsistent layout. | |
| Show raw JSON | Debug-only; not user-friendly. | |

**User's choice:** Fallback strings, stable layout.
**Notes:** Locks D-21.

---

## Browser-open + SSH fallback

### Which crate handles `o` — opening a URL in the system browser?

| Option | Description | Selected |
|--------|-------------|----------|
| open | Sebastian Thiel's crate; cross-platform; used by `cargo doc --open`. | ✓ |
| webbrowser | Similar functionality; more deps; rougher cross-platform edge cases. | |
| Roll our own per-OS shell-out | Zero deps; we own the SSH-detection / fallback logic. More code to maintain. | |

**User's choice:** open.
**Notes:** Locks D-22.

### How do we behave on headless/SSH (no DISPLAY, browser launch fails)?

| Option | Description | Selected |
|--------|-------------|----------|
| Try open, on failure print URL with hint | Reactive: attempt `open::that()`, on Err suspend TUI, print URL + 'press any key', restore TUI. | ✓ |
| Detect SSH/headless first, branch | Check `$SSH_CONNECTION` / `$DISPLAY` before trying. Brittle in practice. | |
| Hard error on failure | Two-line stderr error; loses press-any-key affordance. | |

**User's choice:** Try open, on failure print URL with hint.
**Notes:** Locks D-23. Don't sniff env vars — react to actual `open::that()` failure.

### Which URL do we open for a space when the user presses `o`?

| Option | Description | Selected |
|--------|-------------|----------|
| Use the API's `_links.webui` or homepage link | Authoritative; works on instances with custom routing/context paths. | ✓ |
| Construct `<base>/display/<KEY>` | Hand-built; fails on customized Confluence routing. | |
| Construct, fall back to API link if 404 detected | Adds complexity for a marginal win. | |

**User's choice:** Use the API's `_links.webui` / homepage link.
**Notes:** Locks D-24.

### Should we also offer `c` (copy URL to clipboard) as a Phase 2 keybinding?

| Option | Description | Selected |
|--------|-------------|----------|
| Defer to a later phase | Skip clipboard in Phase 2; the print-URL fallback covers SSH copy-paste. | ✓ |
| Add `c` keybind in Phase 2 with arboard | Wire `arboard::Clipboard::set_text(url)`. Adds dep + Linux clipboard-daemon caveat. | |
| Add `y` (yank) instead of `c` | vim-style alternative. Same trade-offs. | |

**User's choice:** Defer to a later phase.
**Notes:** Locks D-25. Re-evaluate when Phase 3 page IDs make yanking compelling.

---

## Claude's Discretion

- TUI module layout (`src/tui/{app.rs, screens/spaces.rs, widgets/*}`), event-loop pattern (`tokio::select!` over crossterm events + fetch channels), tick rate.
- Panic-safe terminal restoration (panic hook calling `disable_raw_mode()` + `LeaveAlternateScreen`).
- Default sort order in space list — `key` ascending.
- Including all space types (global / personal / archived) by default; no `--type` filter.
- Spinner / loading text during page-walk (e.g., `Loading spaces… 75/200`).
- TUI default columns shown in the list (key, name); preview pane carries the rest.
- Color theme — monochrome with single accent color.

## Deferred Ideas

- Clipboard / copy-URL keybind (`c` / `y`) — needs `arboard` + Linux clipboard daemon; defer until Phase 3 makes yanking compelling.
- Page list teaser inside the space preview pane — naturally lands in Phase 3.
- Space-type filtering flag (`--type=global` / `--type=personal`) — v2.
- `--limit` / `--page` flags on `ccli space list` — only if reported slow.
- Lazy-load pagination in the TUI — same trigger.
- TUI color themes — out of scope for v1.
- Dynamic responsive layout for narrow terminals — defer unless feedback demands it.
