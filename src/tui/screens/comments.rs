//! CommentsBrowse render — D-48..D-50, CMNT-01.
//!
//! Pure render: no I/O, no state mutation beyond ListState (required by
//! render_stateful_widget). Mirrors src/tui/screens/pages.rs structure.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, HighlightSpacing, List, ListItem,
    Padding, Paragraph, Wrap,
};

use crate::api::comment::Comment;
use crate::output::strip_storage_xml;
use crate::tui::app::{AppState, CommentsBrowseState, SPINNER_FRAMES};

/// Top-level render — vertical 4-row split (title, body, status, help).
pub fn render_comments(f: &mut Frame, state: &mut CommentsBrowseState, area: Rect) {
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
}

fn render_title(f: &mut Frame, state: &CommentsBrowseState, area: Rect) {
    let block = Block::default().borders(Borders::BOTTOM).border_type(BorderType::Plain);
    let inner = block.inner(area);
    f.render_widget(block, area);
    // " Comments — {PAGE-ID}                         {N} total"
    let count_text = match state.browse_state {
        AppState::Loading => "Loading…".to_string(),
        _ => format!("{} total", state.comments.len()),
    };
    let total_w = inner.width as usize;
    let left = format!(" Comments — {}", state.page_id);
    let pad = total_w.saturating_sub(left.chars().count() + count_text.chars().count() + 1);
    let line = Line::from(vec![
        Span::styled(left, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" ".repeat(pad)),
        Span::styled(count_text, Style::default().add_modifier(Modifier::DIM)),
        Span::raw(" "),
    ]);
    f.render_widget(Paragraph::new(line), inner);
}

fn render_body(f: &mut Frame, state: &mut CommentsBrowseState, area: Rect) {
    let horizontal = Layout::horizontal([
        Constraint::Percentage(40),
        Constraint::Percentage(60),
    ]);
    let [list_area, preview_area] = horizontal.areas(area);
    render_list_pane(f, state, list_area);
    render_preview_pane(f, state, preview_area);
}

fn render_list_pane(f: &mut Frame, state: &mut CommentsBrowseState, area: Rect) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_type(BorderType::Plain)
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

    let items: Vec<ListItem> = state.comments.iter().map(|c| {
        let label = format_list_row(c, inner_w.saturating_sub(2));
        ListItem::new(label)
    }).collect();

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

/// Build "{author} · {date} · {first_line}" truncated to `max_chars` with "…".
fn format_list_row(c: &Comment, max_chars: usize) -> String {
    let author = c.version.as_ref()
        .and_then(|v| v.by.as_ref())
        .and_then(|a| a.display_name.as_deref())
        .unwrap_or("—");
    let date = c.version.as_ref()
        .and_then(|v| v.when.as_deref())
        .map(|w| w.get(..10).unwrap_or(w))
        .unwrap_or("—");
    let body = c.body.as_ref()
        .and_then(|b| b.storage.as_ref())
        .and_then(|s| s.value.as_deref())
        .unwrap_or("");
    let stripped = strip_storage_xml(body);
    let first_line = stripped.lines().next().unwrap_or("").to_string();
    let raw = format!("{} · {} · {}", author, date, first_line);
    truncate_with_ellipsis(&raw, max_chars)
}

fn truncate_with_ellipsis(s: &str, max: usize) -> String {
    if max == 0 { return String::new(); }
    let count = s.chars().count();
    if count <= max { return s.to_string(); }
    let take = max.saturating_sub(1);
    let mut out: String = s.chars().take(take).collect();
    out.push('…');
    out
}

