//! Spaces browse view renderer — pure drawing functions for the TUI.
//!
//! Requirements covered: TUI-01, TUI-02, TUI-03, TUI-06, SPC-01, SPC-02.
//! All functions are pure: they take &App and &mut Frame and produce no side effects.
//! This makes them testable without a real terminal (state-only unit tests on App).
//!
//! Visual contract source: 02-UI-SPEC.md

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
///
/// Called on every draw cycle from the event loop. Pure: no I/O, no state mutation.
/// `app` is taken as `&mut App` because `render_stateful_widget` requires
/// a mutable reference to ListState.
pub fn render_spaces(f: &mut Frame, app: &mut App, area: Rect) {
    // Vertical layout: title block (3) + body (flex) + status bar (1) + help bar (1)
    // UI-SPEC Layout section
    let vertical = Layout::vertical([
        Constraint::Length(3), // title block (1 border-top + 1 title row + 1 border-bottom)
        Constraint::Min(0),    // main body
        Constraint::Length(1), // status bar
        Constraint::Length(1), // help bar
    ]);
    let [title_area, body_area, status_area, help_area] = vertical.areas(area);

    render_title(f, app, title_area);
    render_body(f, app, body_area);
    render_status_bar(f, app, status_area);
    render_help_bar(f, help_area);

    // Overlays are rendered last so they appear on top
    match &app.state {
        AppState::Filter { query } => {
            let q = query.clone();
            // Compute list pane area for overlay position
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

/// Render the title block with bold title and dim subtitle.
///
/// Subtitle shows loading state, total count, or filtered count per UI-SPEC Title Block.
fn render_title(f: &mut Frame, app: &App, area: Rect) {
    let subtitle = match &app.state {
        AppState::Loading => "Loading…".to_string(),
        AppState::Browse | AppState::Modal => {
            format!("{} total", app.spaces.len())
        }
        AppState::Filter { .. } => {
            format!("{} total / {} shown", app.spaces.len(), app.visible_count())
        }
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
        .borders(Borders::BOTTOM)
        .border_type(BorderType::Plain)
        .title(left_title)
        .title(right_title);

    f.render_widget(block, area);
}

/// Render the main body — horizontal 40%/60% split (D-20).
fn render_body(f: &mut Frame, app: &mut App, area: Rect) {
    let horizontal = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]);
    let [list_area, preview_area] = horizontal.areas(area);

    render_list_pane(f, app, list_area);
    render_preview_pane(f, app, preview_area);
}

/// Render the left list pane with filtered spaces.
///
/// Pattern 4: HighlightSpacing::Always prevents layout shift on navigation.
/// T-02-15: name truncated with "…" when exceeding pane width.
fn render_list_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let is_filter_active = matches!(&app.state, AppState::Filter { .. });

    // Active filter state → cyan border (UI-SPEC List Pane Block)
    let border_style = if is_filter_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_type(BorderType::Plain)
        .border_style(border_style)
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);

    // Empty state during Loading (UI-SPEC Empty State Contract)
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

    // Empty state — no spaces or no filter matches (UI-SPEC Empty State Contract)
    if app.filtered_indices.is_empty() {
        let msg = match &app.state {
            AppState::Filter { query } => format!("No matches for \"{}\"", query),
            _ => "No spaces accessible".to_string(),
        };
        let empty = Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray)))
            .alignment(Alignment::Center);
        f.render_widget(block, area);
        f.render_widget(empty, inner);
        return;
    }

    // Build list items: "{KEY:<8} {name}" (UI-SPEC List item format)
    // T-02-15: truncate name with "…" to prevent layout overflow
    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .filter_map(|&idx| app.spaces.get(idx))
        .map(|space| {
            // 4 = 1 (RIGHT border) + 2 (HighlightSpacing gutter "> ") + 1 (padding)
            let pane_width = area.width.saturating_sub(4) as usize;
            let name_max = pane_width.saturating_sub(9); // 8 for key + 1 space
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
        // HighlightSpacing::Always reserves 2-char gutter, prevents layout shift (TUI-01)
        .highlight_spacing(HighlightSpacing::Always);

    // When the filter overlay is visible it occupies 3 rows at the top of inner,
    // so push the list down to prevent the overlay from covering the first item.
    let list_area = if is_filter_active {
        Rect {
            y: inner.y + 3,
            height: inner.height.saturating_sub(3),
            ..inner
        }
    } else {
        inner
    };

    f.render_widget(block, area);
    f.render_stateful_widget(list, list_area, &mut app.list_state);
}

/// Render the right preview pane showing selected space fields.
///
/// D-18: shows Key, Name, Type, Description, Homepage URL.
/// D-21: "—" fallback for missing fields, "No description" for missing description.
fn render_preview_pane(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::NONE)
        .padding(Padding::new(1, 1, 0, 0));

    let inner = block.inner(area);

    // Resolve selected space through filtered_indices indirection
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

/// Build the preview pane Text from Space + optional SpaceDetail.
///
/// Field labels padded to 14 chars so values align vertically (UI-SPEC column alignment).
/// Missing values fall back to "—" (DIM) per D-21.
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

    // Key value gets a cyan accent
    let key_line = Line::from(vec![
        Span::styled(format!("{:<14}", "Key:"), label_style),
        Span::styled(
            space.key.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    // Type gets a magenta badge
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

/// Render the single-row status bar with state-appropriate content.
///
/// States: Loading (spinner + count), Browse/Modal (idle count DIM), Filter (M/N match),
/// Error (red message). Error takes priority over state.
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
            AppState::Browse | AppState::Modal => {
                // Idle count in DIM per UI-SPEC Status Bar
                Line::from(vec![Span::styled(
                    format!(" {} spaces", app.spaces.len()),
                    Style::default().fg(Color::DarkGray),
                )])
            }
            AppState::Filter { .. } => {
                let m = app.visible_count();
                let n = app.spaces.len();
                Line::from(vec![
                    Span::styled(format!(" {}", m), Style::default()),
                    Span::styled(
                        format!("/{} spaces match", n),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            }
        }
    };

    let status = Paragraph::new(content);
    f.render_widget(status, area);
}

/// Render the static one-row help bar at the bottom.
///
/// Exact spans from UI-SPEC Help Bar section — alternating cyan key / reset description.
fn render_help_bar(f: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled(
            " /",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  filter   "),
        Span::styled(
            "o",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  open   "),
        Span::styled(
            "?",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  help   "),
        Span::styled(
            "q",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  quit"),
    ]);
    let help = Paragraph::new(line);
    f.render_widget(help, area);
}

/// Render the inline filter input overlay at the top of the list pane.
///
/// 3-row block at the top of list_area — does NOT shift list layout (RESEARCH Pattern 9).
/// Cyan border + title, cursor rendered as trailing "█" character.
fn render_filter_overlay(f: &mut Frame, query: &str, list_area: Rect) {
    // 3-row overlay positioned at top of list pane
    let overlay_area = Rect {
        x: list_area.x,
        y: list_area.y,
        width: list_area.width,
        height: 3,
    };

    let cursor_display = format!("{}█", query);

    let filter_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            "Filter",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::horizontal(1));

    // Clear the area first so underlying content doesn't bleed through
    f.render_widget(Clear, overlay_area);
    f.render_widget(
        Paragraph::new(Span::styled(
            cursor_display,
            Style::default().fg(Color::Cyan),
        ))
        .block(filter_block),
        overlay_area,
    );
}

/// Render the help modal centered over the full terminal area.
///
/// Fixed 50x14 Clear + Block overlay (RESEARCH Pattern 8, UI-SPEC Help Modal).
/// Contains full keyboard binding reference grouped by Navigation / Actions / Application.
fn render_help_modal(f: &mut Frame, area: Rect) {
    // Fixed 50x14 centered per UI-SPEC Help Modal dimensions
    let modal_width = 50u16;
    let modal_height = 14u16;
    let x = area.x + area.width.saturating_sub(modal_width) / 2;
    let y = area.y + area.height.saturating_sub(modal_height) / 2;
    let modal_area = Rect {
        x,
        y,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Key Bindings ",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::horizontal(1));

    let inner = block.inner(modal_area);

    let heading = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default().fg(Color::Cyan);
    // Key column: 12 chars wide, left-aligned per UI-SPEC
    let text = Text::from(vec![
        Line::from(Span::styled("Navigation", heading)),
        Line::from(vec![
            Span::styled(format!("  {:<12}", "↑ / k"), key_style),
            Span::raw("Move up"),
        ]),
        Line::from(vec![
            Span::styled(format!("  {:<12}", "↓ / j"), key_style),
            Span::raw("Move down"),
        ]),
        Line::from(""),
        Line::from(Span::styled("Actions", heading)),
        Line::from(vec![
            Span::styled(format!("  {:<12}", "o"), key_style),
            Span::raw("Open in browser"),
        ]),
        Line::from(vec![
            Span::styled(format!("  {:<12}", "/"), key_style),
            Span::raw("Start filter"),
        ]),
        Line::from(vec![
            Span::styled(format!("  {:<12}", "Esc"), key_style),
            Span::raw("Close filter / dismiss"),
        ]),
        Line::from(vec![
            Span::styled(format!("  {:<12}", "?"), key_style),
            Span::raw("Show this help"),
        ]),
        Line::from(""),
        Line::from(Span::styled("Application", heading)),
        Line::from(vec![
            Span::styled(format!("  {:<12}", "q"), key_style),
            Span::raw("Quit"),
        ]),
    ]);

    // Clear area first, then render block and content
    f.render_widget(Clear, modal_area);
    f.render_widget(block, modal_area);
    f.render_widget(Paragraph::new(text), inner);
}
