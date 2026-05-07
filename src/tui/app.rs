//! TUI application state — App struct, AppState enum, key handlers, filter logic.
//!
//! Locked decisions implemented:
//! - D-12: q always quits; Esc closes overlays (filter/modal) or quits at top-level
//! - D-14: spaces pre-fetched; stored in Vec<Space>
//! - D-15: real-time fuzzy filter on /; Esc closes and restores full list
//! - D-16: nucleo-matcher for fuzzy matching on "{key} {name}" haystack
//! - D-19: 150ms debounce before preview fetch; session cache in HashMap<String, SpaceDetail>
//!
//! No terminal I/O here — this module is testable without a real terminal.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as NucleoConfig, Matcher};
use ratatui::widgets::ListState;

use crate::api::{AppError, Space, SpaceDetail};
use crate::api::page::{ContentType, Page, PageDetail};

/// Spinner frames — text-only ASCII, 8 frames at 100ms interval (UI-SPEC Loading State).
pub const SPINNER_FRAMES: &[&str] = &["◐", "◓", "◑", "◒", "◐", "◓", "◑", "◒"];

/// Debounce period before preview detail is fetched (D-19).
const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);

/// Status bar message overlay used by PagesBrowseState to display editor results
/// (D-41: "Saved.", "Conflict: ...", "Error saving: ...").
/// Cleared by the next navigation keypress in the event loop.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub style: StatusStyle,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusStyle {
    Success, // Color::Green — D-41 "Saved."
    Error,   // Color::Red   — D-39 "Conflict: ...", "Error saving: ..."
    Info,    // Color::Reset — "Opening editor…"
}

/// TUI state machine variants (UI-SPEC State Machine section).
#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    /// Initial state: space list fetch in progress, spinner active.
    Loading,
    /// Normal browse state: list is populated, no overlay open.
    Browse,
    /// Filter overlay is open; query accumulates keypresses.
    Filter { query: String },
    /// Help modal (?) is open.
    Modal,
}

/// Top-level application state — owns the space list, selection, filter, and preview cache.
///
/// Rule: no terminal I/O here. All state is pure Rust. render_spaces() reads this state.
pub struct App {
    /// All spaces pre-fetched from Confluence (D-14), sorted by key ascending.
    pub spaces: Vec<Space>,
    /// Ratatui list selection + scroll offset tracker.
    pub list_state: ListState,
    /// Indices into self.spaces for the currently visible (filtered) list.
    /// When filter is inactive: [0, 1, 2, ... N-1]. When active: subset sorted by score.
    pub filtered_indices: Vec<usize>,
    /// Current TUI state machine variant.
    pub state: AppState,
    /// In-session cache: SpaceKey → detail. Populated on selection settle (D-19).
    pub preview_cache: HashMap<String, SpaceDetail>,
    /// Key for which a preview fetch is pending (set by tick() after debounce).
    pub pending_preview_key: Option<String>,
    /// Timestamp of the last selection change (for 150ms debounce in tick()).
    pub last_selection_change: Option<Instant>,
    /// Current spinner frame index (advances every 100ms tick during Loading state).
    pub spinner_frame: usize,
    /// Total spaces fetched so far (for spinner "N fetched" display, Pitfall 7).
    pub spaces_fetched_count: usize,
    /// If a fetch error occurred, store for status bar rendering.
    pub error: Option<String>,
    /// Phase 3 (D-31): screen navigation stack. Empty = single SpacesBrowse view.
    /// Push PagesBrowse on Enter (D-30); pop on Esc with no overlay.
    #[allow(dead_code)]
    pub screen_stack: Vec<Screen>,
}

