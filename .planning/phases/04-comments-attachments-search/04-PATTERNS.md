# Phase 4: Comments, Attachments & Search - Pattern Map

**Mapped:** 2026-05-07
**Files analyzed:** 10 (new/modified)
**Analogs found:** 10 / 10

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `src/api/comment.rs` | service | CRUD + request-response | `src/api/page.rs` | exact |
| `src/api/attachment.rs` | service | CRUD + file-I/O | `src/api/page.rs` | role-match |
| `src/cli/comment.rs` | controller | request-response | `src/cli/page.rs` | exact |
| `src/cli/attachment.rs` | controller | file-I/O | `src/cli/page.rs` | role-match |
| `src/tui/screens/comments.rs` | component | event-driven | `src/tui/screens/pages.rs` | exact |
| `src/tui/app.rs` | store | event-driven | `src/tui/app.rs` (self) | exact |
| `src/cli/mod.rs` | config | request-response | `src/cli/mod.rs` (self) | exact |
| `src/main.rs` | controller | request-response | `src/main.rs` (self) | exact |
| `src/tui/mod.rs` | controller | event-driven | `src/tui/mod.rs` (self) | exact |
| `src/tui/screens/pages.rs` | component | event-driven | `src/tui/screens/pages.rs` (self) | exact |

---

## Pattern Assignments

### `src/api/comment.rs` (service, CRUD)

**Analog:** `src/api/page.rs`

**File-level doc comment pattern** (lines 1-15):
```rust
//! Confluence Content API client — page comments.
//!
//! Locked decisions implemented:
//! - D-50: list fetches body.storage + version for preview rendering
//! - D-52: add_comment wraps plain text into <p> blocks before POST
//!
//! Security:
//! - PAT NEVER appears in tracing output. #[instrument(skip(client))] on every async fn.
```

**Imports pattern** (analog lines 16-20):
```rust
use serde::Deserialize;
use tracing::{debug, instrument};

use crate::api::client::Client;
use crate::api::error::AppError;
```

**Serde structs pattern** (analog lines 42-101) — define parallel structs for comments:
```rust
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    pub id: String,
    pub title: String,       // Confluence auto-generates; often "Re: <page title>"
    pub version: Option<CommentVersion>,
    pub body: Option<CommentBody>,
    #[serde(rename = "_links")]
    pub links: CommentLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CommentVersion {
    pub number: u32,
    pub when: Option<String>,
    pub by: Option<CommentAuthor>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CommentAuthor {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CommentBody {
    pub storage: Option<StorageBody>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct StorageBody {
    pub value: Option<String>,
    pub representation: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CommentLinks {
    pub webui: Option<String>,
    #[serde(rename = "self")]
    pub self_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CommentListResponse {
    pub results: Vec<Comment>,
    pub start: u32,
    pub limit: u32,
    pub size: u32,
    #[serde(rename = "_links")]
    pub links: CommentListLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CommentListLinks {
    pub next: Option<String>,
}
```

**Core GET list pattern with .query() builder** (analog lines 139-208) — copy pagination loop, change endpoint:
```rust
#[allow(dead_code)]
#[instrument(skip(client))]
pub async fn list_comments(
    client: &Client,
    page_id: &str,
) -> Result<Vec<Comment>, AppError> {
    let base = format!(
        "{}/rest/api/content/{}/child/comment",
        client.base_url().trim_end_matches('/'),
        page_id,
    );
    // WR-05: .query() builder percent-encodes all values
    let resp = client
        .inner()
        .get(&base)
        .query(&[
            ("expand", "body.storage,version"),
            ("limit", "50"),
        ])
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                AppError::Network(format!("Cannot reach server: {}", e))
            } else {
                AppError::Network(e.to_string())
            }
        })?;
    // ... match resp.status() — see error handling pattern below
}
```

**POST comment pattern** (analog lines 262-323) — same `.json(&body)` shape:
```rust
// D-52: wrap plain text into <p> blocks
// POST /rest/api/content/{page_id}/child/comment
let body = serde_json::json!({
    "type": "comment",
    "body": {
        "storage": {
            "value": xml_wrapped,        // plain-text → <p>…</p> wrapping done by caller
            "representation": "storage",
        }
    }
});
let resp = client
    .inner()
    .post(&url)
    .json(&body)
    .send()
    .await
    .map_err(|e| { /* same Network mapping */ })?;
```

