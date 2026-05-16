//! CommentsBrowse render — D-48..D-50, CMNT-01.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, HighlightSpacing, List, ListItem, Padding, Paragraph, Wrap,
};
use ratatui::Frame;

use crate::api::comment::Comment;
use crate::output::strip_storage_xml;
use crate::tui::app::{AppState, CommentsBrowseState, SPINNER_FRAMES};

/// Top-level render — vertical split: title (with shortcuts), body, status.
pub fn render_comments(f: &mut Frame, state: &mut CommentsBrowseState, area: Rect) {
    let vertical = Layout::vertical([
        Constraint::Length(3),  // title block (with shortcuts inside)
        Constraint::Min(0),     // body
        Constraint::Length(1),  // status bar
    ]);
    let [title_area, body_area, status_area] = vertical.areas(area);

    render_title(f, state, title_area);
    render_body(f, state, body_area);
    render_status_bar(f, state, status_area);

    if matches!(&state.browse_state, AppState::Modal) {
        render_help_modal(f, area);
    }
}

/// Title block: rounded cyan border with screen name + shortcuts row inside.
fn render_title(f: &mut Frame, state: &CommentsBrowseState, area: Rect) {
    let count_text = match state.browse_state {
        AppState::Loading => "Loading…".to_string(),
        _ => format!("{} total", state.comments.len()),
    };

    let left_title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "Comments",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" — ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            state.page_id.clone(),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
    .left_aligned();
    let right_title = Line::from(Span::styled(
        format!(" {} ", count_text),
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
        kb("g/G"),
        desc("Top/Bot"),
        kb("?"),
        desc("Help"),
        kb("Esc"),
        desc("Back"),
        kb("q"),
        desc("Quit"),
    ]);
    f.render_widget(Paragraph::new(shortcuts), inner);
}

/// Body: single rounded cyan outer box, list and preview panes inside.
fn render_body(f: &mut Frame, state: &mut CommentsBrowseState, area: Rect) {
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

fn render_list_pane(f: &mut Frame, state: &mut CommentsBrowseState, area: Rect) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::horizontal(1));
    let inner_w = block.inner(area).width as usize;

    if matches!(state.browse_state, AppState::Loading) {
        let p = Paragraph::new("Loading…")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(p, area);
        return;
    }
    if state.comments.is_empty() {
        let p = Paragraph::new("No comments on this page")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = state
        .comments
        .iter()
        .map(|c| ListItem::new(format_list_row(c, inner_w.saturating_sub(2))))
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ")
        .highlight_spacing(HighlightSpacing::Always);
    f.render_stateful_widget(list, area, &mut state.list_state);
}

/// Build a colored line: LightBlue author · DarkGray date · default first-line, truncated.
fn format_list_row(c: &Comment, max_chars: usize) -> Line<'static> {
    let author = c
        .version
        .as_ref()
        .and_then(|v| v.by.as_ref())
        .and_then(|a| a.display_name.as_deref())
        .unwrap_or("—")
        .to_string();
    let date = c
        .version
        .as_ref()
        .and_then(|v| v.when.as_deref())
        .map(|w| w.get(..10).unwrap_or(w))
        .unwrap_or("—")
        .to_string();
    let body = c
        .body
        .as_ref()
        .and_then(|b| b.storage.as_ref())
        .and_then(|s| s.value.as_deref())
        .unwrap_or("");
    let stripped = strip_storage_xml(body);
    let first_line = stripped.lines().next().unwrap_or("").to_string();

    let sep = " · ";
    let total = author.chars().count()
        + sep.len()
        + date.chars().count()
        + sep.len()
        + first_line.chars().count();

    let sep_style = Style::default().fg(Color::DarkGray);
    let date_style = Style::default().fg(Color::DarkGray);
    let author_style = Style::default().fg(Color::LightBlue);

    if total <= max_chars {
        return Line::from(vec![
            Span::styled(author, author_style),
            Span::styled(sep, sep_style),
            Span::styled(date, date_style),
            Span::styled(sep, sep_style),
            Span::raw(first_line),
        ]);
    }

    let raw = format!("{} · {} · {}", author, date, first_line);
    Line::raw(truncate_with_ellipsis(&raw, max_chars))
}

fn truncate_with_ellipsis(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let take = max.saturating_sub(1);
    let mut out: String = s.chars().take(take).collect();
    out.push('…');
    out
}

fn render_preview_pane(f: &mut Frame, state: &CommentsBrowseState, area: Rect) {
    let block = Block::default()
        .borders(Borders::NONE)
        .padding(Padding::new(1, 1, 0, 0));
    if state.comments.is_empty() {
        let p = Paragraph::new("Select a comment to preview")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(p, area);
        return;
    }
    let Some(c) = state.selected_comment() else {
        let p = Paragraph::new("No comment selected")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(p, area);
        return;
    };

    let bold = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let dim_dash = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM);

    let author = c
        .version
        .as_ref()
        .and_then(|v| v.by.as_ref())
        .and_then(|a| a.display_name.clone());
    let date = c
        .version
        .as_ref()
        .and_then(|v| v.when.as_deref())
        .map(|w| w.get(..10).unwrap_or(w).to_string());
    let body_xml = c
        .body
        .as_ref()
        .and_then(|b| b.storage.as_ref())
        .and_then(|s| s.value.as_deref())
        .unwrap_or("");
    let body_text = strip_storage_xml(body_xml);

    fn field<'a>(label: &'a str, value: Option<&'a str>, bold: Style, dim: Style) -> Line<'a> {
        Line::from(vec![
            Span::styled(format!("{:<14}", label), bold),
            match value {
                Some(v) => Span::raw(v.to_string()),
                None => Span::styled("—", dim),
            },
        ])
    }

    let mut lines: Vec<Line> = Vec::new();
    let author_value_line = Line::from(vec![
        Span::styled(format!("{:<14}", "Author:"), bold),
        match author.as_deref() {
            Some(a) => Span::styled(a.to_string(), Style::default().fg(Color::LightBlue)),
            None => Span::styled("—", dim_dash),
        },
    ]);
    lines.push(author_value_line);
    lines.push(field("Date:", date.as_deref(), bold, dim_dash));
    lines.push(Line::from(""));
    for body_line in body_text.lines() {
        lines.push(Line::from(body_line.to_string()));
    }
    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(block);
    f.render_widget(p, area);
}

