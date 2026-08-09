//! Search overlay: a centered prompt while typing a filter.

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::state::AppState;
use crate::ui::theme::Theme;

/// Renders the search prompt over the browse screen.
pub fn overlay(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let theme = Theme::default();
    let popup = centered_rect(area, 60, 3);
    let lines = vec![
        Line::from(Span::styled(" Filter listeners ", theme.warning_text())),
        Line::from(Span::styled(
            format!("> {}", state.query),
            Style::default().fg(theme.secondary),
        )),
    ];
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).border_style(theme.warning_text()))
            .style(theme.secondary),
        popup,
    );
}

/// A centered box with a given width percentage and height in lines.
fn centered_rect(area: Rect, width_percent: u16, height: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(100)]).flex(Flex::Center).split(area);
    let horizontal = Layout::horizontal([Constraint::Percentage(width_percent)])
        .flex(Flex::Center)
        .split(vertical[0]);
    Rect {
        x: horizontal[0].x,
        y: vertical[0].y,
        width: horizontal[0].width.min(area.width),
        height,
    }
}
