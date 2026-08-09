//! Details panel: the right half of the Inspect screen.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::format::{address, duration, memory};
use crate::state::AppState;
use crate::ui::components::process_tree;
use crate::ui::theme::Theme;

/// Renders the details of the selected listener.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let theme = Theme::default();
    let Some(listener) = state.selected_listener() else {
        frame.render_widget(
            Paragraph::new("No selection")
                .block(Block::default().borders(Borders::ALL).title(" details "))
                .style(theme.muted_text()),
            area,
        );
        return;
    };

    let confidence = match listener.application.confidence {
        portspy_core::domain::confidence::Confidence::Exact => "exact",
        portspy_core::domain::confidence::Confidence::High => "high",
        portspy_core::domain::confidence::Confidence::Medium => "medium",
        portspy_core::domain::confidence::Confidence::Low => "low",
        portspy_core::domain::confidence::Confidence::Unknown => "unknown",
    };

    let mut lines = vec![
        Line::from(Span::styled(
            format!(" {} ", listener.application.label),
            theme.accent_text(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" listener ", theme.footer_hint()),
            Span::styled(format!(":{} ({})", listener.port, listener.protocol), theme.secondary),
        ]),
        Line::from(vec![
            Span::styled(" address  ", theme.footer_hint()),
            Span::styled(address::full(listener.address, listener.port), theme.secondary),
        ]),
        Line::from(vec![
            Span::styled(" process  ", theme.footer_hint()),
            Span::styled(listener.process.display_name().to_owned(), theme.secondary),
        ]),
        Line::from(vec![
            Span::styled(" pid      ", theme.footer_hint()),
            Span::styled(listener.pid().to_string(), theme.secondary),
        ]),
        Line::from(vec![
            Span::styled(" memory   ", theme.footer_hint()),
            Span::styled(memory::memory(listener.process.memory_bytes), theme.secondary),
        ]),
        Line::from(vec![
            Span::styled(" uptime   ", theme.footer_hint()),
            Span::styled(
                duration::uptime(listener.process.started_at, std::time::SystemTime::now()),
                theme.secondary,
            ),
        ]),
        Line::from(vec![
            Span::styled(" command  ", theme.footer_hint()),
            Span::styled(
                listener.process.command_line.as_deref().unwrap_or("unknown"),
                theme.secondary,
            ),
        ]),
        Line::from(vec![
            Span::styled(" cwd      ", theme.footer_hint()),
            Span::styled(
                listener
                    .process
                    .working_directory
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "unknown".to_owned()),
                theme.secondary,
            ),
        ]),
        Line::from(vec![
            Span::styled(" detected ", theme.footer_hint()),
            Span::styled(
                format!("{} ({confidence})", listener.application.label),
                theme.warning_text(),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(" process chain ", theme.table_header())),
    ];
    lines.extend(process_tree::lines(listener, theme));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            " origin: {} ({})",
            listener.origin.label,
            match listener.origin.confidence {
                portspy_core::domain::confidence::Confidence::Exact => "exact",
                portspy_core::domain::confidence::Confidence::High => "high",
                portspy_core::domain::confidence::Confidence::Medium => "medium",
                portspy_core::domain::confidence::Confidence::Low => "low",
                portspy_core::domain::confidence::Confidence::Unknown => "unknown",
            }
        ),
        theme.muted_text(),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" details "))
            .wrap(Wrap { trim: false })
            .style(Style::default()),
        area,
    );
}
