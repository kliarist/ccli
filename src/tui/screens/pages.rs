//! Pages and Blog Posts browse view renderer — pure drawing functions for the TUI.
//!
//! Requirements covered: PAGE-01, PAGE-03, PAGE-04, BLOG-01.
//! All functions are pure: no I/O, no state mutation. `&mut PagesBrowseState`
//! is required only because Ratatui's `render_stateful_widget` mutates ListState.
//!
//! Visual contract source: 03-UI-SPEC.md
//! Locked decisions implemented: D-31, D-37, D-41, D-42, D-43, D-44.
//!
//! Wired into the event loop in src/tui/mod.rs (Plan 06).

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
    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ]);
    let [title_area, body_area, status_area, help_area] = vertical.areas(area);

    render_title(f, state, title_area);
    render_body(f, state, body_area);
    render_status_bar(f, state, status_area);
    render_help_bar(f, help_area);

    match &state.browse_state {
        AppState::Filter { query } => {
            let q = query.clone();
            let horizontal =
                Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]);
            let [list_area, _] = horizontal.areas(body_area);
            render_filter_overlay(f, &q, list_area);
        }
        AppState::Modal => {
            render_help_modal(f, area);
        }
        _ => {}
    }
}

/// Title block: bold left "Pages — {SPACE-KEY}" + dim right "{N} total" / "Loading…".
fn render_title(f: &mut Frame, state: &PagesBrowseState, area: Rect) {
    let kind_label = match state.content_type {
        ContentType::Page => "Pages",
        ContentType::BlogPost => "Blog Posts",
    };
    let subtitle = match &state.browse_state {
        AppState::Loading => "Loading…".to_string(),
        AppState::Browse | AppState::Modal => format!("{} total", state.pages.len()),
        AppState::Filter { .. } => format!(
            "{} total / {} shown",
            state.pages.len(),
            state.visible_count()
        ),
    };
    let left_title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            kind_label,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" — ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            state.space_key.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
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
        .borders(Borders::BOTTOM)
        .border_type(BorderType::Plain)
        .title(left_title)
        .title(right_title);
    f.render_widget(block, area);
}

/// Body: horizontal 40/60 split → list pane + preview pane.
fn render_body(f: &mut Frame, state: &mut PagesBrowseState, area: Rect) {
    let horizontal = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]);
    let [list_area, preview_area] = horizontal.areas(area);
    render_list_pane(f, state, list_area);
    render_preview_pane(f, state, preview_area);
}

