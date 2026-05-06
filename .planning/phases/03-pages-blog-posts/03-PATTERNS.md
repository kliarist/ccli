# Phase 3: Pages & Blog Posts - Pattern Map

**Mapped:** 2026-05-05
**Files analyzed:** 9 new/modified files
**Analogs found:** 9 / 9

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/api/page.rs` | service | CRUD + request-response | `src/api/space.rs` | exact |
| `src/cli/page.rs` | controller | request-response | `src/cli/space.rs` | exact |
| `src/cli/blog.rs` | controller | request-response | `src/cli/space.rs` | exact |
| `src/output/xml.rs` | utility | transform | `src/output/mod.rs` | role-match |
| `src/tui/app.rs` | store | event-driven | `src/tui/app.rs` (extend) | exact |
| `src/tui/mod.rs` | controller | event-driven | `src/tui/mod.rs` (extend) | exact |
| `src/tui/screens/pages.rs` | component | request-response | `src/tui/screens/spaces.rs` | exact |
| `src/tui/screens/mod.rs` | config | — | `src/tui/screens/mod.rs` (extend) | exact |
| `src/cli/mod.rs` | config | — | `src/cli/mod.rs` (extend) | exact |

---

## Pattern Assignments

### `src/api/page.rs` (service, CRUD + request-response)

**Analog:** `src/api/space.rs`

**Imports pattern** (lines 1-18):
```rust
//! Confluence Content API client — pages and blog posts.
//!
//! Locked decisions implemented:
//! - D-33: flat list, sorted by title ascending
//! - D-43: 150ms debounce + session cache (types defined here; debounce in tui/app.rs)
//! - D-24: URL resolved from _links.webui via resolve_webui_url()
//!
//! Security:
//! - Token NEVER appears in tracing logs.
//! - #[instrument(skip(client))] applied to all public async functions.

use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::api::client::Client;
use crate::api::error::AppError;
```

**Serde struct pattern** (lines 22-94 of space.rs, adapted):
```rust
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ContentListResponse {
    pub results: Vec<Page>,
    pub start: u32,
    pub limit: u32,
    pub size: u32, // count on THIS page, not total — Pitfall 6
    #[serde(rename = "_links")]
    pub links: ContentListLinks,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ContentListLinks {
    pub next: Option<String>, // present when more pages exist
    pub base: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Page {
    pub id: String,           // Confluence content IDs are strings
    pub title: String,
    #[serde(rename = "type")]
    pub content_type: String, // "page" or "blogpost"
    pub version: Option<PageVersion>,
    pub ancestors: Option<Vec<PageAncestor>>,
    #[serde(rename = "_links")]
    pub links: PageLinks,
}
// ... PageVersion, PageAuthor, PageAncestor, PageLinks, PageDetail, PageBody, StorageBody
// Full struct set from RESEARCH.md Code Examples section
```

**Pagination walk pattern** (lines 102-161 of space.rs, adapted):
```rust
// list_all_spaces pattern — copy verbatim, change endpoint + params + sort key
#[instrument(skip(client))]
pub async fn list_all_pages(
    client: &Client,
    space_key: &str,
    content_type: ContentType,
) -> Result<Vec<Page>, AppError> {
    let mut all: Vec<Page> = Vec::new();
    let mut start = 0u32;
    let limit = 50u32;  // D: page list uses limit=50

    loop {
        let url = format!(
            "{}/rest/api/content?spaceKey={}&type={}&start={}&limit={}&expand=version,ancestors",
            client.base_url().trim_end_matches('/'),
            space_key,
            match content_type { ContentType::Page => "page", ContentType::BlogPost => "blogpost" },
            start,
            limit,
        );
        debug!("Fetching content page: GET {}", url);

        let resp = client.inner().get(&url).send().await.map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                AppError::Network(format!("Cannot reach server: {}", e))
            } else {
                AppError::Network(e.to_string())
            }
        })?;

        match resp.status().as_u16() {
            200 => {
                let page: ContentListResponse = resp
                    .json()
                    .await
                    .map_err(|e| AppError::Api(format!("Failed to parse content list: {}", e)))?;
                let fetched = page.results.len() as u32;
                let has_next = page.links.next.is_some();
                all.extend(page.results);
                if !has_next || fetched < limit {
                    break;
                }
                start += limit;
            }
            401 | 403 => {
                return Err(AppError::Auth(
                    "Authentication failed. Run 'ccli init' to reconfigure.".to_string(),
                ))
            }
            status => {
                return Err(AppError::Api(format!(
                    "Unexpected HTTP {} from /rest/api/content",
                    status
                )))
            }
        }
    }

    // D-33: sort by title ascending
    all.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(all)
}
```

**Single-item GET pattern** (lines 167-202 of space.rs, adapted):
```rust
// get_space_detail pattern — copy, change endpoint to /rest/api/content/{id}
#[instrument(skip(client))]
pub async fn get_page_detail(client: &Client, page_id: &str) -> Result<PageDetail, AppError> {
    let url = format!(
        "{}/rest/api/content/{}?expand=body.storage,version,ancestors",
        client.base_url().trim_end_matches('/'),
        page_id,
    );
    debug!("Fetching page detail: GET {}", url);
    // ... same resp.status() match arm structure as get_space_detail
}
```

**POST create pattern** (new, no analog — from RESEARCH.md Pattern 4):
```rust
#[instrument(skip(client))]
pub async fn create_page(
    client: &Client,
    space_key: &str,
    title: &str,
    xml_content: &str,
    content_type: ContentType,
    parent_id: Option<&str>,
) -> Result<Page, AppError> {
    // Build body per RESEARCH.md Pattern 4
    // Pitfall 5: only include "ancestors" for Page with parent — omit for BlogPost
}
```

**PUT update pattern** (new, no analog — from RESEARCH.md Pattern 5):
```rust
#[instrument(skip(client))]
pub async fn update_page(
    client: &Client,
    page_id: &str,
    title: &str,
    space_key: &str,
    new_xml: &str,
    content_type: ContentType,
    current_version: u32,
) -> Result<(), AppError> {
    // D-39: on 409 → return Err with distinct message so caller can preserve temp file
    // Pitfall 1: always send current_version + 1; never guess
    // Pitfall 2: always include title even when unchanged
}
```

**Test pattern** (lines 213-390 of space.rs):
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
    async fn list_all_pages_returns_sorted_vec_on_200() { ... }

    #[tokio::test]
    async fn list_all_pages_returns_auth_error_on_401() { ... }

    #[tokio::test]
    async fn list_all_pages_paginates_until_no_next_link() { ... }

    #[tokio::test]
    async fn get_page_detail_returns_detail_on_200() { ... }
    // ... same 401 / 404 / 500 variants as space.rs
}
```