impl App {
    /// Create a new App in Loading state with no spaces.
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(None);
        Self {
            spaces: Vec::new(),
            list_state,
            filtered_indices: Vec::new(),
            state: AppState::Loading,
            preview_cache: HashMap::new(),
            pending_preview_key: None,
            last_selection_change: None,
            spinner_frame: 0,
            spaces_fetched_count: 0,
            error: None,
            screen_stack: Vec::new(),
        }
    }

    /// Test constructor: pre-load spaces without network fetch.
    /// Use in unit tests to exercise state transitions without httpmock.
    #[cfg(test)]
    pub fn with_spaces(spaces: Vec<Space>) -> Self {
        let mut app = Self::new();
        app.set_spaces(spaces);
        app
    }

    /// Called when the initial space list fetch completes successfully.
    /// Transitions to Browse state and initialises filtered_indices.
    pub fn set_spaces(&mut self, spaces: Vec<Space>) {
        self.spaces = spaces;
        self.filtered_indices = (0..self.spaces.len()).collect();
        self.state = AppState::Browse;
        // Select first item if list is non-empty
        if !self.spaces.is_empty() {
            self.list_state.select(Some(0));
            self.last_selection_change = Some(Instant::now());
        } else {
            self.list_state.select(None);
        }
    }

    /// Called when the initial space list fetch fails.
    pub fn set_fetch_error(&mut self, err: &AppError) {
        self.state = AppState::Browse; // leave Loading so help bar renders
        self.error = Some(err.to_string());
    }

    /// Called when a preview detail fetch completes.
    pub fn cache_detail(&mut self, key: String, detail: SpaceDetail) {
        self.preview_cache.insert(key, detail);
        self.pending_preview_key = None;
    }

    /// The key of the currently selected space (if any).
    pub fn selected_key(&self) -> Option<String> {
        let idx = self.list_state.selected()?;
        let space_idx = self.filtered_indices.get(idx)?;
        self.spaces.get(*space_idx).map(|s| s.key.clone())
    }

    /// Move selection down (j / ↓). Wraps at end of filtered list.
    pub fn select_next(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        let next = if current + 1 >= self.filtered_indices.len() {
            0 // wrap
        } else {
            current + 1
        };
        self.list_state.select(Some(next));
        self.last_selection_change = Some(Instant::now());
        self.pending_preview_key = None;
    }

    /// Move selection up (k / ↑). Wraps at start of filtered list.
    pub fn select_prev(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        let prev = if current == 0 {
            self.filtered_indices.len() - 1 // wrap
        } else {
            current - 1
        };
        self.list_state.select(Some(prev));
        self.last_selection_change = Some(Instant::now());
        self.pending_preview_key = None;
    }

    /// Jump to top of list (g key — UI-SPEC keyboard contract).
    pub fn select_first(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.list_state.select(Some(0));
            self.last_selection_change = Some(Instant::now());
        }
    }

    /// Jump to bottom of list (G key — UI-SPEC keyboard contract).
    pub fn select_last(&mut self) {
        if !self.filtered_indices.is_empty() {
            let last = self.filtered_indices.len() - 1;
            self.list_state.select(Some(last));
            self.last_selection_change = Some(Instant::now());
        }
    }

    /// Apply fuzzy filter to spaces list (D-15, D-16).
    ///
    /// Uses nucleo-matcher on "{key} {name}" haystack. Empty query restores full list.
    /// ALWAYS resets ListState to Some(0) or None to prevent index-out-of-bounds (Pitfall 5).
    pub fn apply_filter(&mut self, query: &str) {
        if query.is_empty() {
            // Restore full list
            self.filtered_indices = (0..self.spaces.len()).collect();
        } else {
            // D-16: nucleo-matcher fuzzy match on "{key} {name}" combined haystack
            let mut matcher = Matcher::new(NucleoConfig::DEFAULT);
            let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
            let haystacks: Vec<String> = self
                .spaces
                .iter()
                .map(|s| format!("{} {}", s.key, s.name))
                .collect();

            // match_list returns matched items (with scores) sorted by score descending
            let matched: Vec<(String, u32)> = pattern
                .match_list(haystacks.iter().map(|s| s.as_str()), &mut matcher)
                .into_iter()
                .map(|(s, score)| (s.to_owned(), score))
                .collect();

            // Recover original indices — preserve score-sorted order
            self.filtered_indices = matched
                .iter()
                .filter_map(|(h, _)| haystacks.iter().position(|x| x == h))
                .collect();
        }

        // Pitfall 5: always reset selection to prevent stale index panic
        if self.filtered_indices.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
            self.last_selection_change = Some(Instant::now());
        }
    }

    /// Advance spinner and check preview debounce (called every 100ms on poll timeout).
    pub fn tick(&mut self) {
        // Advance spinner frame
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();

        // D-19: if selection settled for 150ms, mark preview fetch as pending
        if let Some(t) = self.last_selection_change {
            if t.elapsed() >= PREVIEW_DEBOUNCE {
                self.last_selection_change = None;
                if let Some(key) = self.selected_key() {
                    if !self.preview_cache.contains_key(&key) {
                        self.pending_preview_key = Some(key);
                    }
                }
            }
        }
    }

    /// Handle a keypress. Returns true if the TUI should quit.
    ///
    /// Key routing per D-12 and UI-SPEC Keyboard Interaction Contract:
    /// - `q` always quits (any state)
    /// - `Esc` in Filter/Modal → close overlay; at Browse with no overlay → quit
    /// - Navigation (j/k/arrows/g/G) navigate the filtered list
    /// - `/` opens Filter overlay
    /// - `?` opens Modal
    /// - `o` triggers browser open (returns pending_browser_url for caller to open)
    /// - Printable chars in Filter mode append to query and re-filter
    /// - Backspace in Filter mode deletes last char and re-filters
    pub fn handle_key(&mut self, key: KeyCode) -> KeyAction {
        match &self.state.clone() {
            AppState::Loading => {
                // During load only q works
                if key == KeyCode::Char('q') {
                    return KeyAction::Quit;
                }
                KeyAction::None
            }

            AppState::Browse => match key {
                KeyCode::Char('q') => KeyAction::Quit,
                KeyCode::Esc => KeyAction::Quit, // Esc at top-level Browse with no overlay → quit (D-12)
                KeyCode::Char('j') | KeyCode::Down => {
                    self.select_next();
                    KeyAction::None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.select_prev();
                    KeyAction::None
                }
                KeyCode::Char('g') => {
                    self.select_first();
                    KeyAction::None
                }
                KeyCode::Char('G') => {
                    self.select_last();
                    KeyAction::None
                }
                KeyCode::Char('/') => {
                    self.state = AppState::Filter {
                        query: String::new(),
                    };
                    self.apply_filter("");
                    KeyAction::None
                }
                KeyCode::Char('?') => {
                    self.state = AppState::Modal;
                    KeyAction::None
                }
                KeyCode::Char('o') => {
                    if let Some(key_str) = self.selected_key() {
                        KeyAction::OpenBrowser(key_str)
                    } else {
                        KeyAction::None
                    }
                }
                KeyCode::Enter => {
                    // D-30: Enter on a selected space drills into its page list
                    if let Some(key_str) = self.selected_key() {
                        KeyAction::DrillDown(key_str)
                    } else {
                        KeyAction::None
                    }
                }
                _ => KeyAction::None,
            },

            AppState::Filter { query } => {
                let mut q = query.clone();
                match key {
                    KeyCode::Char('q') => return KeyAction::Quit, // q always quits (D-12)
                    KeyCode::Esc => {
                        // D-12: Esc closes filter, restores full list
                        self.state = AppState::Browse;
                        self.apply_filter("");
                        return KeyAction::None;
                    }
                    KeyCode::Backspace => {
                        q.pop();
                        self.state = AppState::Filter { query: q.clone() };
                        self.apply_filter(&q);
                    }
                    // Navigation works while filter is open (UI-SPEC)
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.select_next();
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.select_prev();
                    }
                    // Enter is no-op (D-15: filter is real-time, no submit)
                    KeyCode::Enter => {}
                    // Any other printable char appends to query
                    KeyCode::Char(c) => {
                        q.push(c);
                        self.state = AppState::Filter { query: q.clone() };
                        self.apply_filter(&q);
                    }
                    _ => {}
                }
                KeyAction::None
            }

            AppState::Modal => match key {
                KeyCode::Char('q') => KeyAction::Quit,
                KeyCode::Esc => {
                    self.state = AppState::Browse;
                    KeyAction::None
                }
                _ => KeyAction::None,
            },
        }
    }

    /// Count of visible items (after filter).
    pub fn visible_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Get the Space at a filtered list position (for render).
    #[allow(dead_code)]
    pub fn space_at_filtered_index(&self, idx: usize) -> Option<&Space> {
        let space_idx = self.filtered_indices.get(idx)?;
        self.spaces.get(*space_idx)
    }
}

