//! Footer: one-line context-sensitive key hints.

use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::state::{AppMode, AppState};
use crate::ui::theme::Theme;

/// Renders the single-line footer with hints for the current mode.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let theme = Theme::default();
    let hints = match state.mode {
        AppMode::Browse => "↑/↓ move · j/k vim · Enter inspect · / search · r sort · o order · x kill · q quit",
        AppMode::Inspect => "←/Esc back · q quit",
        AppMode::Search => "type to filter · ← backspace · Esc clear",
        AppMode::ConfirmKill => "y confirm kill · n cancel",
    };
    let line = Line::from(vec![Span::styled(format!(" {hints} "), theme.footer_hint())]);
    frame.render_widget(Paragraph::new(line), area);
}
