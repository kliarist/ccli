//! Spaces browse view renderer — pure drawing functions for the TUI.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, HighlightSpacing, List, ListItem, Padding, Paragraph, Wrap,
};
use ratatui::Frame;

use crate::api::space::SpaceDetail;
use crate::tui::app::{App, AppState, SPINNER_FRAMES};

/// Top-level render function for the Spaces browse view.
pub fn render_spaces(f: &mut Frame, app: &mut App, area: Rect) {
    let has_filter =
        !app.active_filter.is_empty() || matches!(app.state, AppState::Filtering { .. });
    let filter_height: u16 = if has_filter { 3 } else { 0 };

    let vertical = Layout::vertical([
        Constraint::Length(3),             // title block (with shortcuts inside)
        Constraint::Min(0),                // main body
        Constraint::Length(filter_height), // filter panel (only when active)
        Constraint::Length(1),             // status bar
    ]);
    let [title_area, body_area, filter_area, status_area] = vertical.areas(area);

    render_title(f, app, title_area);
    render_body(f, app, body_area);
    if has_filter {
        render_filter_panel(f, app, filter_area);
    }
    render_status_bar(f, app, status_area);

    if matches!(&app.state, AppState::Modal) {
        render_help_modal(f, area);
    }
}

/// Title block: rounded cyan border with screen name + shortcuts row inside.
fn render_title(f: &mut Frame, app: &App, area: Rect) {
    let has_filter =
        !app.active_filter.is_empty() || matches!(app.state, AppState::Filtering { .. });
    let subtitle = match &app.state {
        AppState::Loading => "Loading…".to_string(),
        _ if has_filter => format!("{} total / {} shown", app.spaces.len(), app.visible_count()),
        _ => format!("{} total", app.spaces.len()),
    };

    let left_title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "Spaces",
            Style::default()
                .fg(Color::Cyan)
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
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    };
    let desc =
        |d: &'static str| Span::styled(format!(" {}  ", d), Style::default().fg(Color::DarkGray));
    let shortcuts = Line::from(vec![
        kb("/"),
        desc("Filter"),
        kb("Enter"),
        desc("Pages"),
        kb("o"),
        desc("Open"),
        kb("g/G"),
        desc("Top/Bot"),
        kb("?"),
        desc("Help"),
        kb("q"),
        desc("Quit"),
    ]);
    f.render_widget(Paragraph::new(shortcuts), inner);
}

/// Body: single rounded cyan outer box, list and preview panes inside.
fn render_body(f: &mut Frame, app: &mut App, area: Rect) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let horizontal = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]);
    let [list_area, preview_area] = horizontal.areas(inner);
    render_list_pane(f, app, list_area);
    render_preview_pane(f, app, preview_area);
}

fn render_list_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);

    if app.spaces.is_empty() && matches!(app.state, AppState::Loading) {
        let loading = Paragraph::new(Span::styled(
            "Loading…",
            Style::default().fg(Color::DarkGray),
        ))
        .alignment(Alignment::Center);
        f.render_widget(block, area);
        f.render_widget(loading, inner);
        return;
    }

    if app.filtered_indices.is_empty() {
        let active_query = match &app.state {
            AppState::Filtering { query } => query.as_str(),
            _ => app.active_filter.as_str(),
        };
        let msg = if !active_query.is_empty() {
            format!("No matches for \"{}\"", active_query)
        } else {
            "No spaces accessible".to_string()
        };
        let empty = Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray)))
            .alignment(Alignment::Center);
        f.render_widget(block, area);
        f.render_widget(empty, inner);
        return;
    }

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .filter_map(|&idx| app.spaces.get(idx))
        .map(|space| {
            let pane_width = area.width.saturating_sub(4) as usize;
            let name_max = pane_width.saturating_sub(9);
            let name = if space.name.chars().count() > name_max && name_max > 1 {
                let truncated: String = space.name.chars().take(name_max - 1).collect();
                format!("{}…", truncated)
            } else {
                space.name.clone()
            };
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<8}", space.key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {}", name)),
            ]);
            ListItem::new(line)
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
    f.render_stateful_widget(list, inner, &mut app.list_state);
}