---

### `src/cli/page.rs` (controller, request-response)

**Analog:** `src/cli/space.rs`

**Imports pattern** (lines 1-14 of space.rs, adapted):
```rust
//! `ccli page` subcommand handlers.
//!
//! Locked decisions implemented:
//! - D-34: space key is required positional argument for list
//! - D-35: TUI when TTY, table when piped (D-06 pattern)
//! - D-36: ccli page view prints plain text via strip_storage_xml()
//! - D-38: ccli page create uses CLI flags with dialoguer fallback
//! - D-39: 409 conflict preserves temp file and shows path in stderr

use anyhow::Context;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;

use crate::api::{Client, page as page_api};
use crate::cli::{Cli, PageArgs, PageCommands};
use crate::config;
use crate::output::{strip_storage_xml, OutputFormatter};
```

**Non-interactive list handler** (lines 17-43 of space.rs, adapted):
```rust
// D-35: TTY dispatch — TUI when interactive, table when piped
pub async fn run(cli: &Cli, args: &PageArgs) -> anyhow::Result<()> {
    match &args.command {
        PageCommands::List { space_key } => handle_list(cli, space_key).await,
        PageCommands::View { page_id }   => handle_view(page_id).await,
        PageCommands::Create { space, title, parent } => handle_create(space, title, parent).await,
        PageCommands::Edit { page_id }   => handle_edit(page_id).await,
    }
}

async fn handle_list(cli: &Cli, space_key: &str) -> anyhow::Result<()> {
    let config = config::load_or_error()
        .map_err(anyhow::Error::from)
        .context("No configuration found. Run 'ccli init' to configure.")?;
    let client = Client::new(&config)?;

    // D-35: TTY → TUI pre-loaded to PagesBrowse; piped/--plain → table
    if std::io::stdout().is_terminal() && !cli.plain {
        // Launch TUI with initial PagesBrowse screen
        return tui::run_with_screen(tui::Screen::PagesBrowse { ... }).await;
    }

    let pages = page_api::list_all_pages(&client, space_key, ContentType::Page)
        .await
        .map_err(anyhow::Error::from)
        .context("Failed to fetch pages")?;

    let formatter = OutputFormatter::new(cli.output_config());
    let headers = &["Title", "ID", "Author"];
    let rows: Vec<Vec<String>> = pages.iter().map(|p| vec![
        p.title.clone(),
        p.id.clone(),
        p.version.as_ref()
            .and_then(|v| v.by.as_ref())
            .and_then(|a| a.display_name.clone())
            .unwrap_or_default(),
    ]).collect();
    formatter.print(headers, &rows);
    Ok(())
}
```

