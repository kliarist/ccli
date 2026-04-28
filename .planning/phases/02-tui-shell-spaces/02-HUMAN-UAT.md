---
status: partial
phase: 02-tui-shell-spaces
source: [02-VERIFICATION.md]
started: 2026-04-28T12:00:00Z
updated: 2026-04-28T12:00:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Full TUI session — spinner, layout, navigation, preview pane
expected: Run `ccli` — spinner shows "Loading spaces…", then space list with cyan selection highlight and 40/60 split preview pane; j/k/arrows navigate, preview updates after ~150ms
result: [pending]

### 2. Fuzzy filter overlay — real-time narrowing, M/N status bar
expected: Press `/` — filter overlay opens; typing narrows list in real time; status bar shows M/N count; Esc restores full list
result: [pending]

### 3. Browser open including SSH/headless fallback
expected: Press `o` on a selected space — system browser opens the space URL; on headless/SSH: TUI suspends, URL printed to stderr, "Press any key to continue." shown, TUI restores on keypress
result: [pending]

### 4. Help modal — visual dimensions and content
expected: Press `?` — 50×14 centered modal with rounded border (cyan) shows full keybinding reference; Esc closes it
result: [pending]

### 5. Quit/terminal restore — no raw-mode artifacts
expected: Press `q` or `Esc` from Browse state — TUI exits cleanly, terminal fully restored (no garbled text, no raw-mode leakage)
result: [pending]

### 6. ccli space list with real data — table output and flags
expected: `ccli space list` shows Key/Name/Type table; `--plain` gives TSV; `--no-headers` suppresses header; `ccli space list | cat` gives TSV (pipe detection)
result: [pending]

## Summary

total: 6
passed: 0
issues: 0
pending: 6
skipped: 0
blocked: 0

## Gaps
