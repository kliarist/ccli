# ccli

Confluence Data Center CLI and terminal UI written in Rust, for when the browser tab count has become a moral issue.

`ccli` lets you browse spaces, pages, comments, and attachments from the terminal. It supports both command-oriented usage and an interactive TUI, so you can stay in the shell and pretend that is a lifestyle choice.

## Features

- Interactive setup with connection validation via `ccli init`
- Terminal UI launched by running bare `ccli`
- Space listing and browsing
- Page listing, viewing, searching, creating, and editing
- Blog post listing, viewing, creating, and editing
- Plain-text comment authoring in `$EDITOR`
- Attachment listing, download, and upload
- Pretty table output for interactive terminals and TSV output for pipes, because the tool should know when it is being watched

## Build

```bash
cargo build
cargo build --release
```

## Test

```bash
cargo test
cargo test -- --nocapture
```

## Configuration

First-time setup:

```bash
cargo run -- init
```

or, after building:

```bash
ccli init
```

`ccli init` prompts for:

- Confluence base URL
- Personal Access Token

The CLI validates the connection before saving the config, so bad credentials do not get a permanent home.

Config file location:

```text
~/.config/ccli/config.toml
```

Example:

```toml
url = "https://confluence.example.com"
token = "AT-xxxxxxxxxxxx"
```

Environment variable overrides are supported:

- `CCLI_URL`
- `CCLI_TOKEN`
- `RUST_LOG`

If both `CCLI_URL` and `CCLI_TOKEN` are set, `ccli` can run without a config file. Handy for CI, containers, or committing fewer secrets to muscle memory.

## Usage

Run the TUI:

```bash
ccli
```

Top-level help:

```bash
ccli --help
```

### Common Commands

Initialize config:

```bash
ccli init
```

List spaces:

```bash
ccli space list
```

List pages in a space:

```bash
ccli page list DEV
```

View a page as plain text:

```bash
ccli page view 123456
```

Search pages with CQL:

```bash
ccli page search "type=page AND space=DEV AND text~deploy" --limit 25
```

Create a page:

```bash
ccli page create --space DEV --title "Release Notes"
```

Edit a page in `$EDITOR`:

```bash
ccli page edit 123456
```

Create a blog post:

```bash
ccli blog create --space DEV --title "Weekly Update"
```

Add a comment:

```bash
ccli comment add 123456
```

List attachments:

```bash
ccli attachment list 123456
```

Download an attachment:

```bash
ccli attachment get 123456 design.pdf
```

Upload an attachment:

```bash
ccli attachment add 123456 ./design.pdf
```

## Output Modes

Global output flags apply to supported tabular commands:

```bash
ccli --plain --no-headers --columns key,name space list
```

- Interactive terminal output defaults to a formatted table
- Piped or redirected output defaults to TSV
- `--plain` forces TSV
- `--no-headers` removes the header row
- `--columns` filters and reorders columns

Example:

```bash
ccli page list DEV --plain --columns title,id
```

## Editor Integration

Some commands open your editor:

- `ccli page create`
- `ccli page edit`
- `ccli blog create`
- `ccli blog edit`
- `ccli comment add`

`ccli` uses `$EDITOR`, then `$VISUAL`, and falls back to `vi`.

Page and blog editing use Confluence storage XML. Comment editing uses plain text and converts paragraphs to Confluence storage format automatically.

## Development Notes

- Runtime: async Rust with `tokio`
- HTTP client: `reqwest`
- CLI parsing: `clap`
- TUI: `ratatui` + `crossterm`

## License

MIT. See [LICENSE](/Users/thekliar/Dev/projects/5soft/confluence-cli/LICENSE).

<a href='https://ko-fi.com/V7V81Z4ZAU' target='_blank'><img height='36' style='border:0px;height:36px;' src='https://storage.ko-fi.com/cdn/kofi6.png?v=6' border='0' alt='Buy Me a Coffee at ko-fi.com' /></a>
