//! Pages and Blog Posts browse view renderer — pure drawing functions for the TUI.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, HighlightSpacing, List, ListItem, Padding, Paragraph, Wrap,
};
use ratatui::Frame;

use crate::api::page::{ContentType, Page, PageDetail};
use crate::output::render_xml_to_lines;
use crate::tui::app::{AppState, PagesBrowseState, StatusStyle, SPINNER_FRAMES};

/// Top-level render function for the Pages / Blog Posts browse view.
pub fn render_pages(f: &mut Frame, state: &mut PagesBrowseState, area: Rect) {
    let has_filter = !state.active_filter.is_empty() || matches!(state.browse_state, AppState::Filtering { .. });
    let filter_height: u16 = if has_filter { 3 } else { 0 };

    let vertical = Layout::vertical([
        Constraint::Length(3),              // title block (with shortcuts inside)
        Constraint::Min(0),                 // main body
        Constraint::Length(filter_height),  // filter panel (only when active)
        Constraint::Length(1),              // status bar
    ]);
    let [title_area, body_area, filter_area, status_area] = vertical.areas(area);

    render_title(f, state, title_area);
    render_body(f, state, body_area);
    if has_filter {
        render_filter_panel(f, state, filter_area);
    }
    render_status_bar(f, state, status_area);

    if matches!(&state.browse_state, AppState::Modal) {
        render_help_modal(f, area);
    }
}

/// Title block: rounded cyan border with screen name + shortcuts row inside.
fn render_title(f: &mut Frame, state: &PagesBrowseState, area: Rect) {
    let kind_label = match state.content_type {
        ContentType::Page => "Pages",
        ContentType::BlogPost => "Blog Posts",
    };
    let subtitle = match &state.browse_state {
        AppState::Loading => "Loading…".to_string(),
        AppState::Browse | AppState::Modal => {
            if !state.active_filter.is_empty() {
                format!("{} total / {} shown", state.pages.len(), state.visible_count())
            } else {
                format!("{} total", state.pages.len())
            }
        }
        AppState::Filtering { .. } => format!(
            "{} total / {} shown",
            state.pages.len(),
            state.visible_count()
        ),
    };

    let left_title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            kind_label,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" — ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            state.space_key.clone(),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
    .left_aligned();
    let right_title = Line::from(Span::styled(
        format!(" {} ", subtitle),
        Style::default().fg(Color::DarkGray),
    ))
    .right_aligned();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(left_title)
        .title(right_title);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Shortcuts row — k9s-style <key> tags
    let kb = |k: &'static str| {
        Span::styled(
            format!("<{}>", k),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )
    };
    let desc = |d: &'static str| {
        Span::styled(format!(" {}  ", d), Style::default().fg(Color::DarkGray))
    };
    let shortcuts = Line::from(vec![
        kb("/"),
        desc("Filter"),
        kb("Enter/o"),
        desc("Open"),
        kb("e"),
        desc("Edit"),
        kb("c"),
        desc("Comments"),
        kb("g/G"),
        desc("Top/Bot"),
        kb("?"),
        desc("Help"),
        kb("Esc"),
        desc("Back"),
    ]);
    f.render_widget(Paragraph::new(shortcuts), inner);
}

/// Body: single rounded cyan outer box, list and preview panes inside.
fn render_body(f: &mut Frame, state: &mut PagesBrowseState, area: Rect) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let horizontal = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]);
    let [list_area, preview_area] = horizontal.areas(inner);
    render_list_pane(f, state, list_area);
    render_preview_pane(f, state, preview_area);
}