**Error handling pattern** (analog lines 178-207):
```rust
match resp.status().as_u16() {
    200 => {
        let page: CommentListResponse = resp.json().await.map_err(|e| {
            AppError::Api(format!("Failed to parse comment list: {}", e))
        })?;
        // ... collect results
    }
    401 | 403 => {
        return Err(AppError::Auth(
            "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
        ))
    }
    status => {
        return Err(AppError::Api(format!(
            "Unexpected HTTP {} from /rest/api/content/{}/child/comment",
            status, page_id
        )))
    }
}
```

**Test pattern** (analog lines 400-414):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use httpmock::prelude::*;

    fn test_client(base_url: &str) -> Client {
        Client::new(&Config {
            url: base_url.to_string(),
            token: "AT-test-token".to_string(),
        })
        .expect("client")
    }

    #[tokio::test]
    async fn list_comments_returns_vec_on_200() { /* ... */ }

    #[tokio::test]
    async fn list_comments_returns_auth_error_on_401() { /* ... */ }

    #[tokio::test]
    async fn add_comment_posts_storage_xml_body() { /* ... */ }
}
```

---

### `src/api/attachment.rs` (service, CRUD + file-I/O)

**Analog:** `src/api/page.rs`

Same imports, struct, and error-handling patterns as `src/api/comment.rs`. Key differences:

**Attachment structs:**
```rust
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub title: String,          // filename in Confluence
    #[serde(rename = "metadata")]
    pub metadata: Option<AttachmentMetadata>,
    pub extensions: Option<AttachmentExtensions>,
    pub version: Option<AttachmentVersion>,
    #[serde(rename = "_links")]
    pub links: AttachmentLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct AttachmentExtensions {
    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,
    #[serde(rename = "fileSize")]
    pub file_size: Option<u64>,
    pub comment: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AttachmentLinks {
    pub download: Option<String>,  // used for attachment get (ATTCH-02)
    pub webui: Option<String>,
}
```

**Multipart upload pattern** (reqwest multipart — D-55):
```rust
// POST /rest/api/content/{page_id}/child/attachment
// Content-Type: multipart/form-data (set automatically by reqwest)
// Header: X-Atlassian-Token: no-check  (required for attachment uploads on DC)
use reqwest::multipart;

let file_bytes = std::fs::read(file_path)
    .map_err(|e| AppError::Api(format!("Cannot read file: {}", e)))?;
let mime_str = mime_guess::from_path(file_path)
    .first_or_octet_stream()
    .to_string();
let part = multipart::Part::bytes(file_bytes)
    .file_name(filename.to_string())
    .mime_str(&mime_str)
    .map_err(|e| AppError::Api(format!("Invalid MIME type: {}", e)))?;
let form = multipart::Form::new().part("file", part);

let resp = client
    .inner()
    .post(&url)
    .header("X-Atlassian-Token", "no-check")  // required for DC attachment upload
    .multipart(form)
    .send()
    .await
    .map_err(|e| { /* same Network mapping */ })?;
```

**Binary download pattern** (ATTCH-02):
```rust
// GET the download URL from attachment._links.download
// Then GET bytes and write to disk
let resp = client
    .inner()
    .get(&download_url)
    .send()
    .await
    .map_err(|e| { /* Network mapping */ })?;
let bytes = resp.bytes().await
    .map_err(|e| AppError::Network(format!("Download failed: {}", e)))?;
std::fs::write(dest_path, &bytes)
    .map_err(|e| AppError::Api(format!("Cannot write file: {}", e)))?;
```

---

### `src/cli/comment.rs` (controller, request-response)

**Analog:** `src/cli/page.rs`

**Imports pattern** (analog lines 17-27):
```rust
use anyhow::Context;

use crate::api::comment::{add_comment, list_comments};
use crate::api::{AppError, Client};
use crate::cli::{Cli, CommentArgs, CommentCommands};
use crate::config;
```

**Top-level dispatcher pattern** (analog lines 33-51):
```rust
pub async fn run(cli: &Cli, args: &CommentArgs) -> anyhow::Result<()> {
    match &args.command {
        CommentCommands::Add { page_id } => handle_add(page_id).await,
    }
}
```

**`sanitize_id` reuse** (analog lines 252-261) — copy verbatim, apply to `page_id`:
```rust
// T-03-14: digits-only validation prevents path traversal in /tmp/ccli-comment-{id}.txt
let id = sanitize_id(page_id)
    .ok_or_else(|| AppError::Api(format!("Invalid id: must be numeric (got {:?})", page_id)))?;
```

**`$EDITOR` workflow** (analog lines 142-146 + 289-303) — copy `launch_editor` verbatim; change template and temp file name:
```rust
// D-51: empty template (no placeholder), D-53
let temp_path = std::env::temp_dir()
    .join(format!("ccli-comment-{}.txt", id));  // .txt extension, not .xml
std::fs::write(&temp_path, "")                   // D-53: empty template
    .with_context(|| format!("Failed to write template to {:?}", temp_path))?;
launch_editor(&temp_path)?;
let plain_text = std::fs::read_to_string(&temp_path)
    .with_context(|| format!("Failed to read comment from {:?}", temp_path))?;
```

**Plain-text → XML wrapping** (D-52) — new helper, no analog exists:
```rust
// D-52: split on blank lines → each paragraph becomes a <p> block
fn wrap_plain_text_to_storage_xml(text: &str) -> String {
    let paragraphs: Vec<&str> = text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()).collect();
    if paragraphs.is_empty() {
        return "<p></p>".to_string();
    }
    paragraphs
        .iter()
        .map(|p| format!("<p>{}</p>", p.replace('\n', " ")))
        .collect::<Vec<_>>()
        .join("")
}
```

**Config + client load pattern** (analog lines 64-67 and 149-151) — copy verbatim:
```rust
let cfg = config::load_or_error()
    .map_err(anyhow::Error::from)
    .context("No configuration found. Run 'ccli init' to configure.")?;
