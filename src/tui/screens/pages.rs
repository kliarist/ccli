// RED phase — stub module with tests only.
// Implementation will be added in GREEN phase.

use crate::tui::app::PagesBrowseState;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Placeholder that will be replaced in GREEN phase.
pub fn render_pages(_f: &mut Frame, _state: &mut PagesBrowseState, _area: Rect) {
    unimplemented!("render_pages not yet implemented — RED phase stub")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use crate::api::page::{ContentType as ApiContentType, Page, PageLinks};
    use crate::tui::app::{AppState, StatusMessage, StatusStyle};

    fn make_page(id: &str, title: &str) -> Page {
        Page {
            id: id.to_string(),
            title: title.to_string(),
            content_type: "page".to_string(),
            version: None,
            ancestors: None,
            links: PageLinks { webui: Some(format!("/x/{}", id)), self_url: None },
        }
    }

    #[test]
    fn render_pages_does_not_panic_on_loading_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::new("DEV".to_string(), ApiContentType::Page);
        terminal.draw(|f| render_pages(f, &mut state, f.area())).expect("draw");
    }

    #[test]
    fn render_pages_does_not_panic_on_browse_state_with_pages() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV", ApiContentType::Page,
            vec![make_page("1", "Alpha"), make_page("2", "Beta")],
        );
        terminal.draw(|f| render_pages(f, &mut state, f.area())).expect("draw");
    }

    #[test]
    fn render_pages_does_not_panic_on_filter_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV", ApiContentType::Page, vec![make_page("1", "Alpha")],
        );
        state.browse_state = AppState::Filter { query: "alp".to_string() };
        state.apply_filter("alp");
        terminal.draw(|f| render_pages(f, &mut state, f.area())).expect("draw");
    }

    #[test]
    fn render_pages_does_not_panic_on_modal_state() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV", ApiContentType::Page, vec![make_page("1", "Alpha")],
        );
        state.browse_state = AppState::Modal;
        terminal.draw(|f| render_pages(f, &mut state, f.area())).expect("draw");
    }

    #[test]
    fn render_pages_blog_posts_title_uses_blog_posts_label() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV", ApiContentType::BlogPost, vec![make_page("1", "Post")],
        );
        terminal.draw(|f| render_pages(f, &mut state, f.area())).expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let dump: String = (0..buffer.area.height)
            .map(|y| (0..buffer.area.width)
                .map(|x| buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" ").to_string())
                .collect::<String>())
            .collect::<Vec<_>>().join("\n");
        assert!(dump.contains("Blog Posts"), "title bar must say 'Blog Posts' for BlogPost content_type. Got:\n{}", dump);
    }

    #[test]
    fn render_pages_help_bar_shows_required_bindings() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV", ApiContentType::Page, vec![make_page("1", "Alpha")],
        );
        terminal.draw(|f| render_pages(f, &mut state, f.area())).expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let dump: String = (0..buffer.area.height)
            .map(|y| (0..buffer.area.width)
                .map(|x| buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" ").to_string())
                .collect::<String>())
            .collect::<Vec<_>>().join("\n");
        for token in &["filter", "open", "edit", "help", "back"] {
            assert!(dump.contains(token), "help bar must contain '{}'\nGot:\n{}", token, dump);
        }
    }

    #[test]
    fn render_pages_status_message_renders_when_set() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("term");
        let mut state = PagesBrowseState::with_pages(
            "DEV", ApiContentType::Page, vec![make_page("1", "Alpha")],
        );
        state.status_message = Some(StatusMessage {
            text: "Saved.".to_string(),
            style: StatusStyle::Success,
        });
        terminal.draw(|f| render_pages(f, &mut state, f.area())).expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let dump: String = (0..buffer.area.height)
            .map(|y| (0..buffer.area.width)
                .map(|x| buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" ").to_string())
                .collect::<String>())
            .collect::<Vec<_>>().join("\n");
        assert!(dump.contains("Saved."), "status message must render. Got:\n{}", dump);
    }
}