/// List pane: page titles only (D-44), truncated with "…" if longer than pane width.
fn render_list_pane(f: &mut Frame, state: &mut PagesBrowseState, area: Rect) {
    let filter_active = matches!(state.browse_state, AppState::Filter { .. });
    let border_style = if filter_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let kind_plural = match state.content_type {
        ContentType::Page => "pages",
        ContentType::BlogPost => "blog posts",
    };

    if matches!(state.browse_state, AppState::Loading) {
        let block = Block::default()
            .borders(Borders::RIGHT)
            .border_type(BorderType::Plain)
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
        if let AppState::Filter { query } = &state.browse_state {
            let msg = format!("No matches for \"{}\"", query);
            let block = Block::default()
                .borders(Borders::RIGHT)
                .border_type(BorderType::Plain)
                .border_style(border_style)
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
    }

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_type(BorderType::Plain)
        .border_style(border_style)
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);

    // When the filter overlay is visible it occupies 3 rows at the top of inner,
    // so push the list down to prevent the overlay from covering the first item.
    let list_area = if filter_active {
        Rect {
            y: inner.y + 3,
            height: inner.height.saturating_sub(3),
            ..inner
        }
    } else {
        inner
    };

    // List items: title only, truncated to pane inner width
    // 4 = 1 (RIGHT border) + 2 (HighlightSpacing gutter "> ") + 1 (padding)
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

    // Title — prominent, no label
    lines.push(Line::from(Span::styled(
        page.title.clone(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(separator.clone()));

    // Extract metadata
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
        .map(|s| s.chars().take(10).collect::<String>()) // YYYY-MM-DD only
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

    // Single compact metadata line: "thekliar  ·  v3  ·  2026-03-15  ·  ↳ Parent Page"
    let dot = || Span::styled("  ·  ", label_style);
    let mut meta: Vec<Span> = Vec::new();
    if !author.is_empty() {
        meta.push(Span::styled(author, Style::default().fg(Color::LightBlue)));
    }
    if !version.is_empty() {
        if !meta.is_empty() {
            meta.push(dot());
        }
        meta.push(Span::styled(
            format!("v{}", version),
            Style::default().fg(Color::Green),
        ));
    }
    if !last_modified.is_empty() {
        if !meta.is_empty() {
            meta.push(dot());
        }
        meta.push(Span::styled(last_modified, label_style));
    }
    if !parent.is_empty() {
        if !meta.is_empty() {
            meta.push(dot());
        }
        meta.push(Span::styled("↳ ", label_style));
        meta.push(Span::styled(parent, label_style));
    }
    if !meta.is_empty() {
        lines.push(Line::from(meta));
    }

    lines.push(Line::from(separator));
    lines.push(Line::from(""));

    // Body — styled lines from TUI renderer; no line cap (scroll with PgDn/PgUp)
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
        AppState::Filter { .. } => Line::from(vec![
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

/// Help bar — fixed key/description spans per UI-SPEC.
fn render_help_bar(f: &mut Frame, area: Rect) {
    let key = |k: &'static str| {
        Span::styled(
            k,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    };
    let line = Line::from(vec![
        Span::raw(" "),
        key("/"),
        Span::raw("  filter   "),
        key("o"),
        Span::raw("  open   "),
        key("e"),
        Span::raw("  edit   "),
        key("c"),
        Span::raw("  comments   "),
        key("PgDn/PgUp"),
        Span::raw("  scroll   "),
        key("?"),
        Span::raw("  help   "),
        key("Esc"),
        Span::raw("  back"),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// 3-row Filter input overlay at the top of the list pane.
fn render_filter_overlay(f: &mut Frame, query: &str, list_area: Rect) {
    let overlay = Rect {
        x: list_area.x,
        y: list_area.y,
        width: list_area.width,
        height: 3.min(list_area.height),
    };
    f.render_widget(Clear, overlay);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Filter ");
    let cursor_span = Span::styled(
        "█",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let query_span = Span::styled(query, Style::default().fg(Color::Cyan));
    let line = Line::from(vec![Span::raw(" "), query_span, cursor_span]);
    let p = Paragraph::new(line).block(block);
    f.render_widget(p, overlay);
}

/// 50x16 Help modal — UI-SPEC Help Modal (Pages View) content.
fn render_help_modal(f: &mut Frame, full_area: Rect) {
    let modal_w: u16 = 50;
    let modal_h: u16 = 18;
    let x = full_area.x + (full_area.width.saturating_sub(modal_w)) / 2;
    let y = full_area.y + (full_area.height.saturating_sub(modal_h)) / 2;
    let area = Rect {
        x,
        y,
        width: modal_w.min(full_area.width),
        height: modal_h.min(full_area.height),
    };
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Key Bindings ",
            Style::default().add_modifier(Modifier::BOLD),
        ));

    let bold = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default().fg(Color::Cyan);
    let row = |k: &'static str, desc: &'static str| {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<12}", k), key_style),
            Span::raw(desc),
        ])
    };

    let lines = vec![
        Line::from(Span::styled("Navigation", bold)),
        row("↑ / k", "Move up"),
        row("↓ / j", "Move down"),
        row("g", "Jump to top"),
        row("G", "Jump to bottom"),
        row("PgDn / PgUp", "Scroll preview"),
        Line::from(""),
        Line::from(Span::styled("Actions", bold)),
        row("Enter / o", "Open in browser"),
        row("e", "Edit in $EDITOR"),
        row("c", "View comments"),
        row("/", "Start filter"),
        row("Esc", "Close filter / go back"),
        Line::from(""),
        Line::from(Span::styled("Application", bold)),
        row("?", "Show this help"),
        row("q", "Quit"),
    ];
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
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
        state.browse_state = AppState::Filter {
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
    fn render_pages_help_bar_shows_required_bindings() {
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
        for token in &["filter", "open", "edit", "comments", "help", "back"] {
            assert!(
                dump.contains(token),
                "help bar must contain '{}'\nGot:\n{}",
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