**Dialoguer prompt pattern** (lines 29-66 of init.rs, adapted):
```rust
async fn handle_create(
    space: &Option<String>,
    title: &Option<String>,
    parent: &Option<String>,
) -> anyhow::Result<()> {
    let theme = ColorfulTheme::default();

    // D-38: prompt for missing flags via dialoguer (same pattern as ccli init)
    let space_key: String = match space {
        Some(s) => s.clone(),
        None => Input::with_theme(&theme)
            .with_prompt("Space key")
            .interact_text()
            .context("Space key prompt failed")?,
    };
    let title: String = match title {
        Some(t) => t.clone(),
        None => Input::with_theme(&theme)
            .with_prompt("Page title")
            .interact_text()
            .context("Title prompt failed")?,
    };
    // ... fetch template, open $EDITOR, POST
}
```

---

### `src/cli/blog.rs` (controller, request-response)

**Analog:** `src/cli/space.rs` + `src/cli/page.rs`

**Pattern:** Exact mirror of `src/cli/page.rs` — thin wrappers that call page handlers with `ContentType::BlogPost`. The only structural difference is the absence of the `--parent` flag on `Create` (D-37, Pitfall 5).

**Imports pattern:**
```rust
//! `ccli blog` subcommand handlers.
//!
//! Locked decisions implemented:
//! - D-37: exact mirror of ccli page with ContentType::BlogPost
//! - No --parent flag on create (blog posts have no parent page — Pitfall 5)

use crate::cli::page; // delegate to page handlers with content_type param
```

**Delegation pattern:**
```rust
pub async fn run(cli: &Cli, args: &BlogArgs) -> anyhow::Result<()> {
    match &args.command {
        BlogCommands::List { space_key } =>
            page::handle_list_typed(cli, space_key, ContentType::BlogPost).await,
        BlogCommands::View { post_id } =>
            page::handle_view(post_id).await,
        BlogCommands::Create { space, title } =>
            page::handle_create_typed(space, title, &None, ContentType::BlogPost).await,
        BlogCommands::Edit { post_id } =>
            page::handle_edit(post_id).await,
    }
}
```

---

### `src/output/xml.rs` (utility, transform)

**Analog:** `src/output/mod.rs` (same output module, pure transform function)

**Module declaration style** (line 9 of output/mod.rs):
```rust
// In src/output/mod.rs, add:
pub mod xml;
pub use xml::strip_storage_xml;
```

**Function signature and quick-xml pattern** (from RESEARCH.md Pattern 7):
```rust
//! Storage XML to plain-text converter.
//!
//! Locked decisions implemented:
//! - D-36: strip tags; headings uppercase; code indented 2-space; paragraphs as text
//! - D-42: shared between TUI preview pane and ccli page view stdout output
//!
//! Security:
//! - Input is Confluence storage XML from authenticated API — treated as untrusted content.
//! - quick-xml event-based reader handles malformed XML gracefully (stops, returns partial).

use quick_xml::Reader;
use quick_xml::events::Event;

/// Convert Confluence storage XML to human-readable plain text.
///
/// Transformation rules (D-42, 03-UI-SPEC.md):
/// - h1..h6 content → UPPERCASED text + blank line
/// - p content → text + blank line
/// - code/pre content → lines prefixed "  " (2-space indent)
/// - li content → prefixed "- "
/// - All other tags → strip tag, retain inner text
/// - HTML entities (&amp; &lt; &gt; &quot;) → decoded characters
/// - Consecutive blank lines → collapsed to one blank line
///
/// Pitfall 7: ac: and ri: namespace-prefixed tags — match on full qualified name
/// or use .local_name() to strip prefix. Unknown tags → strip tag, retain text.
pub fn strip_storage_xml(xml: &str) -> String {
    // IMPORTANT: verify read_event_into / unescape method names against
    // quick-xml 0.39 docs at implementation time (Assumption A1, RESEARCH.md)
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    // ... event loop per RESEARCH.md Pattern 7 code example
}
```

