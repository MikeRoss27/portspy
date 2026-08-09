//! Kill confirmation overlay: `y` kills, `n` cancels.

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::state::AppState;
use crate::ui::theme::Theme;

/// Renders the kill confirmation prompt over the browse screen.
pub fn overlay(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let theme = Theme::default();
    let Some(listener) = state.kill_target_listener() else {
        return;
    };
    let popup = centered_rect(area, 70, 5);
    let lines = vec![
        Line::from(Span::styled(" Kill process ", theme.danger_text())),
        Line::from(Span::styled(
            format!(
                " {} on :{} ({}, pid {})",
                listener.process.display_name(),
                listener.port,
                listener.application.label,
                listener.pid()
            ),
            Style::default().fg(theme.secondary),
        )),
        Line::from(Span::styled(" y to kill · n to cancel ", theme.footer_hint())),
    ];
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).border_style(theme.danger_text()))
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