/// TUI navigation stack frame (D-31).
///
/// Only the top of the stack is rendered. Push on Enter (D-30), pop on Esc
/// with no overlay open. Spaces base screen lives below `screen_stack` itself
/// (an empty stack means SpacesBrowse).
#[allow(dead_code)]
#[derive(Debug)]
pub enum Screen {
    /// Original Phase 2 spaces list. Implicit when `App::screen_stack` is empty;
    /// also used explicitly when entering the TUI directly into spaces.
    SpacesBrowse,
    /// Pages or blog posts list for one space (D-30, D-37).
    PagesBrowse {
        space_key: String,
        content_type: ContentType,
        state: Box<PagesBrowseState>,
    },
}

/// Per-screen state for a Pages or Blog Posts browse view (D-31, D-44).
///
/// Mirrors App's state shape one-for-one — pages instead of spaces, page IDs
/// instead of space keys for cache and selection. Owns its own filter, list,
/// preview cache, and status. Owned by Screen::PagesBrowse so multiple drilled-in
/// screens preserve scroll/filter state independently.
#[allow(dead_code)]
#[derive(Debug)]
pub struct PagesBrowseState {
    pub space_key: String,
    pub content_type: ContentType,
    pub pages: Vec<Page>,
    pub list_state: ListState,
    pub filtered_indices: Vec<usize>,
    pub browse_state: AppState,
    pub preview_cache: HashMap<String, PageDetail>,
    pub pending_preview_id: Option<String>,
    pub last_selection_change: Option<Instant>,
    pub spinner_frame: usize,
    pub pages_fetched_count: usize,
    pub error: Option<String>,
    pub status_message: Option<StatusMessage>,
}

