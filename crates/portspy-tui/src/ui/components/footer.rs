//! Footer: context-sensitive key hints and feedback.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::state::{AppMode, AppState};
use crate::ui::theme::Theme;

/// Renders the key hints or most recent action feedback.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let theme = Theme::default();
    if let Some(notification) = &state.notification
        && matches!(state.mode, AppMode::Browse | AppMode::Inspect)
    {
        frame.render_widget(
            Paragraph::new(format!(" {}", notification.message)).style(theme.accent_text()),
            area,
        );
        return;
    }

    let compact = area.width < 64;
    let hints = match state.mode {
        AppMode::Browse if compact => "↵ Inspect  / Find  r Recent  u Owners  x Stop",
        AppMode::Browse => {
            "↑↓ Select  ↵ Inspect  / Find  r Recent  u Owners  x Stop  X Tree  q Quit"
        }
        AppMode::Inspect if compact => "Esc Back  Pg↑↓ Scroll  x Stop  X Tree",
        AppMode::Inspect => "Esc Back  Pg↑↓ Scroll  x Stop  X Stop tree  q Quit",
        AppMode::Search => "↑↓ Select result  ↵ Inspect  Esc Clear",
        AppMode::ConfirmKill => "y Stop  Esc Cancel  ↑↓ Review",
    };
    frame.render_widget(
        Paragraph::new(format!(" {hints}")).style(theme.footer_hint()),
        area,
    );
}