fn render_preview_pane(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::NONE)
        .padding(Padding::new(1, 1, 0, 0));

    let inner = block.inner(area);

    let selected_space = app
        .list_state
        .selected()
        .and_then(|i| app.filtered_indices.get(i))
        .and_then(|&idx| app.spaces.get(idx));

    if selected_space.is_none() {
        let placeholder = Paragraph::new(Span::styled(
            "Select a space to preview",
            Style::default().fg(Color::DarkGray),
        ));
        f.render_widget(block, area);
        f.render_widget(placeholder, inner);
        return;
    }

    let space = selected_space.unwrap();
    let detail = app.preview_cache.get(&space.key);
    let content = build_preview_text(space, detail);

    let preview = Paragraph::new(content).wrap(Wrap { trim: true });
    f.render_widget(block, area);
    f.render_widget(preview, inner);
}

fn build_preview_text<'a>(
    space: &'a crate::api::space::Space,
    detail: Option<&'a SpaceDetail>,
) -> Text<'a> {
    let label_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let dim_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM);
    let value_style = Style::default();

    let make_field = |label: &'static str, value: Option<String>| -> Line<'a> {
        let val_span = match value {
            Some(v) if !v.is_empty() => Span::styled(v, value_style),
            _ => Span::styled("—", dim_style),
        };
        Line::from(vec![
            Span::styled(format!("{:<14}", label), label_style),
            val_span,
        ])
    };

    let key_line = Line::from(vec![
        Span::styled(format!("{:<14}", "Key:"), label_style),
        Span::styled(
            space.key.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let type_line = Line::from(vec![
        Span::styled(format!("{:<14}", "Type:"), label_style),
        match space.space_type.as_deref() {
            Some(t) if !t.is_empty() => {
                Span::styled(t.to_string(), Style::default().fg(Color::Magenta))
            }
            _ => Span::styled("—", dim_style),
        },
    ]);

    let description = detail
        .and_then(|d| d.description.as_ref())
        .and_then(|d| d.plain.as_ref())
        .and_then(|p| p.value.clone())
        .filter(|s| !s.is_empty());

    let description_line: Line<'a> = match description {
        Some(desc) => Line::from(vec![
            Span::styled(format!("{:<14}", "Description:"), label_style),
            Span::styled(desc, value_style),
        ]),
        None => Line::from(vec![
            Span::styled(format!("{:<14}", "Description:"), label_style),
            Span::styled("No description", dim_style),
        ]),
    };

    let homepage_url = detail
        .and_then(|d| d.homepage.as_ref())
        .and_then(|h| h.links.as_ref())
        .and_then(|l| l.webui.clone());

    Text::from(vec![
        key_line,
        make_field("Name:", Some(space.name.clone())),
        type_line,
        description_line,
        make_field("Homepage URL:", homepage_url),
    ])
}

/// 3-row bordered filter panel shown below the body when a filter is active.
/// Cyan border + cursor when typing; dim border, no cursor when committed.
fn render_filter_panel(f: &mut Frame, app: &App, area: Rect) {
    let (typing, query) = match &app.state {
        AppState::Filtering { query } => (true, query.as_str()),
        _ => (false, app.active_filter.as_str()),
    };

    let border_color = if typing { Color::Cyan } else { Color::DarkGray };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            " Filter ",
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ));

    let line = if typing {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(query.to_string(), Style::default().fg(Color::Cyan)),
            Span::styled(
                "█",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(query.to_string(), Style::default().fg(Color::White)),
            Span::styled(
                "  · Esc to clear  / to refine",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ),
        ])
    };

    f.render_widget(Paragraph::new(line).block(block), area);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let content: Line = if let Some(err) = &app.error {
        Line::from(vec![Span::styled(
            format!(" Error: {}", err),
            Style::default().fg(Color::Red),
        )])
    } else {
        match &app.state {
            AppState::Loading => {
                let spinner = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];
                Line::from(vec![
                    Span::styled(format!(" {} ", spinner), Style::default().fg(Color::Cyan)),
                    Span::styled("Loading spaces…", Style::default()),
                ])
            }
            _ => Line::from(vec![Span::styled(
                format!(" {} spaces", app.spaces.len()),
                Style::default().fg(Color::DarkGray),
            )]),
        }
    };

    let status = Paragraph::new(content);
    f.render_widget(status, area);
}

fn render_help_modal(f: &mut Frame, area: Rect) {
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Key Bindings ",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::new(2, 2, 1, 1));

    let inner = block.inner(area);

    let heading = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::DarkGray);

    // Fixed-width key column so descriptions align vertically.
    let row = |k: &'static str, desc: &'static str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{:<16}", format!("<{}>", k)), key_style),
            Span::styled(desc, desc_style),
        ])
    };
    let sec = |s: &'static str| Line::from(Span::styled(s, heading));

    let text = Text::from(vec![
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
    ]);

    f.render_widget(block, area);
    f.render_widget(Paragraph::new(text), inner);
}