**Test pattern** (output/mod.rs test structure):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_converted_to_uppercase_with_blank_line() { ... }

    #[test]
    fn paragraph_converted_to_text_with_blank_line() { ... }

    #[test]
    fn code_block_indented_two_spaces() { ... }

    #[test]
    fn list_item_prefixed_with_dash() { ... }

    #[test]
    fn consecutive_blank_lines_collapsed() { ... }

    #[test]
    fn malformed_xml_returns_partial_output_without_panic() { ... }

    #[test]
    fn ac_namespace_tags_stripped_retain_inner_text() { ... }
}
```

---

### `src/tui/app.rs` (store, event-driven) — EXTEND

**Analog:** `src/tui/app.rs` (this file is extended, not replaced)

**Screen enum addition** (after existing AppState enum, ~line 40):
```rust
// D-31: enum-based screen stack
#[derive(Debug)]
pub enum ContentType {
    Page,
    BlogPost,
}

#[derive(Debug)]
pub enum Screen {
    SpacesBrowse,
    PagesBrowse {
        space_key: String,
        content_type: ContentType,
        state: Box<PagesBrowseState>,
    },
}
```

**PagesBrowseState struct** (new struct, mirrors App fields — RESEARCH.md Pattern 2):
```rust
// Mirrors App struct field-for-field; owns all per-pages-screen state (D-31)
pub struct PagesBrowseState {
    pub pages: Vec<Page>,
    pub list_state: ListState,
    pub filtered_indices: Vec<usize>,
    pub browse_state: AppState,    // Loading | Browse | Filter | Modal (reuse existing enum)
    pub preview_cache: HashMap<String, PageDetail>,
    pub pending_preview_id: Option<String>,
    pub last_selection_change: Option<Instant>,
    pub spinner_frame: usize,
    pub pages_fetched_count: usize,
    pub error: Option<String>,
    pub status_message: Option<StatusMessage>,  // D-41 editor result
    pub space_key: String,
    pub content_type: ContentType,
}

pub struct StatusMessage {
    pub text: String,
    pub style: StatusStyle,
}

pub enum StatusStyle { Success, Error, Info }
```

**App struct extension** (after existing `error` field, line 66):
```rust
// Phase 3 addition to existing App struct
pub screen_stack: Vec<Screen>,
```

**App::new() extension** (line 84):
```rust
// Add to App::new() initializer block:
screen_stack: Vec::new(),
```

**KeyAction extension** (after existing KeyAction enum, line 358):
```rust
pub enum KeyAction {
    None,
    Quit,
    OpenBrowser(String),
    // Phase 3 additions:
    DrillDown(String),       // Enter on a space: push PagesBrowse { space_key }
    PopScreen,               // Esc from PagesBrowse: pop stack
    EditPage(String),        // e on a page: trigger editor workflow
}
```

**PagesBrowseState method pattern** (mirrors App methods, lines 68-356):
```rust
// PagesBrowseState methods — mirror App: set_pages(), set_fetch_error(),
// cache_detail(), selected_id(), select_next(), select_prev(),
// select_first(), select_last(), apply_filter(), tick()
// Copy each method from App, substituting Space->Page, key->id
impl PagesBrowseState {
    pub fn new(space_key: String, content_type: ContentType) -> Self { ... }
    pub fn set_pages(&mut self, pages: Vec<Page>) { ... }
    pub fn set_fetch_error(&mut self, err: &AppError) { ... }
    pub fn cache_detail(&mut self, id: String, detail: PageDetail) { ... }
    pub fn selected_id(&self) -> Option<String> { ... }
    pub fn select_next(&mut self) { /* copy from App::select_next() */ }
    pub fn select_prev(&mut self) { /* copy from App::select_prev() */ }
    pub fn select_first(&mut self) { /* copy from App::select_first() */ }
    pub fn select_last(&mut self) { /* copy from App::select_last() */ }
    /// D-33 + Pitfall 3: ALWAYS reset list_state.select after filter
    pub fn apply_filter(&mut self, query: &str) { /* copy from App::apply_filter() */ }
    pub fn tick(&mut self) { /* copy from App::tick() */ }
    pub fn handle_key(&mut self, key: KeyCode) -> KeyAction { /* see below */ }
}
```

**handle_key for pages** (mirrors App::handle_key pattern, lines 249-343):
```rust
// D-32: Enter on page = OpenBrowser; D-41: e = EditPage
pub fn handle_key(&mut self, key: KeyCode) -> KeyAction {
    match &self.browse_state.clone() {
        AppState::Browse => match key {
            KeyCode::Char('q') => KeyAction::Quit,
            KeyCode::Esc => KeyAction::PopScreen,  // D-30: Esc pops to SpacesBrowse
            KeyCode::Enter | KeyCode::Char('o') => {
                // D-32: Enter = open in browser (same as o)
                if let Some(id) = self.selected_id() {
                    KeyAction::OpenBrowser(id)
                } else { KeyAction::None }
            }
            KeyCode::Char('e') => {
                // D-41: e = edit in $EDITOR
                if let Some(id) = self.selected_id() {
                    KeyAction::EditPage(id)
                } else { KeyAction::None }
            }
            // j/k/g/G/arrows — same as App::handle_key Browse arm (lines 261-277)
            // / — same Filter open (lines 278-282)
            // ? — same Modal open (lines 283-285)
            _ => KeyAction::None,
        },
        // Filter and Modal arms identical to App::handle_key (lines 299-342)
        _ => { /* copy verbatim from App::handle_key */ }
    }
}
```

---

### `src/tui/mod.rs` (controller, event-driven) — EXTEND

**Analog:** `src/tui/mod.rs` (this file is extended)

**run() signature extension** (line 43):
```rust
// Extend to accept optional initial screen (RESEARCH.md Open Question 3)
pub async fn run() -> anyhow::Result<()> {
    run_with_screen(None).await
}

