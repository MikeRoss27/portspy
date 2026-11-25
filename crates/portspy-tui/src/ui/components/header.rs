//! Header: active view, search filter and sort.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::state::{AppMode, AppState};
use crate::ui::theme::Theme;

/// Renders the two-line header.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    use portspy_core::snapshot::query::SortField;

    let theme = Theme::default();
    let recent = state.recent_count();
    let total = state.snapshot.listeners.len();
    let visible = state.visible_listeners().len();
    let (view, scope) = if state.recent_only() {
        ("Recently opened", recent)
    } else {
        ("All listeners", total)
    };

    let mut headline = vec![
        Span::styled(" portspy  ", theme.header()),
        Span::styled(view, theme.table_header()),
    ];
    if !state.recent_only() && recent > 0 {
        headline.push(Span::styled(
            format!("  ·  {recent} new"),
            theme.accent_text(),
        ));
    }
    if state.mode == AppMode::Inspect {
        headline.push(Span::styled("  ·  Inspect", theme.accent_text()));
    }

    let sort = match state.sort {
        SortField::Port => "port",
        SortField::Application => "app",
        SortField::Origin => "origin",
        SortField::Process => "process",
        SortField::Memory => "memory",
        SortField::Uptime => "uptime",
    };
    let direction = if state.order.is_ascending() {
        "↑"
    } else {
        "↓"
    };

    let mut context = vec![Span::styled(
        format!(" {visible}/{scope} shown  ·  {sort} {direction}"),
        theme.muted_text(),
    )];
    let unknown = state.unknown_owner_count();
    if state.known_owners_only() {
        context.push(Span::styled(
            format!("  ·  Known owners ({unknown} hidden)"),
            theme.accent_text(),
        ));
    } else if unknown > 0 {
        context.push(Span::styled(
            format!("  ·  {unknown} without PID"),
            theme.muted_text(),
        ));
    }
    if !state.query.is_empty() {
        context.push(Span::styled(
            format!("  ·  filter: {}", state.query),
            theme.accent_text(),
        ));
    }

    frame.render_widget(
        Paragraph::new(vec![Line::from(headline), Line::from(context)]),
        area,
    );
}