let client = Client::new(&cfg)?;
```

**Error handling** (analog lines 229-245) — same `Err(AppError::Api(msg))` dispatch:
```rust
add_comment(&client, &id, &xml_content)
    .await
    .map_err(anyhow::Error::from)
    .context("Failed to add comment")?;
let _ = std::fs::remove_file(&temp_path);
println!("Comment added.");
Ok(())
```

---

### `src/cli/attachment.rs` (controller, file-I/O)

**Analog:** `src/cli/page.rs`

Same config load, `sanitize_id`, error handling, and client patterns. Additional patterns:

**`ccli attachment list` — OutputFormatter reuse** (analog `handle_list_typed` lines 59-92):
```rust
let formatter = OutputFormatter::new(cli.output_config());
let headers = &["Filename", "Size", "Media-Type", "Date"];
let rows: Vec<Vec<String>> = attachments
    .iter()
    .map(|a| {
        vec![
            a.title.clone(),
            human_size(a.extensions.as_ref().and_then(|e| e.file_size).unwrap_or(0)),
            a.extensions.as_ref().and_then(|e| e.media_type.clone()).unwrap_or_default(),
            a.version.as_ref().and_then(|v| v.when.clone()).unwrap_or_default(),
        ]
    })
    .collect();
formatter.print(headers, &rows);
```

**`ccli attachment get` — download to CWD** (D-54):
```rust
// Default destination: CWD + filename; --output overrides
let dest = output.map(std::path::PathBuf::from)
    .unwrap_or_else(|| std::path::Path::new(".").join(filename));
```

**`ccli attachment add` — MIME auto-detection** (D-55):
```rust
// mime_guess::from_path falls back to application/octet-stream for unknown extensions
let _mime = mime_guess::from_path(file_path).first_or_octet_stream();
```

---

### `src/tui/screens/comments.rs` (component, event-driven)

**Analog:** `src/tui/screens/pages.rs`

**Imports pattern** (analog lines 12-23):
```rust
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, HighlightSpacing, List, ListItem,
    Padding, Paragraph, Wrap,
};