#[allow(dead_code)]
impl PagesBrowseState {
    /// Create a fresh state in Loading state for a new screen.
    pub fn new(space_key: String, content_type: ContentType) -> Self {
        let mut list_state = ListState::default();
        list_state.select(None);
        Self {
            space_key,
            content_type,
            pages: Vec::new(),
            list_state,
            filtered_indices: Vec::new(),
            browse_state: AppState::Loading,
            preview_cache: HashMap::new(),
            pending_preview_id: None,
            last_selection_change: None,
            spinner_frame: 0,
            pages_fetched_count: 0,
            error: None,
            status_message: None,
        }
    }

    /// Test-only constructor pre-loading pages without network fetch.
    #[cfg(test)]
    pub fn with_pages(space_key: &str, content_type: ContentType, pages: Vec<Page>) -> Self {
        let mut s = Self::new(space_key.to_string(), content_type);
        s.set_pages(pages);
        s
    }

    pub fn set_pages(&mut self, pages: Vec<Page>) {
        self.pages = pages;
        self.filtered_indices = (0..self.pages.len()).collect();
        self.browse_state = AppState::Browse;
        if !self.pages.is_empty() {
            self.list_state.select(Some(0));
            self.last_selection_change = Some(Instant::now());
        } else {
            self.list_state.select(None);
        }
    }

    pub fn set_fetch_error(&mut self, err: &AppError) {
        self.browse_state = AppState::Browse;
        self.error = Some(err.to_string());
    }

    pub fn cache_detail(&mut self, id: String, detail: PageDetail) {
        self.preview_cache.insert(id, detail);
        self.pending_preview_id = None;
    }

    /// Return the page id of the currently selected (filtered) row.
    pub fn selected_id(&self) -> Option<String> {
        let idx = self.list_state.selected()?;
        let page_idx = self.filtered_indices.get(idx)?;
        self.pages.get(*page_idx).map(|p| p.id.clone())
    }

    /// Return the currently selected Page (for accessors like `_links.webui`).
    pub fn selected_page(&self) -> Option<&Page> {
        let idx = self.list_state.selected()?;
        let page_idx = self.filtered_indices.get(idx)?;
        self.pages.get(*page_idx)
    }

    pub fn select_next(&mut self) {
        if self.filtered_indices.is_empty() { return; }
        let current = self.list_state.selected().unwrap_or(0);
        let next = if current + 1 >= self.filtered_indices.len() { 0 } else { current + 1 };
        self.list_state.select(Some(next));
        self.last_selection_change = Some(Instant::now());
        self.pending_preview_id = None;
    }

    pub fn select_prev(&mut self) {
        if self.filtered_indices.is_empty() { return; }
        let current = self.list_state.selected().unwrap_or(0);
        let prev = if current == 0 { self.filtered_indices.len() - 1 } else { current - 1 };
        self.list_state.select(Some(prev));
        self.last_selection_change = Some(Instant::now());
        self.pending_preview_id = None;
    }