/// List pane: page titles only (D-44), truncated with "…" if longer than pane width.
fn render_list_pane(f: &mut Frame, state: &mut PagesBrowseState, area: Rect) {
    let kind_plural = match state.content_type {
        ContentType::Page => "pages",
        ContentType::BlogPost => "blog posts",
    };

    if matches!(state.browse_state, AppState::Loading) {
        let block = Block::default()
            .borders(Borders::RIGHT)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(Color::DarkGray))
            .padding(Padding::horizontal(1));
        let p = Paragraph::new(Span::styled(
            "Loading…",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ))
        .alignment(Alignment::Center)
        .block(block);
        f.render_widget(p, area);
        return;
    }

    if state.pages.is_empty() {
        let msg = format!("No {} in this space", kind_plural);
        let block = Block::default()
            .borders(Borders::RIGHT)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(Color::DarkGray))
            .padding(Padding::horizontal(1));
        let p = Paragraph::new(Span::styled(
            msg,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ))
        .alignment(Alignment::Center)
        .block(block);
        f.render_widget(p, area);
        return;
    }
    if state.filtered_indices.is_empty() {
        let active_query = match &state.browse_state {
            AppState::Filtering { query } => query.as_str(),
            _ => state.active_filter.as_str(),
        };
        let msg = if !active_query.is_empty() {
            format!("No matches for \"{}\"", active_query)
        } else {
            let kind = match state.content_type { ContentType::Page => "pages", ContentType::BlogPost => "blog posts" };
            format!("No {}", kind)
        };
        let block = Block::default()
            .borders(Borders::RIGHT)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(Color::DarkGray))
            .padding(Padding::horizontal(1));
        let p = Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)))
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(p, area);
        return;
    }

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    let list_area = inner;

    let inner_width = area.width.saturating_sub(4) as usize;
    let items: Vec<ListItem> = state
        .filtered_indices
        .iter()
        .filter_map(|&idx| state.pages.get(idx))
        .map(|page| {
            let title = if page.title.chars().count() > inner_width && inner_width > 1 {
                let truncated: String = page
                    .title
                    .chars()
                    .take(inner_width.saturating_sub(1))
                    .collect();
                format!("{}…", truncated)
            } else {
                page.title.clone()
            };
            ListItem::new(title)
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ")
        .highlight_spacing(HighlightSpacing::Always);

    f.render_widget(block, area);
    f.render_stateful_widget(list, list_area, &mut state.list_state);
}

/// Preview pane: metadata + stripped body. (D-42, D-43, D-44.)
fn render_preview_pane(f: &mut Frame, state: &PagesBrowseState, area: Rect) {
    let selected = state.selected_page();
    if selected.is_none() {
        let kind = match state.content_type {
            ContentType::Page => "page",
            ContentType::BlogPost => "blog post",
        };
        let msg = format!("Select a {} to preview", kind);
        let block = Block::default()
            .borders(Borders::NONE)
            .padding(Padding::new(1, 1, 0, 0));
        let p = Paragraph::new(Span::styled(
            msg,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ))
        .block(block);
        f.render_widget(p, area);
        return;
    }
    let selected_page = selected.unwrap();
    let detail = state.preview_cache.get(&selected_page.id);
    let text = build_preview_text(selected_page, detail);

    let block = Block::default()
        .borders(Borders::NONE)
        .padding(Padding::new(1, 1, 0, 0));
    let p = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .scroll((state.preview_scroll, 0))
        .block(block);
    f.render_widget(p, area);
}

/// Build the preview pane Text — title prominent at top, compact metadata row, then body.
fn build_preview_text<'a>(page: &'a Page, detail: Option<&'a PageDetail>) -> Text<'a> {
    let mut lines: Vec<Line> = Vec::new();

    let label_style = Style::default().fg(Color::DarkGray);
    let sep_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM);
    let separator = Span::styled("─".repeat(64), sep_style);

    lines.push(Line::from(Span::styled(
        page.title.clone(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(separator.clone()));

    let author = detail
        .and_then(|d| d.version.as_ref())
        .and_then(|v| v.by.as_ref())
        .and_then(|a| a.display_name.clone())
        .unwrap_or_default();
    let version = detail
        .and_then(|d| d.version.as_ref())
        .map(|v| v.number.to_string())
        .or_else(|| page.version.as_ref().map(|v| v.number.to_string()))
        .unwrap_or_default();
    let last_modified = detail
        .and_then(|d| d.version.as_ref())
        .and_then(|v| v.when.clone())
        .or_else(|| page.version.as_ref().and_then(|v| v.when.clone()))
        .map(|s| s.chars().take(10).collect::<String>())
        .unwrap_or_default();
    let parent = detail
        .and_then(|d| d.ancestors.as_ref())
        .and_then(|a| a.last())
        .and_then(|a| a.title.clone())
        .or_else(|| {
            page.ancestors
                .as_ref()
                .and_then(|a| a.last())
                .and_then(|a| a.title.clone())
        })
        .unwrap_or_default();

    let dot = || Span::styled("  ·  ", label_style);
    let mut meta: Vec<Span> = Vec::new();
    if !author.is_empty() {
        meta.push(Span::styled(author, Style::default().fg(Color::LightBlue)));
    }
    if !version.is_empty() {
        if !meta.is_empty() { meta.push(dot()); }
        meta.push(Span::styled(format!("v{}", version), Style::default().fg(Color::Green)));
    }
    if !last_modified.is_empty() {
        if !meta.is_empty() { meta.push(dot()); }
        meta.push(Span::styled(last_modified, label_style));
    }
    if !parent.is_empty() {
        if !meta.is_empty() { meta.push(dot()); }
        meta.push(Span::styled("↳ ", label_style));
        meta.push(Span::styled(parent, label_style));
    }
    if !meta.is_empty() {
        lines.push(Line::from(meta));
    }

    lines.push(Line::from(separator));
    lines.push(Line::from(""));

    if let Some(d) = detail {
        if let Some(body) = d
            .body
            .as_ref()
            .and_then(|b| b.storage.as_ref())
            .and_then(|s| s.value.as_ref())
        {
            lines.extend(render_xml_to_lines(body));
        }
    }

    Text::from(lines)
}

/// Status bar: status_message takes priority; fallback is state-based text.
fn render_status_bar(f: &mut Frame, state: &PagesBrowseState, area: Rect) {
    if let Some(msg) = &state.status_message {
        let color = match msg.style {
            StatusStyle::Success => Color::Green,
            StatusStyle::Error => Color::Red,
            StatusStyle::Info => Color::Reset,
        };
        let p = Paragraph::new(Line::from(Span::styled(
            format!(" {}", msg.text),
            Style::default().fg(color),
        )));
        f.render_widget(p, area);
        return;
    }
    if let Some(err) = &state.error {
        let p = Paragraph::new(Line::from(Span::styled(
            format!(" Error: {}", err),
            Style::default().fg(Color::Red),
        )));
        f.render_widget(p, area);
        return;
    }
    let kind_plural = match state.content_type {
        ContentType::Page => "pages",
        ContentType::BlogPost => "blog posts",
    };
    let line = match &state.browse_state {
        AppState::Loading => {
            let frame = SPINNER_FRAMES[state.spinner_frame % SPINNER_FRAMES.len()];
            Line::from(vec![
                Span::styled(format!(" {}", frame), Style::default().fg(Color::Cyan)),
                Span::raw(format!("  Loading {}…", kind_plural)),
            ])
        }
        AppState::Browse | AppState::Modal => Line::from(Span::styled(
            format!(" {} {}", state.pages.len(), kind_plural),
            Style::default().fg(Color::DarkGray),
        )),
        AppState::Filtering { .. } => Line::from(vec![
            Span::raw(format!(" {}", state.visible_count())),
            Span::styled(
                format!("/{} {} match", state.pages.len(), kind_plural),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    };
    let p = Paragraph::new(line);
    f.render_widget(p, area);
}

/// 3-row bordered filter panel shown below the body when a filter is active.
fn render_filter_panel(f: &mut Frame, state: &PagesBrowseState, area: Rect) {
    let (typing, query) = match &state.browse_state {
        AppState::Filtering { query } => (true, query.as_str()),
        _ => (false, state.active_filter.as_str()),
    };

    let border_color = if typing { Color::Cyan } else { Color::DarkGray };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            " Filter ",
            Style::default().fg(border_color).add_modifier(Modifier::BOLD),
        ));

    let line = if typing {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(query.to_string(), Style::default().fg(Color::Cyan)),
            Span::styled("█", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ])
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(query.to_string(), Style::default().fg(Color::White)),
            Span::styled(
                "  · Esc to clear  / to refine",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
            ),
        ])
    };

    f.render_widget(Paragraph::new(line).block(block), area);
}

/// Full-panel Help modal — fills the entire area (k9s-style).
fn render_help_modal(f: &mut Frame, full_area: Rect) {
    f.render_widget(Clear, full_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Key Bindings ",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::new(2, 2, 1, 1));

    let inner = block.inner(full_area);

    let heading = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let key_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::DarkGray);

    let row = |k: &'static str, desc: &'static str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{:<16}", format!("<{}>", k)), key_style),
            Span::styled(desc, desc_style),
        ])
    };
    let sec = |s: &'static str| Line::from(Span::styled(s, heading));

    let lines = vec![
        sec("Navigation"),
        row("↑/k", "Move up"),
        row("↓/j", "Move down"),
        row("g/G", "Jump to top / bottom"),
        Line::from(""),
        sec("Spaces"),
        row("Enter", "Browse pages in space"),
        row("o", "Open space in browser"),
        Line::from(""),
        sec("Pages"),
        row("Enter/o", "Open page in browser"),
        row("e", "Edit in $EDITOR"),
        row("c", "View comments"),
        row("PgDn/PgUp", "Scroll preview pane"),
        Line::from(""),
        sec("Comments"),
        row("Esc", "Go back to pages"),
        Line::from(""),
        sec("General"),
        row("/", "Filter  (Enter commits · Esc cancels)"),
        row("Esc", "Close overlay / go back"),
        row("?", "Toggle help"),
        row("q", "Quit"),
    ];

    f.render_widget(block, full_area);
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::page::{ContentType as ApiContentType, Page, PageLinks};
    use crate::tui::app::{AppState, StatusMessage, StatusStyle};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn make_page(id: &str, title: &str) -> Page {
        Page {
            id: id.to_string(),
            title: title.to_string(),
            content_type: "page".to_string(),
            version: None,
            ancestors: None,
            links: PageLinks {
                webui: Some(format!("/x/{}", id)),
                self_url: None,
            },
        }
    }

    #[test]
    fn render_pages_does_not_panic_on_loading_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::new("DEV".to_string(), ApiContentType::Page);
        terminal
            .draw(|f| render_pages(f, &mut state, f.area()))
            .expect("draw");
    }

    #[test]
    fn render_pages_does_not_panic_on_browse_state_with_pages() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV",
            ApiContentType::Page,
            vec![make_page("1", "Alpha"), make_page("2", "Beta")],
        );
        terminal
            .draw(|f| render_pages(f, &mut state, f.area()))
            .expect("draw");
    }

    #[test]
    fn render_pages_does_not_panic_on_filter_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV",
            ApiContentType::Page,
            vec![make_page("1", "Alpha")],
        );
        state.browse_state = AppState::Filtering {
            query: "alp".to_string(),
        };
        state.apply_filter("alp");
        terminal
            .draw(|f| render_pages(f, &mut state, f.area()))
            .expect("draw");
    }

    #[test]
    fn render_pages_does_not_panic_on_modal_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV",
            ApiContentType::Page,
            vec![make_page("1", "Alpha")],
        );
        state.browse_state = AppState::Modal;
        terminal
            .draw(|f| render_pages(f, &mut state, f.area()))
            .expect("draw");
    }

    #[test]
    fn render_pages_blog_posts_title_uses_blog_posts_label() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV",
            ApiContentType::BlogPost,
            vec![make_page("1", "Post")],
        );
        terminal
            .draw(|f| render_pages(f, &mut state, f.area()))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let dump: String = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| {
                        buffer
                            .cell((x, y))
                            .map(|c| c.symbol())
                            .unwrap_or(" ")
                            .to_string()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            dump.contains("Blog Posts"),
            "title bar must say 'Blog Posts' for BlogPost content_type. Got:\n{}",
            dump
        );
    }

    #[test]
    fn render_pages_shortcuts_bar_shows_required_bindings() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV",
            ApiContentType::Page,
            vec![make_page("1", "Alpha")],
        );
        terminal
            .draw(|f| render_pages(f, &mut state, f.area()))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let dump: String = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| {
                        buffer
                            .cell((x, y))
                            .map(|c| c.symbol())
                            .unwrap_or(" ")
                            .to_string()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        for token in &["Filter", "Open", "Edit", "Comments", "Help", "Back"] {
            assert!(
                dump.contains(token),
                "shortcuts bar must contain '{}'\nGot:\n{}",
                token,
                dump
            );
        }
    }

    #[test]
    fn render_pages_help_modal_lists_view_comments_action() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV",
            ApiContentType::Page,
            vec![make_page("1", "Alpha")],
        );
        state.browse_state = AppState::Modal;
        terminal
            .draw(|f| render_pages(f, &mut state, f.area()))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let dump: String = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| {
                        buffer
                            .cell((x, y))
                            .map(|c| c.symbol())
                            .unwrap_or(" ")
                            .to_string()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            dump.contains("View comments"),
            "help modal must list 'View comments' row. Got:\n{}",
            dump
        );
    }

    #[test]
    fn render_pages_status_message_renders_when_set() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV",
            ApiContentType::Page,
            vec![make_page("1", "Alpha")],
        );
        state.status_message = Some(StatusMessage {
            text: "Saved.".to_string(),
            style: StatusStyle::Success,
        });
        terminal
            .draw(|f| render_pages(f, &mut state, f.area()))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let dump: String = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| {
                        buffer
                            .cell((x, y))
                            .map(|c| c.symbol())
                            .unwrap_or(" ")
                            .to_string()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            dump.contains("Saved."),
            "status message must render. Got:\n{}",
            dump
        );
    }
}