pub async fn run_with_screen(initial_screen: Option<Screen>) -> anyhow::Result<()> {
    // same config load + client build as existing run() lines 44-50
    // push initial_screen onto app.screen_stack before event loop if Some
}
```

**run_app() screen-stack routing** (replace single `render_spaces` call, line 95):
```rust
// D-31: route render based on top of screen stack
terminal.draw(|f| {
    match app.screen_stack.last_mut() {
        None | Some(Screen::SpacesBrowse) =>
            render_spaces(f, &mut app, f.area()),
        Some(Screen::PagesBrowse { state, .. }) =>
            render_pages(f, state, f.area()),
    }
})?;
```

**Screen stack push/pop** (after existing KeyAction::OpenBrowser arm, lines 131-133):
```rust
KeyAction::DrillDown(space_key) => {
    // D-30: Enter on space → push PagesBrowse
    let pages_state = PagesBrowseState::new(space_key.clone(), ContentType::Page);
    app.screen_stack.push(Screen::PagesBrowse {
        space_key: space_key.clone(),
        content_type: ContentType::Page,
        state: Box::new(pages_state),
    });
    // Spawn page list fetch immediately (mirrors spaces fetch on TUI launch)
    let (pages_tx, pages_rx) = oneshot::channel();
    let fetch_client = client.clone();
    let key = space_key.clone();
    tokio::spawn(async move {
        let result = list_all_pages(&fetch_client, &key, ContentType::Page).await;
        let _ = pages_tx.send(result);
    });
}
KeyAction::PopScreen => {
    // D-30: Esc from PagesBrowse → pop back to SpacesBrowse
    app.screen_stack.pop();
}
```

**Editor workflow** (new function, pattern from D-23 headless fallback lines 156-210):
```rust
// D-41: suspend TUI → editor → restore TUI
// Pitfall 4: always restore raw mode even on error
async fn handle_edit_page(
    app: &mut App,
    page_id: &str,
    client: &Client,
    terminal: &mut ratatui::DefaultTerminal,
) -> anyhow::Result<()> {
    // 1. Suspend TUI (mirrors D-23 pattern: ratatui::restore())
    ratatui::restore();

    // 2. Fetch current storage XML + version
    let detail = page_api::get_page_detail(client, page_id).await?;
    let xml = detail.body.as_ref()
        .and_then(|b| b.storage.as_ref())
        .and_then(|s| s.value.as_ref())
        .cloned()
        .unwrap_or_default();
    let current_version = detail.version.as_ref().map(|v| v.number).unwrap_or(1);

    // 3. Write temp file (D-39, Security: sanitize page_id — digits only)
    let path = format!("/tmp/ccli-edit-{}.xml", page_id);
    std::fs::write(&path, &xml)?;

    // 4. Launch $EDITOR ($EDITOR → $VISUAL → vi fallback)
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());
    std::process::Command::new(&editor).arg(&path).status()?;

    // 5. Read modified content
    let new_xml = std::fs::read_to_string(&path)?;

    // 6. PUT to Confluence
    let put_result = page_api::update_page(
        client, page_id, &detail.title, &space_key,
        &new_xml, content_type, current_version,
    ).await;

    // 7. Restore TUI (always — Pitfall 4)
    *terminal = ratatui::init();

    // 8. Show status message in page list status bar (D-41)
    // ... set status_message on PagesBrowseState
    Ok(())
}
```

---

### `src/tui/screens/pages.rs` (component, request-response)

**Analog:** `src/tui/screens/spaces.rs`

**Imports pattern** (lines 1-20 of spaces.rs, adapted):
```rust
//! Pages/blog posts browse view renderer — pure drawing functions for the TUI.
//!
//! Requirements covered: PAGE-01, PAGE-03, PAGE-04, PAGE-07, BLOG-01, BLOG-04.
//! All functions are pure: no I/O, no state mutation.
//!
//! Visual contract source: 03-UI-SPEC.md

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, HighlightSpacing, List, ListItem,
    Padding, Paragraph, Wrap,
};

