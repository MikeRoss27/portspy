//! Header: brand, mode badge, counters and current sort.

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::format::{duration, memory};
use crate::state::{AppMode, AppState};
use crate::ui::theme::Theme;

/// Renders the two-line header.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let theme = Theme::default();
    let total = state.snapshot.listeners.len();
    let visible = state.visible_listeners().len();
    let (sum_memory, sum_uptime): (u64, Option<std::time::SystemTime>) =
        state.snapshot.listeners.iter().fold((0_u64, None), |(memory, uptime), listener| {
            let memory = memory + listener.process.memory_bytes.unwrap_or(0);
            let uptime = match (uptime, listener.process.started_at) {
                (Some(existing), Some(next)) => Some(existing.min(next)),
                (Some(existing), None) => Some(existing),
                (None, Some(next)) => Some(next),
                (None, None) => None,
            };
            (memory, uptime)
        });

    let mode_label = match state.mode {
        AppMode::Browse => "BROWSE",
        AppMode::Inspect => "INSPECT",
        AppMode::Search => "SEARCH",
        AppMode::ConfirmKill => "CONFIRM",
    };
    let mode_color = match state.mode {
        AppMode::Browse => theme.primary,
        AppMode::Inspect => theme.accent,
        AppMode::Search => theme.warning,
        AppMode::ConfirmKill => theme.danger,
    };

    let sort_label = match state.sort {
        portspy_core::snapshot::query::SortField::Port => "port",
        portspy_core::snapshot::query::SortField::Application => "app",
        portspy_core::snapshot::query::SortField::Origin => "origin",
        portspy_core::snapshot::query::SortField::Process => "process",
        portspy_core::snapshot::query::SortField::Memory => "memory",
        portspy_core::snapshot::query::SortField::Uptime => "uptime",
    };
    let sort_arrow = if state.order.is_ascending() { "↑" } else { "↓" };

    let mut lines = vec![Line::from(vec![
        Span::styled(" portspy ", theme.header()),
        Span::styled(format!(" {mode_label} "), theme.header().fg(mode_color)),
        Span::styled(
            format!(" {visible} listening · {total} total "),
            theme.muted_text(),
        ),
    ])];

    let mut right = vec![Span::styled(format!(" sort {sort_label} {sort_arrow}"), theme.footer_hint())];
    if !state.query.is_empty() {
        right.push(Span::styled(format!(" · filter \"{}\"", state.query), theme.warning_text()));
    }
    lines.push(Line::from(right));

    let total_memory = memory::memory(Some(sum_memory));
    let total_uptime = duration::uptime(sum_uptime, std::time::SystemTime::now());
    lines[1].spans.insert(
        0,
        Span::styled(
            format!(" {total_memory} · {total_uptime} "),
            theme.muted_text(),
        ),
    );

    let title = format!(" {total} ports ");
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                ratatui::widgets::Block::bordered()
                    .title(Span::styled(title, theme.header().add_modifier(Modifier::BOLD))),
            )
            .style(theme.secondary),
        area,
    );
}
