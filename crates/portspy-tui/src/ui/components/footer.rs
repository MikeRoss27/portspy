//! Footer: context-sensitive key hints and feedback.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::state::{AppMode, AppState};
use crate::ui::layout::{browse_layout, main_layout};
use crate::ui::theme::Theme;

/// Renders the key hints or most recent action feedback.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let theme = Theme::default();
    if matches!(state.mode, AppMode::Browse | AppMode::Inspect) {
        let feedback = if state.stop_in_progress() {
            Some("Stop in progress")
        } else {
            state
                .notification
                .as_ref()
                .map(|notification| notification.message.as_str())
        };
        if let Some(feedback) = feedback {
            frame.render_widget(
                Paragraph::new(format!(" ◇ {feedback}")).style(theme.accent_text()),
                area,
            );
            return;
        }
        if let Some(error) = state.scan_status.error_message() {
            frame.render_widget(
                Paragraph::new(format!(" △ {error}")).style(theme.warning_text()),
                area,
            );
            return;
        }
    }

    let hints: &[(&str, &str)] = match state.mode {
        AppMode::Browse => &[
            ("Enter", "Inspect"),
            ("/", "Find"),
            ("↑↓", "Move"),
            ("t", "TCP/UDP"),
            ("x", "Stop"),
            ("X", "Stop tree"),
            ("r", "Recent"),
            ("u", "PIDs"),
            ("s/S", "Sort"),
            ("Tab", "Preview"),
            ("q", "Quit"),
        ],
        AppMode::Inspect => &[
            ("Esc", "Back"),
            ("Pg↑↓", "Scroll"),
            ("x", "Stop"),
            ("X", "Stop tree"),
            ("q", "Quit"),
        ],
        AppMode::Search => &[("↑↓", "Select"), ("Enter", "Inspect"), ("Esc", "Clear")],
        AppMode::ConfirmKill => &[("y", "Stop"), ("Esc", "Cancel"), ("↑↓", "Review")],
    };

    let mut lines = Vec::new();
    let mut line = Line::default();
    let mut used = 0;
    for &(key, label) in hints {
        if key == "Tab" && browse_layout(main_layout(frame.area()).1, true).1.is_none() {
            continue;
        }
        let shortcut = Span::styled(format!(" {key} "), theme.keycap());
        let action = Span::styled(format!(" {label}  "), theme.footer_hint());
        let width = shortcut.width() + action.width();
        if width > usize::from(area.width) {
            continue;
        }
        if used + width > usize::from(area.width) && lines.len() + 1 < usize::from(area.height) {
            lines.push(std::mem::take(&mut line));
            used = 0;
        }
        if used + width > usize::from(area.width) {
            continue;
        }
        used += width;
        line.push_span(shortcut);
        line.push_span(action);
    }
    lines.push(line);
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn second_row_exposes_distinct_process_and_tree_actions() -> Result<(), std::convert::Infallible>
    {
        let mut terminal = Terminal::new(TestBackend::new(72, 2))?;
        terminal.draw(|frame| render(frame, frame.area(), &AppState::new()))?;
        let rows: Vec<String> = terminal
            .backend()
            .buffer()
            .content()
            .chunks(72)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect())
            .collect();
        assert!(rows[0].contains("x  Stop"));
        assert!(rows.iter().any(|row| row.contains("X  Stop tree")));
        assert!(rows[1].contains("Quit"));
        Ok(())
    }

    #[test]
    fn compact_footer_displays_only_complete_shortcuts() -> Result<(), std::convert::Infallible> {
        let mut terminal = Terminal::new(TestBackend::new(32, 1))?;
        terminal.draw(|frame| render(frame, frame.area(), &AppState::new()))?;
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert_eq!(text.trim_end(), " Enter  Inspect   /  Find");
        Ok(())
    }
}