fn render_preview_pane(f: &mut Frame, state: &CommentsBrowseState, area: Rect) {
    let block = Block::default().borders(Borders::NONE).padding(Padding::new(1, 1, 0, 0));
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

    let bold = Style::default().add_modifier(Modifier::BOLD);
    let dim_dash = Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM);

    let author = c.version.as_ref()
        .and_then(|v| v.by.as_ref())
        .and_then(|a| a.display_name.clone());
    let date = c.version.as_ref()
        .and_then(|v| v.when.as_deref())
        .map(|w| w.get(..10).unwrap_or(w).to_string());
    let body_xml = c.body.as_ref()
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
    lines.push(field("Author:", author.as_deref(), bold, dim_dash));
    lines.push(field("Date:", date.as_deref(), bold, dim_dash));
    lines.push(Line::from(""));
    for body_line in body_text.lines() {
        lines.push(Line::from(body_line.to_string()));
    }
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(block);
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

fn render_help_bar(f: &mut Frame, area: Rect) {
    let key = |k: &'static str| Span::styled(
        k, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let line = Line::from(vec![
        Span::raw(" "),
        key("o"),
        Span::raw("  open   "),
        key("?"),
        Span::raw("  help   "),
        key("Esc"),
        Span::raw("  back"),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use crate::api::comment::{
        Comment, CommentAuthor, CommentBody, CommentLinks, CommentVersion, StorageBody,
    };

    fn make_comment(author: &str, when: &str, body: &str) -> Comment {
        Comment {
            id: "1".into(),
            title: "Re: page".into(),
            version: Some(CommentVersion {
                number: 1,
                when: Some(when.into()),
                by: Some(CommentAuthor { display_name: Some(author.into()) }),
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
        terminal.draw(|f| render_comments(f, &mut state, f.area())).expect("draw");
    }

    #[test]
    fn render_comments_does_not_panic_on_empty_browse_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::with_comments("12345", vec![]);
        terminal.draw(|f| render_comments(f, &mut state, f.area())).expect("draw");
    }

    #[test]
    fn render_comments_does_not_panic_with_3_comments() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::with_comments("12345", vec![
            make_comment("Alice", "2026-05-01T10:00:00Z", "<p>One</p>"),
            make_comment("Bob",   "2026-04-28T09:00:00Z", "<p>Two</p>"),
            make_comment("Carol", "2026-04-10T08:00:00Z", "<p>Three</p>"),
        ]);
        terminal.draw(|f| render_comments(f, &mut state, f.area())).expect("draw");
    }

    #[test]
    fn render_comments_status_bar_shows_count_in_browse() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::with_comments("12345", vec![
            make_comment("Alice", "2026-05-01T10:00:00Z", "<p>One</p>"),
            make_comment("Bob",   "2026-04-28T09:00:00Z", "<p>Two</p>"),
            make_comment("Carol", "2026-04-10T08:00:00Z", "<p>Three</p>"),
        ]);
        terminal.draw(|f| render_comments(f, &mut state, f.area())).expect("draw");
        assert!(dump(&terminal).contains("3 comments"));
    }

    #[test]
    fn render_comments_status_bar_shows_loading_in_loading_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::new("12345".into());
        terminal.draw(|f| render_comments(f, &mut state, f.area())).expect("draw");
        assert!(dump(&terminal).contains("Loading comments"));
    }

    #[test]
    fn render_comments_help_bar_shows_open_help_back() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::with_comments("12345", vec![
            make_comment("Alice", "2026-05-01T10:00:00Z", "<p>One</p>"),
        ]);
        terminal.draw(|f| render_comments(f, &mut state, f.area())).expect("draw");
        let d = dump(&terminal);
        for token in &["open", "help", "back"] {
            assert!(d.contains(token), "help bar missing token {:?} in dump:\n{}", token, d);
        }
    }

    #[test]
    fn render_comments_list_pane_uses_middle_dot_separator() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = CommentsBrowseState::with_comments("12345", vec![
            make_comment("Alice", "2026-05-01T10:00:00Z", "<p>This needs updating</p>"),
        ]);
        terminal.draw(|f| render_comments(f, &mut state, f.area())).expect("draw");
        assert!(dump(&terminal).contains("Alice · 2026-05-01"));
    }
}