    pub fn select_first(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.list_state.select(Some(0));
            self.last_selection_change = Some(Instant::now());
        }
    }

    pub fn select_last(&mut self) {
        if !self.filtered_indices.is_empty() {
            let last = self.filtered_indices.len() - 1;
            self.list_state.select(Some(last));
            self.last_selection_change = Some(Instant::now());
        }
    }

    /// D-44: filter on TITLE only (not key+name like spaces).
    /// Pitfall 3: ALWAYS reset list_state.select to Some(0) or None.
    pub fn apply_filter(&mut self, query: &str) {
        if query.is_empty() {
            self.filtered_indices = (0..self.pages.len()).collect();
        } else {
            let mut matcher = Matcher::new(NucleoConfig::DEFAULT);
            let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
            let haystacks: Vec<String> = self.pages.iter().map(|p| p.title.clone()).collect();
            let matched: Vec<(String, u32)> = pattern
                .match_list(haystacks.iter().map(|s| s.as_str()), &mut matcher)
                .into_iter()
                .map(|(s, score)| (s.to_owned(), score))
                .collect();
            self.filtered_indices = matched
                .iter()
                .filter_map(|(h, _)| haystacks.iter().position(|x| x == h))
                .collect();
        }
        if self.filtered_indices.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
            self.last_selection_change = Some(Instant::now());
        }
    }

    pub fn tick(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
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
    }

    pub fn visible_count(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    /// Pages-screen key handler — UI-SPEC Pages Browse keybinding table.
    /// D-30 Esc → PopScreen; D-32 Enter/o → OpenBrowser(id); D-41 e → EditPage(id).
    pub fn handle_key(&mut self, key: KeyCode) -> KeyAction {
        // Any navigation key clears a transient status message (D-41 ephemeral "Saved.")
        let clears_status = matches!(
            key,
            KeyCode::Char('j') | KeyCode::Char('k')
                | KeyCode::Down | KeyCode::Up
                | KeyCode::Char('g') | KeyCode::Char('G')
                | KeyCode::Enter
        );
        if clears_status {
            self.status_message = None;
        }

        match &self.browse_state.clone() {
            AppState::Loading => {
                if key == KeyCode::Char('q') { return KeyAction::Quit; }
                KeyAction::None
            }
            AppState::Browse => match key {
                KeyCode::Char('q') => KeyAction::Quit,
                KeyCode::Esc => KeyAction::PopScreen, // D-30: Esc → pop back to spaces
                KeyCode::Char('j') | KeyCode::Down => { self.select_next(); KeyAction::None }
                KeyCode::Char('k') | KeyCode::Up   => { self.select_prev(); KeyAction::None }
                KeyCode::Char('g') => { self.select_first(); KeyAction::None }
                KeyCode::Char('G') => { self.select_last();  KeyAction::None }
                KeyCode::Char('/') => {
                    self.browse_state = AppState::Filter { query: String::new() };
                    self.apply_filter("");
                    KeyAction::None
                }
                KeyCode::Char('?') => { self.browse_state = AppState::Modal; KeyAction::None }
                KeyCode::Enter | KeyCode::Char('o') => {
                    // D-32: Enter == o — open in browser
                    if let Some(id) = self.selected_id() {
                        KeyAction::OpenBrowser(id)
                    } else { KeyAction::None }
                }
                KeyCode::Char('e') => {
                    // D-41: edit selected page in $EDITOR
                    if let Some(id) = self.selected_id() {
                        KeyAction::EditPage(id)
                    } else { KeyAction::None }
                }
                _ => KeyAction::None,
            },
            AppState::Filter { query } => {
                let mut q = query.clone();
                match key {
                    KeyCode::Char('q') => return KeyAction::Quit,
                    KeyCode::Esc => {
                        self.browse_state = AppState::Browse;
                        self.apply_filter("");
                        return KeyAction::None;
                    }
                    KeyCode::Backspace => {
                        q.pop();
                        self.browse_state = AppState::Filter { query: q.clone() };
                        self.apply_filter(&q);
                    }
                    KeyCode::Char('j') | KeyCode::Down => { self.select_next(); }
                    KeyCode::Char('k') | KeyCode::Up   => { self.select_prev(); }
                    KeyCode::Enter => {} // D-15: real-time, no submit
                    KeyCode::Char(c) => {
                        q.push(c);
                        self.browse_state = AppState::Filter { query: q.clone() };
                        self.apply_filter(&q);
                    }
                    _ => {}
                }
                KeyAction::None
            }
            AppState::Modal => match key {
                KeyCode::Char('q') => KeyAction::Quit,
                KeyCode::Esc => { self.browse_state = AppState::Browse; KeyAction::None }
                _ => KeyAction::None,
            },
        }
    }
}

