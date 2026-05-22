# ccli Architecture

`ccli` is a Confluence Data Center CLI written in Rust. It operates in two modes:
- **CLI mode** — `ccli <subcommand>` runs a single command and exits
- **TUI mode** — bare `ccli` launches a full terminal UI

---

## Module Map

```mermaid
graph TD
    main["main.rs\n#Entry point\n#Error dispatch\n#tokio::main"]

    cli["cli/\n#clap Parser + Subcommands\n#sanitize_id\n#OutputConfig"]
    config["config/\n#TOML load/save\n#Env var overlay\n#Atomic write (0o600)"]
    api["api/\n#reqwest Client\n#REST calls per resource"]
    output["output/\n#Table / TSV / XML\n#is-terminal detection"]
    tui["tui/\n#ratatui event loop\n#Screen stack\n#Tokio channels"]

    api_client["api/client.rs\n#Auth (PAT / Basic)\n#TLS config\n#test_connection"]
    api_space["api/space.rs"]
    api_page["api/page.rs"]
    api_comment["api/comment.rs"]
    api_attach["api/attachment.rs"]
    api_error["api/error.rs\n#AppError enum\n#thiserror"]

    tui_app["tui/app.rs\n#App state\n#screen_stack Vec<Screen>\n#KeyAction enum"]
    tui_screens["tui/screens/\n#spaces / pages / comments\n#ratatui render fns"]

    main --> cli
    main --> config
    main --> api
    main --> tui

    cli --> api
    cli --> config
    cli --> output

    api --> api_client
    api --> api_space
    api --> api_page
    api --> api_comment
    api --> api_attach
    api --> api_error

    tui --> tui_app
    tui --> tui_screens
    tui --> api
    tui --> config
```

---

## CLI Command Flow

```mermaid
sequenceDiagram
    participant User
    participant main as main.rs
    participant cli as cli/mod.rs (clap)
    participant handler as cli/<cmd>.rs
    participant cfg as config/mod.rs
    participant client as api/client.rs
    participant api as api/<resource>.rs
    participant out as output/
    participant conf as Confluence REST API

    User->>main: ccli page list DEV
    main->>cli: Cli::parse()
    cli-->>main: Commands::Page(List { space_key: "DEV" })
    main->>handler: cli::page::run(&cli, args)
    handler->>cfg: config::load_or_error()
    cfg-->>handler: Config { url, token, email? }
    handler->>client: Client::new(&config)
    client-->>handler: Client (reqwest pool, auth header baked in)
    handler->>api: list_all_pages(&client, "DEV", ContentType::Page)
    api->>conf: GET /rest/api/content?spaceKey=DEV&type=page
    conf-->>api: JSON response
    api-->>handler: Vec<Page>
    handler->>out: print_table(pages, &output_config)
    out-->>User: formatted table (or TSV if piped)
```

---

## TUI Event Loop

```mermaid
sequenceDiagram
    participant User
    participant main as main.rs
    participant tui as tui/mod.rs
    participant app as tui/app.rs (App)
    participant screens as tui/screens/
    participant tokio as tokio tasks
    participant conf as Confluence REST API

    User->>main: ccli (no subcommand)
    main->>tui: tui::run()
    tui->>tui: config::load_or_error()
    tui->>tui: Client::new(&config)
    tui->>tokio: spawn → list_all_spaces (oneshot channel)
    tui->>tui: ratatui::init() [raw mode + alt screen]

    loop Every 100ms tick
        tui->>screens: terminal.draw(render_spaces / render_pages / render_comments)
        tui->>app: try_recv spaces_rx (oneshot)
        tui->>app: try_recv preview_rx / pages_list_rx / comments_list_rx
        tui->>tui: event::poll(100ms)

        alt Key press
            tui->>app: app.handle_key(key) → KeyAction
            alt KeyAction::DrillDown(space_key)
                tui->>app: screen_stack.push(PagesBrowse)
                tui->>tokio: spawn → list_all_pages (mpsc channel)
            end
            alt KeyAction::DrillDownComments(page_id)
                tui->>app: screen_stack.push(CommentsBrowse)
                tui->>tokio: spawn → list_comments (mpsc channel)
            end
            alt KeyAction::EditPage(page_id)
                tui->>tui: ratatui::restore() [suspend]
                tui->>tui: get_page_detail + $EDITOR + update_page
                tui->>tui: ratatui::init() [restore]
            end
            alt KeyAction::Quit
                tui->>tui: break loop
            end
        else Timeout (100ms)
            tui->>app: app.tick() [spinner + debounce]
            opt 150ms debounce elapsed
                tui->>tokio: spawn → get_space_detail / get_page_detail (preview)
            end
        end
    end

    tokio-->>tui: channel delivers Vec<Space>/Vec<Page>/Vec<Comment>/SpaceDetail
    tui->>tui: ratatui::restore()
    tui-->>main: Ok(())
```

---

## Error Handling Chain

```mermaid
flowchart LR
    thiserror["thiserror\n(AppError enum)\nAuth / Network / Config / Api"]
    anyhow["anyhow\n(.context wrap)"]
    main_chain["main: err.chain()\n.find_map downcast AppError"]
    hint["hint_for(&AppError)\n→ remediation string"]
    stderr["stderr:\nError: ...\n<hint>"]

    thiserror -->|impl std::error::Error| anyhow
    anyhow -->|propagated via ?| main_chain
    main_chain --> hint
    hint --> stderr
```

---

## Key Rust Concepts in This Codebase

| Concept | Where to look |
|---|---|
| `#[derive]` macros (Parser, Serialize, Deserialize) | `cli/mod.rs`, `config/mod.rs`, `api/*.rs` |
| Enums as typed errors (`thiserror`) | `api/error.rs` |
| `anyhow::Result` + `.context()` for error wrapping | everywhere in handlers |
| `async`/`await` + `#[tokio::main]` | `main.rs`, `tui/mod.rs`, all API calls |
| Tokio channels (`oneshot`, `mpsc`) | `tui/mod.rs` — async results from spawned tasks |
| Ownership & cheap `Clone` via `Arc` | `api/client.rs` — `reqwest::Client` is Arc-backed |
| Pattern matching on enums | `main.rs` `match cli.command`, `tui/mod.rs` `match action` |
| `Option<T>` idioms (`take`, `as_deref`, `filter`) | `config/mod.rs`, `tui/mod.rs` |
| `Vec<Screen>` as a navigation stack | `tui/app.rs` `screen_stack` |
| Unit tests in the same file (`#[cfg(test)]`) | every module |
| `Mutex` for serializing parallel tests | `api/client.rs`, `config/mod.rs` |