use crate::api::comment::Comment;
use crate::output::strip_storage_xml;          // D-50: reuse unchanged
use crate::tui::app::{AppState, CommentsBrowseState, StatusStyle, SPINNER_FRAMES};
```

**Top-level render function pattern** (analog lines 26-55):
```rust
pub fn render_comments(f: &mut Frame, state: &mut CommentsBrowseState, area: Rect) {
    let vertical = Layout::vertical([
        Constraint::Length(3),   // title bar
        Constraint::Min(0),      // body (list + preview)
        Constraint::Length(1),   // status bar
        Constraint::Length(1),   // help bar
    ]);
    let [title_area, body_area, status_area, help_area] = vertical.areas(area);

    render_title(f, state, title_area);
    render_body(f, state, body_area);
    render_status_bar(f, state, status_area);
    render_help_bar(f, help_area);
    // No filter overlay for v1 (deferred per CONTEXT.md)
}
```

**Body split 40/60 pattern** (analog lines 83-91):
```rust
fn render_body(f: &mut Frame, state: &mut CommentsBrowseState, area: Rect) {
    let horizontal = Layout::horizontal([
        Constraint::Percentage(40),
        Constraint::Percentage(60),
    ]);
    let [list_area, preview_area] = horizontal.areas(area);
    render_list_pane(f, state, list_area);
    render_preview_pane(f, state, preview_area);
}
```

**List pane items — author · date · first line** (D-50). Adapt analog lines 147-181:
```rust
// D-50: "Alice · 2026-05-01 · This needs updating before…"
let author = comment.version.as_ref()
    .and_then(|v| v.by.as_ref())
    .and_then(|a| a.display_name.as_deref())
    .unwrap_or("—");
let date = comment.version.as_ref()
    .and_then(|v| v.when.as_deref())
    .and_then(|w| w.get(..10))   // take YYYY-MM-DD prefix
    .unwrap_or("—");
let first_line = comment.body.as_ref()
    .and_then(|b| b.storage.as_ref())
    .and_then(|s| s.value.as_deref())
    .map(|xml| strip_storage_xml(xml))
    .and_then(|t| t.lines().next().map(|s| s.to_string()))
    .unwrap_or_default();
let label = format!("{} · {} · {}", author, date, first_line);
// Truncate to inner_width with "…" — same logic as pages list pane
```

**Preview pane — full stripped body** (analog lines 183-208):
```rust
// D-50: preview shows full stripped comment body via strip_storage_xml()
// Same block/paragraph/wrap pattern as pages preview pane
let block = Block::default().borders(Borders::NONE)
    .padding(Padding::new(1, 1, 0, 0));
let p = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);
f.render_widget(p, area);
```

**Help bar** (analog lines 334-351) — simpler for comments (no edit/filter):
```rust
fn render_help_bar(f: &mut Frame, area: Rect) {
    let key = |k: &'static str| Span::styled(k,
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let line = Line::from(vec![
        Span::raw(" "),
        key("↑/↓"),
        Span::raw("  navigate   "),
        key("Esc"),
        Span::raw("  back"),
    ]);
    f.render_widget(Paragraph::new(line), area);
}
```

**Test pattern** (analog lines 424-541) — render-does-not-panic tests using `TestBackend`:
```rust
#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_comments_does_not_panic_on_loading_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::new("12345".to_string());
        terminal.draw(|f| render_comments(f, &mut state, f.area())).expect("draw");
    }
}
```

---

### `src/tui/app.rs` (modify — add `CommentsBrowse` to `Screen` enum and new state struct)

**Analog:** `src/tui/app.rs` lines 398-438 (`Screen` enum + `PagesBrowseState`)

**`Screen` enum extension** (analog lines 403-415):
```rust
#[allow(dead_code)]
#[derive(Debug)]
pub enum Screen {
    SpacesBrowse,
    PagesBrowse {
        space_key: String,
        content_type: ContentType,
        state: Box<PagesBrowseState>,
    },
    // ADD (D-48, D-61):
    CommentsBrowse {
        page_id: String,
        state: Box<CommentsBrowseState>,
    },
}
```

**`CommentsBrowseState` struct** — copy `PagesBrowseState` (analog lines 424-438), substitute `Comment` for `Page`, remove `content_type` and `space_key`, add `page_id`:
```rust
#[allow(dead_code)]
#[derive(Debug)]
pub struct CommentsBrowseState {
    pub page_id: String,
    pub comments: Vec<Comment>,
    pub list_state: ListState,
    pub filtered_indices: Vec<usize>,
    pub browse_state: AppState,
    // No preview cache needed — body already in Comment struct (fetched with list)
    pub spinner_frame: usize,
    pub error: Option<String>,
}