fn render_status_bar(f: &mut Frame, state: &CommentsBrowseState, area: Rect) {
    let line = match (&state.browse_state, &state.error) {
        (_, Some(msg)) => Line::from(vec![
            Span::raw(" "),
            Span::styled(format!("Error: {}", msg), Style::default().fg(Color::Red)),
        ]),
        (AppState::Loading, _) => {
            let frame = SPINNER_FRAMES[state.spinner_frame % SPINNER_FRAMES.len()];
            Line::from(vec![
                Span::raw(" "),
                Span::styled(frame.to_string(), Style::default().fg(Color::Cyan)),
                Span::raw("  Loading comments…"),
            ])
        }
        _ => Line::from(vec![
            Span::raw(" "),
            Span::styled(
                format!("{} comments", state.comments.len()),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    };
    f.render_widget(Paragraph::new(line), area);
}

/// Full-panel Help modal — fills the entire area (k9s-style).
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

    f.render_widget(block, area);
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::comment::{
        Comment, CommentAuthor, CommentBody, CommentLinks, CommentVersion, StorageBody,
    };
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn make_comment(author: &str, when: &str, body: &str) -> Comment {
        Comment {
            id: "1".into(),
            title: "Re: page".into(),
            version: Some(CommentVersion {
                number: 1,
                when: Some(when.into()),
                by: Some(CommentAuthor {
                    display_name: Some(author.into()),
                }),
            }),
            body: Some(CommentBody {
                storage: Some(StorageBody {
                    value: Some(body.into()),
                    representation: Some("storage".into()),
                }),
            }),
            links: CommentLinks::default(),
        }
    }

    fn dump(terminal: &Terminal<TestBackend>) -> String {
        let backend = terminal.backend();
        let buf = backend.buffer();
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn render_comments_does_not_panic_on_loading_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::new("12345".into());
        terminal
            .draw(|f| render_comments(f, &mut state, f.area()))
            .expect("draw");
    }

    #[test]
    fn render_comments_does_not_panic_on_empty_browse_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::with_comments("12345", vec![]);
        terminal
            .draw(|f| render_comments(f, &mut state, f.area()))
            .expect("draw");
    }

    #[test]
    fn render_comments_does_not_panic_with_3_comments() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::with_comments(
            "12345",
            vec![
                make_comment("Alice", "2026-05-01T10:00:00Z", "<p>One</p>"),
                make_comment("Bob", "2026-04-28T09:00:00Z", "<p>Two</p>"),
                make_comment("Carol", "2026-04-10T08:00:00Z", "<p>Three</p>"),
            ],
        );
        terminal
            .draw(|f| render_comments(f, &mut state, f.area()))
            .expect("draw");
    }

    #[test]
    fn render_comments_status_bar_shows_count_in_browse() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::with_comments(
            "12345",
            vec![
                make_comment("Alice", "2026-05-01T10:00:00Z", "<p>One</p>"),
                make_comment("Bob", "2026-04-28T09:00:00Z", "<p>Two</p>"),
                make_comment("Carol", "2026-04-10T08:00:00Z", "<p>Three</p>"),
            ],
        );
        terminal
            .draw(|f| render_comments(f, &mut state, f.area()))
            .expect("draw");
        assert!(dump(&terminal).contains("3 comments"));
    }

    #[test]
    fn render_comments_status_bar_shows_loading_in_loading_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::new("12345".into());
        terminal
            .draw(|f| render_comments(f, &mut state, f.area()))
            .expect("draw");
        assert!(dump(&terminal).contains("Loading comments"));
    }

    #[test]
    fn render_comments_shortcuts_bar_shows_navigate_help_back() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::with_comments(
            "12345",
            vec![make_comment("Alice", "2026-05-01T10:00:00Z", "<p>One</p>")],
        );
        terminal
            .draw(|f| render_comments(f, &mut state, f.area()))
            .expect("draw");
        let d = dump(&terminal);
        for token in &["Top/Bot", "Help", "Back"] {
            assert!(
                d.contains(token),
                "shortcuts bar missing token {:?} in dump:\n{}",
                token,
                d
            );
        }
    }

    #[test]
    fn render_comments_list_pane_uses_middle_dot_separator() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::with_comments(
            "12345",
            vec![make_comment(
                "Alice",
                "2026-05-01T10:00:00Z",
                "<p>This needs updating</p>",
            )],
        );
        terminal
            .draw(|f| render_comments(f, &mut state, f.area()))
            .expect("draw");
        assert!(dump(&terminal).contains("Alice · 2026-05-01"));
    }
}