use crate::api::page::PageDetail;
use crate::output::xml::strip_storage_xml;
use crate::tui::app::{AppState, PagesBrowseState, SPINNER_FRAMES};
```

**Top-level render function** (mirrors render_spaces, lines 26-59):
```rust
// D-44: 40/60 split (same as D-20); list pane shows title only
pub fn render_pages(f: &mut Frame, state: &mut PagesBrowseState, area: Rect) {
    // Identical vertical layout to render_spaces (lines 29-35):
    let vertical = Layout::vertical([
        Constraint::Length(3), // title block
        Constraint::Min(0),    // main body
        Constraint::Length(1), // status bar
        Constraint::Length(1), // help bar
    ]);
    let [title_area, body_area, status_area, help_area] = vertical.areas(area);

    render_title(f, state, title_area);
    render_body(f, state, body_area);
    render_status_bar(f, state, status_area);
    render_help_bar(f, help_area);  // different bindings: o/Enter=browser, e=edit, Esc=back

    // Filter overlay — same render_filter_overlay() as spaces.rs (copy verbatim)
    if let AppState::Filter { query } = &state.browse_state {
        let q = query.clone();
        let horizontal = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ]);
        let [list_area, _] = horizontal.areas(body_area);
        render_filter_overlay(f, &q, list_area);
    }
}
```

**Title block pattern** (lines 64-95 of spaces.rs):
```rust
// Use Line::right_aligned() / Line::left_aligned() — NOT deprecated block::Title (RESEARCH State of the Art)
let left_title = Line::from(Span::styled(
    match state.content_type {
        ContentType::Page => " Pages ",
        ContentType::BlogPost => " Blog Posts ",
    },
    Style::default().add_modifier(Modifier::BOLD),
)).left_aligned();

let right_title = Line::from(Span::styled(
    format!(" {} — {} ", state.space_key, subtitle),
    Style::default().add_modifier(Modifier::DIM),
)).right_aligned();
```

**List pane pattern** (lines 113-188 of spaces.rs, adapted):
```rust
// D-44: title only in list pane (no key column unlike spaces "{KEY:<8} {name}")
// Items: title string only, truncated with "…" same Pitfall-5-safe pattern
let items: Vec<ListItem> = state.filtered_indices.iter()
    .filter_map(|&idx| state.pages.get(idx))
    .map(|page| {
        let pane_width = area.width.saturating_sub(4) as usize;
        let title = if page.title.chars().count() > pane_width && pane_width > 1 {
            let truncated: String = page.title.chars().take(pane_width - 1).collect();
            format!("{}…", truncated)
        } else {
            page.title.clone()
        };
        ListItem::new(title)
    })
    .collect();

// List widget with identical highlight_style / highlight_symbol / highlight_spacing
// as spaces.rs lines 176-185 (copy verbatim)
```

**Preview pane pattern** (lines 195-285 of spaces.rs, adapted):
```rust
// D-42: preview shows stripped plain text from body.storage
// D-43: detail fetched asynchronously; show placeholder while pending
fn render_preview_pane(f: &mut Frame, state: &PagesBrowseState, area: Rect) {
    // Resolve selected page same indirection pattern as spaces.rs lines 203-210
    let selected_page = state.list_state.selected()
        .and_then(|i| state.filtered_indices.get(i))
        .and_then(|&idx| state.pages.get(idx));

    // Detail lookup from preview_cache keyed by page ID (D-43)
    let detail = selected_page.and_then(|p| state.preview_cache.get(&p.id));

    // Preview fields: Title (bold header), Version, Author, Parent, then stripped body text
    // build_preview_text() — same label/value pattern as spaces.rs lines 232-285
    // body text = strip_storage_xml(detail.body.storage.value)
}
```

**Status bar pattern** (lines 291-337 of spaces.rs, adapted):
```rust
// Adds StatusMessage display (D-41 editor result) on top of existing state rendering
fn render_status_bar(f: &mut Frame, state: &PagesBrowseState, area: Rect) {
    // Priority: status_message (editor result) > error > state-based content
    if let Some(msg) = &state.status_message {
        let color = match msg.style {
            StatusStyle::Success => Color::Green,
            StatusStyle::Error => Color::Red,
            StatusStyle::Info => Color::Reset,
        };
        // ... render msg.text in color
        return;
    }
    // Fallback: same pattern as spaces.rs render_status_bar (lines 297-337)
}
```

---

### `src/tui/screens/mod.rs` — EXTEND

**Analog:** `src/tui/screens/mod.rs` (line 1)

**Current content:**
```rust
pub mod spaces;
```

**Extended content:**
```rust
pub mod spaces;
pub mod pages;  // Phase 3: pages and blog posts browse screen
```

---

### `src/cli/mod.rs` — EXTEND

**Analog:** `src/cli/mod.rs`

**Current Commands enum** (lines 41-48):
```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    Init,
    Space(SpaceArgs),
    // Phase 3 will add: Page(PageArgs), Blog(BlogArgs)
}
```

**Extended Commands enum:**
```rust
pub mod init;
pub mod space;
pub mod page;   // Phase 3
pub mod blog;   // Phase 3