#[allow(dead_code)]
impl CommentsBrowseState {
    pub fn new(page_id: String) -> Self { /* same pattern as PagesBrowseState::new */ }

    #[cfg(test)]
    pub fn with_comments(page_id: &str, comments: Vec<Comment>) -> Self { /* ... */ }

    pub fn set_comments(&mut self, comments: Vec<Comment>) { /* mirror set_pages */ }
    pub fn set_fetch_error(&mut self, err: &AppError) { /* mirror */ }
    pub fn selected_comment(&self) -> Option<&Comment> { /* mirror selected_page */ }
    pub fn select_next(&mut self) { /* copy verbatim */ }
    pub fn select_prev(&mut self) { /* copy verbatim */ }
    pub fn select_first(&mut self) { /* copy verbatim */ }
    pub fn select_last(&mut self) { /* copy verbatim */ }
    pub fn tick(&mut self) { /* no debounce needed — body already in struct */ }
    pub fn visible_count(&self) -> usize { /* copy verbatim */ }

    /// Key handler — simpler than pages: no editor, no browser open, no filter for v1.
    pub fn handle_key(&mut self, key: KeyCode) -> KeyAction {
        match &self.browse_state.clone() {
            AppState::Loading => {
                if key == KeyCode::Char('q') { return KeyAction::Quit; }
                KeyAction::None
            }
            AppState::Browse => match key {
                KeyCode::Char('q') => KeyAction::Quit,
                KeyCode::Esc => KeyAction::PopScreen,
                KeyCode::Char('j') | KeyCode::Down => { self.select_next(); KeyAction::None }
                KeyCode::Char('k') | KeyCode::Up   => { self.select_prev(); KeyAction::None }
                KeyCode::Char('g') => { self.select_first(); KeyAction::None }
                KeyCode::Char('G') => { self.select_last();  KeyAction::None }
                _ => KeyAction::None,
            },
            // No filter/modal states for v1 (deferred)
            _ => KeyAction::None,
        }
    }
}
```

**`KeyAction` enum** — add `DrillDownComments` variant alongside existing `DrillDown`:
```rust
// ADD alongside existing KeyAction variants (analog lines 688-704):
/// `c` pressed on PagesBrowse with a selection → push CommentsBrowse (D-48, D-49).
/// String is the selected page id.
DrillDownComments(String),
```

**Add `CommentsBrowse` import** (analog line 22):
```rust
// Add to existing imports in app.rs:
use crate::api::comment::Comment;
```

---

### `src/cli/mod.rs` (modify — add `Comment` and `Attachment` to `Commands`)

**Analog:** `src/cli/mod.rs` lines 11-53

**Module declarations** (analog lines 11-14):
```rust
// ADD after existing `pub mod blog;`:
pub mod comment;
pub mod attachment;
```

**`Commands` enum extension** (analog lines 43-53):
```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    Init,
    Space(SpaceArgs),
    Page(PageArgs),
    Blog(BlogArgs),
    // ADD (D-45 to D-56):
    /// Add or list comments on a Confluence page.
    Comment(CommentArgs),
    /// List, download, or upload attachments on a Confluence page.
    Attachment(AttachmentArgs),
}
```

**New args structs** (analog `PageArgs`/`PageCommands` lines 69-106):
```rust
#[derive(clap::Args, Debug)]
pub struct CommentArgs {
    #[command(subcommand)]
    pub command: CommentCommands,
}

#[derive(Subcommand, Debug)]
pub enum CommentCommands {
    /// Open $EDITOR to write and post a plain-text comment (D-51).
    Add {
        /// Page ID (numeric string).
        page_id: String,
    },
}