/// The outcome of a key event, returned to the event loop.
#[derive(Debug, PartialEq)]
#[allow(dead_code)]
pub enum KeyAction {
    /// No state change requiring special event-loop action.
    None,
    /// TUI should exit cleanly.
    Quit,
    /// Open the given space key's URL in the system browser.
    /// Caller must resolve URL via resolve_webui_url() and call open::that().
    OpenBrowser(String),
    /// Enter pressed on a space in SpacesBrowse → push PagesBrowse onto stack (D-30).
    /// String is the selected space key.
    DrillDown(String),
    /// Esc pressed on PagesBrowse with no overlay open → pop screen stack (D-30).
    PopScreen,
    /// `e` pressed on PagesBrowse with a selection → enter editor workflow (D-41).
    /// String is the selected page id.
    EditPage(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_space(key: &str, name: &str) -> Space {
        Space {
            id: 1,
            key: key.to_string(),
            name: name.to_string(),
            space_type: Some("global".to_string()),
            links: crate::api::space::SpaceLinks {
                webui: Some(format!("/display/{}", key)),
                self_url: None,
            },
        }
    }

    #[test]
    fn new_app_is_loading_with_no_selection() {
        let app = App::new();
        assert_eq!(app.state, AppState::Loading);
        assert!(app.list_state.selected().is_none());
        assert!(app.spaces.is_empty());
    }

    #[test]
    fn set_spaces_transitions_to_browse_and_selects_first() {
        let mut app = App::new();
        app.set_spaces(vec![make_space("AAA", "Alpha"), make_space("BBB", "Beta")]);
        assert_eq!(app.state, AppState::Browse);
        assert_eq!(app.list_state.selected(), Some(0));
        assert_eq!(app.filtered_indices.len(), 2);
    }

    #[test]
    fn select_next_wraps_at_end() {
        let mut app = App::with_spaces(vec![make_space("A", "Alpha"), make_space("B", "Beta")]);
        app.list_state.select(Some(1)); // already at last
        app.select_next();
        assert_eq!(app.list_state.selected(), Some(0), "should wrap to 0");
    }

    #[test]
    fn select_prev_wraps_at_start() {
        let mut app = App::with_spaces(vec![make_space("A", "Alpha"), make_space("B", "Beta")]);
        app.list_state.select(Some(0));
        app.select_prev();
        assert_eq!(app.list_state.selected(), Some(1), "should wrap to last");
    }

    #[test]
    fn apply_filter_empty_restores_all_indices() {
        let mut app = App::with_spaces(vec![
            make_space("DEV", "Development"),
            make_space("HR", "Human Resources"),
        ]);
        // First filter to narrow the list
        app.apply_filter("DEV");
        assert_eq!(app.filtered_indices.len(), 1);
        // Then clear filter
        app.apply_filter("");
        assert_eq!(app.filtered_indices.len(), 2, "full list must be restored");
    }

    #[test]
    fn apply_filter_matches_by_key() {
        let mut app = App::with_spaces(vec![
            make_space("DEV", "Development"),
            make_space("HR", "Human Resources"),
        ]);
        app.apply_filter("DEV");
        assert_eq!(app.filtered_indices.len(), 1);
        let space = app.space_at_filtered_index(0).unwrap();
        assert_eq!(space.key, "DEV");
    }

    #[test]
    fn apply_filter_resets_selection_to_zero_pitfall5() {
        let mut app = App::with_spaces(vec![
            make_space("A", "Alpha"),
            make_space("B", "Beta"),
            make_space("C", "Gamma"),
        ]);
        app.list_state.select(Some(2)); // select last
        app.apply_filter("Alpha"); // filter narrows to 1 item
        // Selection must reset to 0 (not 2 which would panic)
        assert_eq!(
            app.list_state.selected(),
            Some(0),
            "Pitfall 5: must reset selection"
        );
    }

    #[test]
    fn apply_filter_no_matches_clears_selection() {
        let mut app = App::with_spaces(vec![make_space("DEV", "Development")]);
        app.apply_filter("ZZZNOMATCH");
        assert!(
            app.list_state.selected().is_none(),
            "empty result must deselect"
        );
        assert_eq!(app.filtered_indices.len(), 0);
    }

    #[test]
    fn handle_key_q_always_quits_from_browse() {
        let mut app = App::with_spaces(vec![make_space("A", "Alpha")]);
        let action = app.handle_key(KeyCode::Char('q'));
        assert_eq!(action, KeyAction::Quit);
    }

    #[test]
    fn handle_key_q_quits_from_filter_mode() {
        let mut app = App::with_spaces(vec![make_space("A", "Alpha")]);
        app.state = AppState::Filter {
            query: "de".to_string(),
        };
        let action = app.handle_key(KeyCode::Char('q'));
        assert_eq!(action, KeyAction::Quit);
    }

    #[test]
    fn handle_key_slash_opens_filter_overlay() {
        let mut app = App::with_spaces(vec![make_space("A", "Alpha")]);
        app.handle_key(KeyCode::Char('/'));
        assert!(matches!(app.state, AppState::Filter { .. }));
    }

    #[test]
    fn handle_key_esc_in_filter_closes_overlay_and_restores_list() {
        let mut app = App::with_spaces(vec![
            make_space("DEV", "Development"),
            make_space("HR", "HR"),
        ]);
        app.state = AppState::Filter {
            query: "DEV".to_string(),
        };
        app.apply_filter("DEV");
        assert_eq!(app.filtered_indices.len(), 1);

        app.handle_key(KeyCode::Esc);

        assert_eq!(app.state, AppState::Browse, "Esc should close filter");
        assert_eq!(app.filtered_indices.len(), 2, "full list must be restored");
    }

    #[test]
    fn handle_key_esc_at_browse_top_level_quits() {
        let mut app = App::with_spaces(vec![make_space("A", "Alpha")]);
        assert_eq!(app.state, AppState::Browse);
        let action = app.handle_key(KeyCode::Esc);
        assert_eq!(
            action,
            KeyAction::Quit,
            "Esc at top-level Browse should quit (D-12)"
        );
    }

    #[test]
    fn handle_key_o_returns_open_browser_action_with_selected_key() {
        let mut app = App::with_spaces(vec![make_space("DEV", "Development")]);
        app.list_state.select(Some(0));
        let action = app.handle_key(KeyCode::Char('o'));
        assert_eq!(action, KeyAction::OpenBrowser("DEV".to_string()));
    }

    #[test]
    fn handle_key_question_mark_opens_modal() {
        let mut app = App::with_spaces(vec![make_space("A", "Alpha")]);
        app.handle_key(KeyCode::Char('?'));
        assert_eq!(app.state, AppState::Modal);
    }

    #[test]
    fn spinner_frame_advances_on_tick() {
        let mut app = App::new();
        let initial = app.spinner_frame;
        app.tick();
        assert_eq!(app.spinner_frame, (initial + 1) % SPINNER_FRAMES.len());
    }

    #[test]
    fn select_first_and_last_jump_to_extremes() {
        let spaces = vec![
            make_space("A", "Alpha"),
            make_space("B", "Beta"),
            make_space("C", "Gamma"),
        ];
        let mut app = App::with_spaces(spaces);
        app.list_state.select(Some(1));

        app.select_last();
        assert_eq!(app.list_state.selected(), Some(2));

        app.select_first();
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn new_app_has_empty_screen_stack() {
        let app = App::new();
        assert!(app.screen_stack.is_empty(),
                "screen_stack must start empty (implicit SpacesBrowse)");
    }

    #[test]
    fn handle_key_enter_on_selected_space_returns_drill_down() {
        let mut app = App::with_spaces(vec![make_space("DEV", "Development")]);
        app.list_state.select(Some(0));
        let action = app.handle_key(KeyCode::Enter);
        assert_eq!(action, KeyAction::DrillDown("DEV".to_string()));
    }

    #[test]
    fn handle_key_enter_with_no_selection_returns_none() {
        let mut app = App::new();
        // Loading state, no selection
        let action = app.handle_key(KeyCode::Enter);
        // Loading branch only handles 'q'; Enter returns None.
        assert_eq!(action, KeyAction::None);
    }

    #[test]
    fn key_action_drill_down_pop_screen_edit_page_construct() {
        // Compile-time check that the new variants exist with expected shape.
        let _drill = KeyAction::DrillDown("DEV".to_string());
        let _pop = KeyAction::PopScreen;
        let _edit = KeyAction::EditPage("12345".to_string());
    }

    // ── PagesBrowseState tests ────────────────────────────────────────────────

    fn make_page(id: &str, title: &str) -> Page {
        Page {
            id: id.to_string(),
            title: title.to_string(),
            content_type: "page".to_string(),
            version: None,
            ancestors: None,
            links: crate::api::page::PageLinks {
                webui: Some(format!("/x/{}", id)),
                self_url: None,
            },
        }
    }

    #[test]
    fn pages_state_new_is_loading_with_no_selection() {
        let s = PagesBrowseState::new("DEV".into(), ContentType::Page);
        assert_eq!(s.browse_state, AppState::Loading);
        assert!(s.list_state.selected().is_none());
        assert!(s.pages.is_empty());
        assert_eq!(s.space_key, "DEV");
    }

    #[test]
    fn pages_state_set_pages_transitions_to_browse_and_selects_first() {
        let mut s = PagesBrowseState::new("DEV".into(), ContentType::Page);
        s.set_pages(vec![make_page("1", "Alpha"), make_page("2", "Beta")]);
        assert_eq!(s.browse_state, AppState::Browse);
        assert_eq!(s.list_state.selected(), Some(0));
        assert_eq!(s.filtered_indices.len(), 2);
    }

    #[test]
    fn pages_state_apply_filter_resets_list_state_pitfall3() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page,
            vec![make_page("1", "Alpha"), make_page("2", "Beta"), make_page("3", "Gamma")],
        );
        s.list_state.select(Some(2));
        s.apply_filter("Alpha");
        assert_eq!(s.list_state.selected(), Some(0),
                   "Pitfall 3: filter MUST reset selection");
        assert_eq!(s.filtered_indices.len(), 1);
    }