#[derive(Subcommand, Debug)]
pub enum Commands {
    Init,
    Space(SpaceArgs),
    Page(PageArgs),  // Phase 3 — D-34
    Blog(BlogArgs),  // Phase 3 — D-37
}

// PageArgs pattern (mirrors SpaceArgs lines 51-55):
#[derive(clap::Args, Debug)]
pub struct PageArgs {
    #[command(subcommand)]
    pub command: PageCommands,
}

#[derive(Subcommand, Debug)]
pub enum PageCommands {
    /// List pages in a space (TUI when TTY, table when piped — D-35).
    List {
        /// Space key (e.g. DEV) — required positional argument (D-34)
        space_key: String,
    },
    /// Print page content as plain text to stdout (D-36).
    View {
        page_id: String,
    },
    /// Create a new page (opens $EDITOR with storage XML template — D-38, D-40).
    Create {
        #[arg(long)]
        space: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        parent: Option<String>,  // omitted from BlogCommands::Create (D-37, Pitfall 5)
    },
    /// Edit an existing page's storage XML in $EDITOR (D-41).
    Edit {
        page_id: String,
    },
}

// BlogArgs / BlogCommands: mirror PageArgs / PageCommands
// Only difference: BlogCommands::Create has no `parent` field
```

**src/main.rs dispatch extension** (lines 37-43, pattern):
```rust
// Add alongside existing Space dispatch (lines 39-41):
Some(Commands::Page(ref args)) => match args.command {
    PageCommands::List { ref space_key } => cli::page::run(&cli, args).await,
    PageCommands::View { ref page_id }   => cli::page::run(&cli, args).await,
    PageCommands::Create { .. }          => cli::page::run(&cli, args).await,
    PageCommands::Edit { ref page_id }   => cli::page::run(&cli, args).await,
},
Some(Commands::Blog(ref args)) => cli::blog::run(&cli, args).await,
```

---

## Shared Patterns

### Instrument + PAT Guard
**Source:** `src/api/space.rs` lines 102-103, 167-168
**Apply to:** All public async functions in `src/api/page.rs`
```rust
#[instrument(skip(client))]
pub async fn list_all_pages(client: &Client, ...) -> Result<...> {
```
The `skip(client)` prevents the PAT token (inside `Client`) from appearing in tracing output.

### HTTP Status Match Arms
**Source:** `src/api/space.rs` lines 130-155
**Apply to:** Every HTTP response in `src/api/page.rs` (list, detail, create, update)
```rust
match resp.status().as_u16() {
    200 => { /* parse */ }
    401 | 403 => return Err(AppError::Auth(
        "Authentication failed. Run 'ccli init' to reconfigure.".to_string()
    )),
    status => return Err(AppError::Api(format!(
        "Unexpected HTTP {} from /rest/api/content",
        status
    ))),
}
// For PUT: add 409 arm before the catch-all:
409 => return Err(AppError::Api(format!(
    "Conflict: page was updated by someone else (version N). \
     Your edits saved at /tmp/ccli-edit-{}.xml — re-open to merge manually.",
    page_id
))),
```

### Config Load + Client Build
**Source:** `src/cli/space.rs` lines 18-21
**Apply to:** `handle_list`, `handle_view`, `handle_create`, `handle_edit` in `src/cli/page.rs` and `src/cli/blog.rs`
```rust
let config = config::load_or_error()
    .map_err(anyhow::Error::from)
    .context("No configuration found. Run 'ccli init' to configure.")?;
let client = Client::new(&config)?;
```

### OutputFormatter Table Print
**Source:** `src/cli/space.rs` lines 28-41
**Apply to:** `handle_list` non-interactive path in `src/cli/page.rs` and `src/cli/blog.rs`
```rust
let formatter = OutputFormatter::new(cli.output_config());
let headers = &["Title", "ID", "Author"];
let rows: Vec<Vec<String>> = pages.iter().map(|p| vec![...]).collect();
formatter.print(headers, &rows);
```

### TUI Suspend / Restore
**Source:** `src/tui/mod.rs` lines 180-210 (`handle_open_browser` headless fallback)
**Apply to:** `handle_edit_page()` in `src/tui/mod.rs`
```rust
// Suspend: ratatui::restore() atomically exits raw mode + alt screen
ratatui::restore();
// ... action ...
// Restore: ratatui::init() atomically re-enters raw mode + alt screen
*terminal = ratatui::init();
```

### Debounce + Preview Cache Fetch
**Source:** `src/tui/mod.rs` lines 113-122 + `src/tui/app.rs` lines 221-236
**Apply to:** `PagesBrowseState::tick()` and the `pending_preview_id` branch in `run_app()`
```rust
// In tick(): after 150ms elapsed, set pending_preview_id if not cached
if let Some(t) = self.last_selection_change {
    if t.elapsed() >= PREVIEW_DEBOUNCE {
        self.last_selection_change = None;
        if let Some(id) = self.selected_id() {
            if !self.preview_cache.contains_key(&id) {
                self.pending_preview_id = Some(id);
            }
        }
    }
}

// In run_app(): if pending_preview_id set, spawn tokio task → mpsc channel
if let Some(id) = pages_state.pending_preview_id.take() {
    let fetch_client = client.clone();
    let tx = preview_tx.clone();
    tokio::spawn(async move {
        let result = get_page_detail(&fetch_client, &id).await;
        let _ = tx.send((id, result)).await;
    });
}
```

### Dialoguer Interactive Prompts
**Source:** `src/cli/init.rs` lines 29-66
**Apply to:** `handle_create` in `src/cli/page.rs` when flags are omitted (D-38)
```rust
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;

let theme = ColorfulTheme::default();
let space_key: String = match space {
    Some(s) => s.clone(),
    None => Input::with_theme(&theme)
        .with_prompt("Space key")
        .interact_text()
        .context("Space key prompt failed")?,
};
```

### Resolve WebUI URL
**Source:** `src/api/space.rs` lines 208-210
**Apply to:** `OpenBrowser` handling in `src/tui/mod.rs` for pages/blog posts
```rust
pub fn resolve_webui_url(base_url: &str, webui: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), webui)
}
// Never hand-build URL from page_id — always use _links.webui from API response (D-24)
```

### Pitfall-5-Safe Filter Reset
**Source:** `src/tui/app.rs` lines 210-218
**Apply to:** `PagesBrowseState::apply_filter()` — must reset `list_state` after every filter
```rust
// ALWAYS reset selection to prevent stale-index panic (Pitfall 3 in RESEARCH.md)
if self.filtered_indices.is_empty() {
    self.list_state.select(None);
} else {
    self.list_state.select(Some(0));
    self.last_selection_change = Some(Instant::now());
}
```

---

## No Analog Found

All files have analogs. The following patterns are genuinely new logic with no existing equivalent in the codebase; implementers should reference RESEARCH.md rather than codebase analogs:

| File | Pattern | New Aspect | Reference |
|------|---------|-----------|-----------|
| `src/api/page.rs` | POST create / PUT update | No existing write endpoints in codebase | RESEARCH.md Patterns 4 & 5 |
| `src/output/xml.rs` | `strip_storage_xml()` | No XML processing exists yet | RESEARCH.md Pattern 7; verify quick-xml 0.39 API at implementation time (Assumption A1) |
| `src/tui/mod.rs` | Screen stack push/pop | Single-screen app today; stack is new | RESEARCH.md Pattern 1 |
| `src/tui/mod.rs` | Editor suspend/restore | Existing headless fallback uses `ratatui::restore()`/`ratatui::init()` but editor workflow adds file I/O + HTTP between | RESEARCH.md Pattern 6, Pitfall 4 |

---

## Metadata

**Analog search scope:** `src/api/`, `src/cli/`, `src/tui/`, `src/output/`
**Files scanned:** 10 source files (space.rs, mod.rs x3, app.rs, spaces.rs, init.rs, output/mod.rs, main.rs, screens/mod.rs)
**Pattern extraction date:** 2026-05-05