#[derive(clap::Args, Debug)]
pub struct AttachmentArgs {
    #[command(subcommand)]
    pub command: AttachmentCommands,
}

#[derive(Subcommand, Debug)]
pub enum AttachmentCommands {
    /// List attachments on a page (D-56).
    List {
        page_id: String,
    },
    /// Download an attachment to the current directory (D-54).
    Get {
        page_id: String,
        filename: String,
        /// Override destination path (D-54).
        #[arg(long)]
        output: Option<String>,
    },
    /// Upload a file as an attachment (D-55).
    Add {
        page_id: String,
        file_path: String,
    },
}
```

---

### `src/main.rs` (modify — add `Comment` and `Attachment` dispatch arms)

**Analog:** `src/main.rs` lines 10-11 and 38-45

**Import extension** (analog lines 10-11):
```rust
// Extend existing destructure:
use cli::{Cli, Commands, SpaceCommands};
// → becomes:
use cli::{Cli, Commands, SpaceCommands};
// (PageArgs/BlogArgs are dispatched via run() in their modules; no import needed here)
```

**`run()` match extension** (analog lines 37-45):
```rust
async fn run() -> anyhow::Result<()> {
    let cli = <Cli as clap::Parser>::parse();

    match cli.command {
        Some(Commands::Init) => cli::init::run(&cli).await,
        Some(Commands::Space(ref args)) => match args.command {
            SpaceCommands::List => cli::space::run(&cli, args).await,
        },
        Some(Commands::Page(ref args)) => cli::page::run(&cli, args).await,
        Some(Commands::Blog(ref args)) => cli::blog::run(&cli, args).await,
        // ADD:
        Some(Commands::Comment(ref args)) => cli::comment::run(&cli, args).await,
        Some(Commands::Attachment(ref args)) => cli::attachment::run(&cli, args).await,
        None => tui::run().await,
    }
}
```

---

### `src/tui/mod.rs` (modify — add `CommentsBrowse` arm and `c` key handler)

**Analog:** `src/tui/mod.rs` lines 44-46, 119-126, 200-270

**Import additions** (analog lines 34-45):
```rust
// ADD to existing imports:
use crate::api::comment::{list_comments, Comment};
use crate::tui::app::{CommentsBrowseState};                 // add to existing App/Screen import
use crate::tui::screens::comments::render_comments;          // mirror render_pages import
```

**Render dispatch** (analog lines 119-126) — add `CommentsBrowse` arm:
```rust
terminal.draw(|f| match app.screen_stack.last_mut() {
    Some(Screen::CommentsBrowse { state, .. }) => {           // ADD
        render_comments(f, state.as_mut(), f.area());
    }
    Some(Screen::PagesBrowse { state, .. }) => {
        render_pages(f, state.as_mut(), f.area());
    }
    None | Some(Screen::SpacesBrowse) => {
        render_spaces(f, &mut app, f.area());
    }
})?;
```

**Channel for comments list fetch** (analog lines 110-115):
```rust
// ADD alongside existing pages_list_tx/pages_list_rx:
let (comments_list_tx, mut comments_list_rx) =
    mpsc::channel::<(String, Result<Vec<Comment>, AppError>)>(4);
```

**Drain comments list results** (analog lines 148-162):
```rust
// ADD alongside pages list drain:
while let Ok((page_id, fetch_result)) = comments_list_rx.try_recv() {
    for screen in app.screen_stack.iter_mut().rev() {
        if let Screen::CommentsBrowse { page_id: pid, state } = screen {
            if *pid == page_id {
                match fetch_result {
                    Ok(comments) => state.set_comments(comments),
                    Err(ref e) => state.set_fetch_error(e),
                }
                break;
            }
        }
    }
}
```

**Key dispatch — add `c` keybinding** (analog lines 200-270):
```rust
// In the event key handling section — route to top-of-stack handler.
// ADD CommentsBrowse routing alongside PagesBrowse:
let action = match app.screen_stack.last_mut() {
    Some(Screen::CommentsBrowse { state, .. }) => state.handle_key(key.code),
    Some(Screen::PagesBrowse { state, .. }) => state.handle_key(key.code),
    _ => app.handle_key(key.code),
};