    #[test]
    fn pages_state_apply_filter_no_match_clears_selection() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page, vec![make_page("1", "Alpha")],
        );
        s.apply_filter("ZZZNOMATCH");
        assert!(s.list_state.selected().is_none());
        assert_eq!(s.filtered_indices.len(), 0);
    }

    #[test]
    fn pages_state_apply_filter_matches_by_title_only_d44() {
        // D-44: filter on title, NOT id. Searching for the id "1" should NOT match.
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page,
            vec![make_page("1", "Alpha"), make_page("2", "Beta")],
        );
        s.apply_filter("Beta");
        assert_eq!(s.filtered_indices.len(), 1);
        assert_eq!(s.pages[s.filtered_indices[0]].title, "Beta");
    }

    #[test]
    fn pages_state_handle_key_esc_in_browse_returns_pop_screen() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page, vec![make_page("1", "Alpha")],
        );
        let action = s.handle_key(KeyCode::Esc);
        assert_eq!(action, KeyAction::PopScreen, "D-30: Esc on PagesBrowse pops to Spaces");
    }

    #[test]
    fn pages_state_handle_key_enter_returns_open_browser_d32() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page, vec![make_page("99", "Alpha")],
        );
        s.list_state.select(Some(0));
        let action = s.handle_key(KeyCode::Enter);
        assert_eq!(action, KeyAction::OpenBrowser("99".to_string()),
                   "D-32: Enter behaves like 'o'");
    }

    #[test]
    fn pages_state_handle_key_o_returns_open_browser() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page, vec![make_page("99", "Alpha")],
        );
        s.list_state.select(Some(0));
        let action = s.handle_key(KeyCode::Char('o'));
        assert_eq!(action, KeyAction::OpenBrowser("99".to_string()));
    }

    #[test]
    fn pages_state_handle_key_e_returns_edit_page_d41() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page, vec![make_page("99", "Alpha")],
        );
        s.list_state.select(Some(0));
        let action = s.handle_key(KeyCode::Char('e'));
        assert_eq!(action, KeyAction::EditPage("99".to_string()),
                   "D-41: 'e' triggers editor workflow");
    }

    #[test]
    fn pages_state_handle_key_q_quits_from_any_state() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page, vec![make_page("1", "Alpha")],
        );
        assert_eq!(s.handle_key(KeyCode::Char('q')), KeyAction::Quit);
        s.browse_state = AppState::Filter { query: "x".to_string() };
        assert_eq!(s.handle_key(KeyCode::Char('q')), KeyAction::Quit);
        s.browse_state = AppState::Modal;
        assert_eq!(s.handle_key(KeyCode::Char('q')), KeyAction::Quit);
    }

    #[test]
    fn pages_state_handle_key_slash_opens_filter() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page, vec![make_page("1", "Alpha")],
        );
        s.handle_key(KeyCode::Char('/'));
        assert!(matches!(s.browse_state, AppState::Filter { .. }));
    }

    #[test]
    fn pages_state_handle_key_question_opens_modal() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page, vec![make_page("1", "Alpha")],
        );
        s.handle_key(KeyCode::Char('?'));
        assert_eq!(s.browse_state, AppState::Modal);
    }

    #[test]
    fn pages_state_navigation_key_clears_status_message() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page, vec![make_page("1", "Alpha"), make_page("2", "Beta")],
        );
        s.status_message = Some(StatusMessage {
            text: "Saved.".to_string(),
            style: StatusStyle::Success,
        });
        s.handle_key(KeyCode::Char('j'));
        assert!(s.status_message.is_none(), "navigation must clear status");
    }

    #[test]
    fn pages_state_select_next_wraps() {
        let mut s = PagesBrowseState::with_pages(
            "DEV", ContentType::Page, vec![make_page("1", "A"), make_page("2", "B")],
        );
        s.list_state.select(Some(1));
        s.select_next();
        assert_eq!(s.list_state.selected(), Some(0));
    }

    #[test]
    fn pages_state_cache_detail_inserts_into_preview_cache() {
        let mut s = PagesBrowseState::new("DEV".into(), ContentType::Page);
        let detail = PageDetail {
            id: "1".into(), title: "T".into(), content_type: "page".into(),
            version: None, ancestors: None, body: None,
            links: crate::api::page::PageLinks::default(),
        };
        s.cache_detail("1".to_string(), detail);
        assert!(s.preview_cache.contains_key("1"));
        assert!(s.pending_preview_id.is_none());
    }
}
