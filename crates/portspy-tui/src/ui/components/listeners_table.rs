//! The listeners table: the heart of the interface.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Row, Table};
use ratatui::Frame;

use portspy_core::domain::listener::Listener;

use crate::format::{duration, memory};
use crate::state::AppState;
use crate::ui::layout::Breakpoint;
use crate::ui::theme::Theme;

/// Renders the table with the columns fitting the current width.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let theme = Theme::default();
    let breakpoint = Breakpoint::of(area.width);

    let mut widths = vec![
        ratatui::layout::Constraint::Length(6),
        ratatui::layout::Constraint::Percentage(28),
        ratatui::layout::Constraint::Percentage(20),
        ratatui::layout::Constraint::Percentage(24),
    ];
    if breakpoint == Breakpoint::Wide {
        widths.insert(3, ratatui::layout::Constraint::Length(9));
    }

    let header_row = match breakpoint {
        Breakpoint::Wide => Row::new(["PORT", "APP", "ORIGIN", "PID", "PROCESS", "MEM", "UPTIME"])
            .style(theme.table_header()),
        Breakpoint::Medium => Row::new(["PORT", "APP", "ORIGIN", "PROCESS", "MEM", "UPTIME"])
            .style(theme.table_header()),
        Breakpoint::Narrow => Row::new(["PORT", "APP", "PROCESS", "MEM", "UPTIME"])
            .style(theme.table_header()),
    };

    let rows: Vec<Row> = state
        .visible_listeners()
        .iter()
        .map(|listener| {
            Row::new(cells(listener, breakpoint, theme, state.selected == Some(listener.id)))
        })
        .collect();

    let table = Table::new(rows, widths)
        .header(header_row)
        .block(Block::bordered().title(format!(" listeners {}", state.visible_listeners().len() )))
        .row_highlight_style(theme.selected_row());

    frame.render_widget(table, area);
}

/// Builds the cells of one data row.
fn cells<'a>(
    listener: &'a Listener,
    breakpoint: Breakpoint,
    theme: Theme,
    selected: bool,
) -> Vec<Cell<'a>> {
    let base = if selected { theme.selected_row() } else { Style::default() };

    let port = Span::styled(format!("{}", listener.port), base.fg(theme.primary));
    let protocol = listener.protocol.as_str();
    let app = Span::styled(listener.application.label.clone(), base.fg(theme.accent));
    let origin = Span::styled(listener.origin.label.clone(), base.fg(theme.secondary));
    let process = Span::styled(listener.process.display_name().to_owned(), base);
    let memory = Span::styled(memory::memory(listener.process.memory_bytes), base.fg(theme.muted));
    let uptime = Span::styled(
        duration::uptime(listener.process.started_at, std::time::SystemTime::now()),
        base.fg(theme.muted),
    );

    let mut cells = match breakpoint {
        Breakpoint::Wide => vec![
            Cell::from(Line::from(vec![port, Span::styled(format!(" {protocol}"), theme.muted_text())])),
            Cell::from(app),
            Cell::from(origin),
            Cell::from(Span::styled(listener.pid().to_string(), base)),
            Cell::from(process),
            Cell::from(memory),
            Cell::from(uptime),
        ],
        Breakpoint::Medium => vec![
            Cell::from(Line::from(vec![port, Span::styled(format!(" {protocol}"), theme.muted_text())])),
            Cell::from(app),
            Cell::from(origin),
            Cell::from(process),
            Cell::from(memory),
            Cell::from(uptime),
        ],
        Breakpoint::Narrow => vec![
            Cell::from(Line::from(vec![port, Span::styled(format!(" {protocol}"), theme.muted_text())])),
            Cell::from(app),
            Cell::from(process),
            Cell::from(memory),
            Cell::from(uptime),
        ],
    };
    if selected {
        cells.insert(0, Cell::from(Span::styled(">", theme.selected_row())));
    } else {
        cells.insert(0, Cell::from(" "));
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::cells;
    use crate::ui::layout::Breakpoint;
    use crate::ui::theme::Theme;
    use portspy_core::domain::application::{ApplicationInfo, ApplicationKind};
    use portspy_core::domain::confidence::Confidence;
    use portspy_core::domain::listener::{Listener, ListenerId};
    use portspy_core::domain::origin::{OriginInfo, OriginKind};
    use portspy_core::domain::process::{ProcessId, ProcessInfo};
    use portspy_core::domain::protocol::Protocol;
    use std::net::{IpAddr, Ipv4Addr};

    fn sample() -> Listener {
        Listener {
            id: ListenerId::new(Protocol::Tcp, IpAddr::V4(Ipv4Addr::LOCALHOST), 3000, ProcessId(42), None),
            protocol: Protocol::Tcp,
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 3000,
            process: ProcessInfo {
                pid: ProcessId(42),
                parent_pid: Some(ProcessId(1)),
                name: "node.exe".to_owned(),
                executable: None,
                command_line: None,
                working_directory: None,
                started_at: None,
                memory_bytes: Some(1_048_576),
                cpu_percent: None,
                ancestors: Vec::new(),
            },
            application: ApplicationInfo::new(
                ApplicationKind::Node,
                ApplicationKind::Node.label().to_owned(),
                Confidence::Exact,
            ),
            origin: OriginInfo::new(
                OriginKind::Unknown,
                OriginKind::Unknown.label().to_owned(),
                Confidence::Unknown,
                None,
            ),
        }
    }

    #[test]
    fn wide_row_has_seven_cells_plus_marker() {
        let listener = sample();
        let c = cells(&listener, Breakpoint::Wide, Theme::default(), true);
        assert_eq!(c.len(), 8);
    }

    #[test]
    fn narrow_row_has_five_cells_plus_marker() {
        let listener = sample();
        let c = cells(&listener, Breakpoint::Narrow, Theme::default(), false);
        assert_eq!(c.len(), 6);
    }
}