// ADD `DrillDownComments` arm in the `match action` block (analog lines 211-268):
KeyAction::DrillDownComments(page_id) => {
    let new_state = Box::new(CommentsBrowseState::new(page_id.clone()));
    app.screen_stack.push(Screen::CommentsBrowse {
        page_id: page_id.clone(),
        state: new_state,
    });
    let fetch_client = client.clone();
    let tx = comments_list_tx.clone();
    let id_tag = page_id.clone();
    tokio::spawn(async move {
        let result = list_comments(&fetch_client, &id_tag).await;
        let _ = tx.send((id_tag, result)).await;
    });
}
```

**`c` key in `PagesBrowseState::handle_key`** — add inside `AppState::Browse` arm (analog `src/tui/app.rs` lines 619-648):
```rust
// ADD alongside 'e' (EditPage) in the Browse arm:
KeyCode::Char('c') => {
    // D-49: push CommentsBrowse for the selected page
    if let Some(id) = self.selected_id() {
        KeyAction::DrillDownComments(id)
    } else {
        KeyAction::None
    }
}
```

---

### `src/tui/screens/pages.rs` (modify — add `c: comments` to help bar)

**Analog:** `src/tui/screens/pages.rs` lines 334-351 (`render_help_bar`)

**Current help bar** (lines 337-350):
```rust
let line = Line::from(vec![
    Span::raw(" "),
    key("/"),
    Span::raw("  filter   "),
    key("o"),
    Span::raw("  open   "),
    key("e"),
    Span::raw("  edit   "),
    key("?"),
    Span::raw("  help   "),
    key("Esc"),
    Span::raw("  back"),
]);
```

**Updated help bar** — add `c: comments` entry (D-49):
```rust
let line = Line::from(vec![
    Span::raw(" "),
    key("/"),
    Span::raw("  filter   "),
    key("o"),
    Span::raw("  open   "),
    key("e"),
    Span::raw("  edit   "),
    key("c"),                       // ADD
    Span::raw("  comments   "),     // ADD
    key("?"),
    Span::raw("  help   "),
    key("Esc"),
    Span::raw("  back"),
]);
```

Also update `render_help_modal` (lines 403-419) to add `c` row:
```rust
// ADD in the "Actions" section:
row("c", "Browse comments"),
```

Update the test `render_pages_help_bar_shows_required_bindings` (lines 503-519) — add `"comments"` to the token list:
```rust
for token in &["filter", "open", "edit", "comments", "help", "back"] {
    assert!(dump.contains(token), /* ... */);
}
```

---

## CQL Search Pattern (new behavior in `src/cli/page.rs`)

**Context:** `ccli page search "<CQL>"` is a new `PageCommands` variant. It does NOT follow D-35 (always table, D-45).

**New `PageCommands::Search` variant** (add to `src/cli/mod.rs` `PageCommands`):
```rust
/// Search pages by CQL expression — always table output (D-45, D-46, D-47).
Search {
    /// CQL expression (positional). Example: "type=page AND space=DEV AND text~deploy"
    cql: String,
    /// Maximum results to return (default: 25, D-46).
    #[arg(long, default_value = "25")]
    limit: u32,
},
```

**Handler pattern** — mirrors `handle_list_typed` in `src/cli/page.rs` (lines 59-92), but uses `OutputFormatter` regardless of TTY:
```rust
// D-45: search always uses table — no TUI, regardless of TTY
// Endpoint: GET /rest/api/content/search?cql={CQL}&limit=25&expand=space
let resp = client
    .inner()
    .get(&url)
    .query(&[
        ("cql", cql),
        ("limit", &limit.to_string()),
        ("expand", "space"),
    ])
    .send()
    .await
    .map_err(|e| { /* Network mapping */ })?;
