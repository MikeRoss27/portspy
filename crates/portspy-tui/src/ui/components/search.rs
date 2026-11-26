//! Search prompt for the current listener list.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::ui::theme::Theme;

/// Renders the active query inline, leaving the matching rows visible below it.
pub fn render(frame: &mut Frame<'_>, area: Rect, query: &str) {
    if area.is_empty() {
        return;
    }

    let theme = Theme::default();
    let prefix = Span::styled(" / ", theme.accent_text());
    let prompt = if query.is_empty() {
        Span::styled("Port, project or origin", theme.muted_text())
    } else {
        Span::raw(query)
    };
    let cursor = prefix.width() + Line::from(query).width();
    let last_column = area.width - 1;
    let offset = u16::try_from(cursor.saturating_sub(usize::from(last_column))).unwrap_or(u16::MAX);
    let column = cursor.min(usize::from(last_column)) as u16;
    frame.render_widget(
        Paragraph::new(Line::from(vec![prefix, prompt])).scroll((0, offset)),
        area,
    );
    frame.set_cursor_position((area.x.saturating_add(column), area.y));
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::render;

    #[test]
    fn long_queries_keep_the_input_tail_visible_on_narrow_terminals()
    -> Result<(), std::convert::Infallible> {
        let mut terminal = Terminal::new(TestBackend::new(12, 1))?;
        terminal.draw(|frame| render(frame, frame.area(), "a very long atlas-web"))?;

        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("atlas-web"));
        Ok(())
    }
}
