//! Header: active view, search filter and sort.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::state::{AppMode, AppState, ScanStatus};
use crate::ui::theme::Theme;

/// Renders the brand, active view and filter summary.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    use portspy_core::snapshot::query::SortField;

    let theme = Theme::default();
    frame.render_widget(Block::default().style(theme.controls()), area);
    let recent = state.recent_count();
    let total = state.snapshot.listeners.len();
    let visible = state.visible_listeners().len();
    let (view, scope) = if state.recent_only() {
        ("Recently opened", recent)
    } else {
        ("All listeners", total)
    };

    let mut headline = Vec::new();
    let text_area = if area.width >= 72 && area.height >= 3 {
        let [mark, text] =
            Layout::horizontal([Constraint::Length(9), Constraint::Min(0)]).areas(area);
        frame.render_widget(
            Paragraph::new(" ▄▄ ▀█▄\n      ██\n ▀▀ ▄█▀").style(Theme::brand_mark()),
            mark,
        );
        text
    } else {
        headline.push(Span::styled(" :>", Theme::brand_mark()));
        area
    };
    headline.extend([
        Span::styled(" PortSpy  ", theme.header()),
        Span::styled(view, theme.table_header()),
    ]);
    match state.scan_status {
        ScanStatus::Stale(_) => headline.push(Span::styled(
            "  △ Scan failed · showing last scan",
            theme.warning_text(),
        )),
        ScanStatus::Current if text_area.width >= 50 => {
            headline.push(Span::styled("  ● Live", theme.success_text()));
        }
        _ => {}
    }
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
        format!(" {visible}/{scope} shown"),
        theme.muted_text(),
    )];
    let protocol = state.protocol_filter().map(|protocol| match protocol {
        portspy_core::domain::protocol::Protocol::Tcp => "TCP only",
        portspy_core::domain::protocol::Protocol::Udp => "UDP only",
    });
    context.extend([
        Span::styled("  ·  ", theme.muted_text()),
        Span::styled(
            protocol.unwrap_or("TCP + UDP"),
            if protocol.is_some() {
                theme.key()
            } else {
                theme.muted_text()
            },
        ),
    ]);
    if text_area.width >= 42 {
        context.push(Span::styled(
            format!("  ·  {sort} {direction}"),
            theme.muted_text(),
        ));
    }
    let unknown = state.unknown_owner_count();
    if state.known_owners_only() {
        context.push(Span::styled(
            format!("  ·  PIDs only ({unknown} hidden)"),
            theme.accent_text(),
        ));
    } else if unknown > 0 {
        context.push(Span::styled(
            format!("  ·  {unknown} without PID"),
            theme.muted_text(),
        ));
    }
    if state.mode != AppMode::Search && !state.query.is_empty() {
        context.push(Span::styled(
            format!("  ·  filter: {}", state.query),
            theme.accent_text(),
        ));
    }

    frame.render_widget(
        Paragraph::new(vec![Line::from(headline), Line::from(context)]),
        text_area,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, style::Color};

    use super::*;

    #[test]
    fn compact_header_keeps_the_ascii_mark_name_and_view_visible()
    -> Result<(), std::convert::Infallible> {
        let mut terminal = Terminal::new(TestBackend::new(32, 2))?;
        terminal.draw(|frame| render(frame, frame.area(), &AppState::new()))?;
        let buffer = terminal.backend().buffer();
        let headline: String = buffer.content()[..32]
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(headline.contains(":> PortSpy  All listeners"));
        assert_eq!(buffer[(1, 0)].fg, Color::Rgb(80, 240, 195));
        assert_eq!(buffer[(1, 0)].bg, Color::Rgb(32, 49, 60));
        Ok(())
    }

    #[test]
    fn wide_header_displays_the_three_line_mark_alongside_the_status()
    -> Result<(), std::convert::Infallible> {
        let mut terminal = Terminal::new(TestBackend::new(80, 3))?;
        terminal.draw(|frame| render(frame, frame.area(), &AppState::new()))?;
        let rows: Vec<String> = terminal
            .backend()
            .buffer()
            .content()
            .as_chunks::<80>()
            .0
            .iter()
            .map(|row| row.iter().map(|cell| cell.symbol()).collect())
            .collect();
        assert!(rows[0].starts_with(" ▄▄ ▀█▄   PortSpy  All listeners"));
        assert!(rows[1].starts_with("      ██  0/0 shown"));
        assert!(rows[2].starts_with(" ▀▀ ▄█▀"));
        Ok(())
    }
}