// D-46: columns: title, ID, space, URL
let headers = &["Title", "ID", "Space", "URL"];
```

---

## Shared Patterns

### Authentication (no change needed)
**Source:** `src/api/client.rs` lines 76-91
**Apply to:** All new API functions in `src/api/comment.rs` and `src/api/attachment.rs`

The `Client` struct already injects `Authorization: Bearer <token>` into every request via `default_headers`. All new API functions call `client.inner().get/post/put(url)` — auth is automatic.

### PAT Security Invariant
**Source:** `src/api/page.rs` line 139, `src/api/client.rs` line 100
**Apply to:** Every `async fn` in `src/api/comment.rs` and `src/api/attachment.rs`
```rust
#[allow(dead_code)]
#[instrument(skip(client))]   // PAT NEVER logged — security invariant from all prior phases
pub async fn list_comments(client: &Client, page_id: &str) -> Result<Vec<Comment>, AppError> {
```

### Error Handling (AppError taxonomy)
**Source:** `src/api/error.rs` lines 1-16
**Apply to:** All new API functions and CLI handlers

No new variants needed. Map all new errors to existing:
- `401/403` → `AppError::Auth("Authentication failed. Run 'ccli init' to reconfigure.")`
- `connect/timeout` → `AppError::Network(format!("Cannot reach server: {}", e))`
- Other non-2xx / parse failure → `AppError::Api(format!(...))`

### ID Sanitization
**Source:** `src/cli/page.rs` lines 252-261
**Apply to:** Every CLI handler that takes a `page_id` argument

```rust
fn sanitize_id(s: &str) -> Option<String> {
    if s.is_empty() { return None; }
    if s.chars().all(|c| c.is_ascii_digit()) { Some(s.to_string()) } else { None }
}
```
Copy this function into `src/cli/comment.rs` and `src/cli/attachment.rs`. Do NOT import from `page.rs` — each CLI module is self-contained.

### Config Load + Client Init
**Source:** `src/cli/page.rs` lines 64-67
**Apply to:** Every `handle_*` function in `src/cli/comment.rs` and `src/cli/attachment.rs`
```rust
let cfg = config::load_or_error()
    .map_err(anyhow::Error::from)
    .context("No configuration found. Run 'ccli init' to configure.")?;
let client = Client::new(&cfg)?;
```

### Query Builder (WR-05 injection prevention)
**Source:** `src/api/page.rs` lines 158-168
**Apply to:** All GET endpoints in `src/api/comment.rs` and `src/api/attachment.rs`
```rust
// Always use .query(&[...]) — never interpolate user input into URL strings directly
client.inner().get(&base).query(&[("key", value), ...]).send().await
```

### OutputFormatter Reuse
**Source:** `src/output/mod.rs` lines 31-48; `src/cli/page.rs` lines 74-91
**Apply to:** `ccli attachment list` and `ccli page search` (both always-table)
```rust
let formatter = OutputFormatter::new(cli.output_config());
formatter.print(headers, &rows);
```

### Screen Stack Push/Pop (D-31)
**Source:** `src/tui/mod.rs` lines 228-255 (`DrillDown` arm)
**Apply to:** `DrillDownComments` arm in `src/tui/mod.rs`

Pattern: create state → push Screen → spawn async fetch → send via channel → drain in loop top.

### XML Stripping for Preview
**Source:** `src/output/xml.rs` (imported via `src/output/mod.rs` line 13)
**Apply to:** `src/tui/screens/comments.rs` preview pane (D-50)
```rust
use crate::output::strip_storage_xml;
// Called in render as: strip_storage_xml(&body_xml)
```

---

## No Analog Found

All files have close analogs. Files with partial novelty:

| File | Novel Element | Strategy |
|---|---|---|
| `src/api/attachment.rs` | `reqwest::multipart` upload | Use CONTEXT.md `client.inner().post(url).multipart(form)` pattern; `mime_guess` crate |
| `src/api/attachment.rs` | Binary download + disk write | Use `resp.bytes()` + `std::fs::write` — standard Rust; no complex analog needed |
| `src/cli/comment.rs` | Plain-text → storage XML wrapping | Implement `wrap_plain_text_to_storage_xml()` helper per D-52 spec; no analog exists |
| `src/api/page.rs` | New `Search` subcommand (CQL) | Follows same GET + `.query()` + `OutputFormatter` pattern as `list_all_pages` |

---

## Metadata

**Analog search scope:** `src/api/`, `src/cli/`, `src/tui/`, `src/output/`
**Files read:** 10 source files
**Pattern extraction date:** 2026-05-07
