# ccli 🦀

Confluence CLI and terminal UI written in Rust, for when the browser tab count has become a moral issue.

`ccli` works with both **Confluence Data Center** and **Confluence Cloud**. It supports command-oriented usage and an interactive TUI, so you can browse, search, and edit Confluence content without leaving the terminal.

## ⚡ Installation

Grab a pre-built binary from the [latest release](https://github.com/kliarist/ccli/releases/latest), or build from source:

```bash
cargo install --path .
```

## ✨ Features

- 🖥️ Interactive TUI launched by running bare `ccli`
- 🔍 Fuzzy filtering in all list views
- 🖱️ Mouse scroll support — scroll the list or preview pane independently
- 🎨 Styled page preview: formatted headings, bullet lists, code blocks
- 📁 Space listing and browsing
- 📄 Page listing, viewing, searching, creating, and editing
- 📝 Blog post listing, viewing, creating, and editing
- 💬 Plain-text comment authoring in `$EDITOR`
- 📎 Attachment listing, download, and upload
- 📊 Pretty table output for interactive terminals; TSV for pipes

## 🔧 Configuration

First-time setup:

```bash
ccli init
```

Prompts for a Confluence base URL and a Personal Access Token. The connection is validated before saving, so bad credentials do not get a permanent home.

Config file: `~/.config/ccli/config.toml`

```toml
url = "https://confluence.example.com"
token = "AT-xxxxxxxxxxxx"
```

Environment variable overrides (useful for CI and containers):

| Variable | Purpose |
|----------|---------|
| `CCLI_URL` | Confluence base URL |
| `CCLI_TOKEN` | Personal Access Token |
| `RUST_LOG` | Log level (e.g. `debug`) |

If both `CCLI_URL` and `CCLI_TOKEN` are set, `ccli` runs without a config file.

## 🖥️ TUI

Launch with:

```bash
ccli
```

### ⌨️ Keyboard shortcuts

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate list |
| `j` / `k` | Navigate list (vim-style, Browse mode only) |
| `g` / `G` | Jump to top / bottom |
| `/` | Open fuzzy filter |
| `Esc` | Close filter / go back |
| `Enter` | Open selected space or page |
| `o` | Open in browser |
| `e` | Edit page in `$EDITOR` |
| `c` | View comments |
| `PgDn` / `PgUp` | Scroll page preview |
| `?` | Show key bindings |
| `q` | Quit |

### 🖱️ Mouse

- Scroll on the **left pane** to move the list selection up and down
- Scroll on the **right pane** to scroll the page preview

### 🔍 Filter

Press `/` to open the fuzzy filter. All printable characters type into the filter query. Use `↑`/`↓` arrow keys to navigate the filtered list, and `Enter` to open the selected item. `Esc` closes the filter and restores the full list.

## 🛠️ CLI Commands

Top-level help:

```bash
ccli --help
```

### Spaces

```bash
ccli space list
```

### Pages

```bash
# List pages in a space
ccli page list DEV

# View page content as plain text
ccli page view 123456

# Search with CQL
ccli page search "type=page AND space=DEV AND text~deploy" --limit 25

# Create a page (opens $EDITOR)
ccli page create --space DEV --title "Release Notes"

# Edit a page (opens $EDITOR)
ccli page edit 123456
```

### Blog Posts

```bash
ccli blog list DEV
ccli blog create --space DEV --title "Weekly Update"
ccli blog edit 123456
```

### Comments

```bash
# Add a comment (opens $EDITOR)
ccli comment add 123456
```

### Attachments

```bash
ccli attachment list 123456
ccli attachment get 123456 design.pdf
ccli attachment add 123456 ./design.pdf
```

## 📤 Output Modes

Global flags apply to all tabular commands:

```bash
ccli --plain --no-headers --columns key,name space list
```

| Flag | Effect |
|------|--------|
| _(none)_ | Formatted table on TTY, TSV when piped |
| `--plain` | Force TSV |
| `--no-headers` | Suppress header row |
| `--columns a,b` | Show only these columns, in this order |

## 📝 Editor Integration

The following commands open `$EDITOR` (falls back to `$VISUAL`, then `vi`):

- `ccli page create` / `ccli page edit`
- `ccli blog create` / `ccli blog edit`
- `ccli comment add`

Pages and blog posts edit Confluence storage XML directly. Comments are written as plain text and converted to storage format automatically.

## 🏗️ Build & Test

```bash
cargo build --release
cargo test
```

## 🧰 Tech Stack

| Concern | Crate |
|---------|-------|
| Async runtime | `tokio` |
| HTTP client | `reqwest` |
| CLI parsing | `clap` |
| TUI | `ratatui` + `crossterm` |
| Fuzzy matching | `nucleo-matcher` |
| XML parsing | `quick-xml` |

## 📜 License

MIT. See [LICENSE](LICENSE).

<a href='https://ko-fi.com/V7V81Z4ZAU' target='_blank'><img height='36' style='border:0px;height:36px;' src='https://storage.ko-fi.com/cdn/kofi6.png?v=6' border='0' alt='Buy Me a Coffee at ko-fi.com' /></a>
