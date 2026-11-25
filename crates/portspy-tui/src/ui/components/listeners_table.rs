//! The listeners table: the heart of the interface.

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Cell, HighlightSpacing, Paragraph, Row, Table};

use portspy_core::domain::listener::Listener;
use portspy_core::ports::Exposure;

use crate::format::{address, duration, memory};
use crate::state::AppState;
use crate::ui::layout::Breakpoint;
use crate::ui::theme::Theme;

/// Renders the current listener view with columns that fit the terminal.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &mut AppState) {
    let theme = Theme::default();
    let mut table_state = std::mem::take(&mut state.table_state);
    let visible = state.visible_listeners();
    let block = Block::bordered().title(format!(" listeners {} ", visible.len()));
    if visible.is_empty() {
        let message = if state.recent_only() && state.recent_count() == 0 {
            "No new listeners in this session."
        } else if !state.query.trim().is_empty() {
            "No listeners match this search."
        } else if state.known_owners_only() && state.unknown_owner_count() > 0 {
            "No identified owners in this view. Press u to show all listeners."
        } else {
            "No TCP or UDP listeners found."
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(theme.muted_text())
                .wrap(ratatui::widgets::Wrap { trim: true })
                .block(block),
            area,
        );
        state.table_state = table_state;
        return;
    }

    let breakpoint = Breakpoint::of(area.width);
    let (header, widths) = columns(breakpoint);
    let rows = visible.iter().map(|listener| {
        Row::new(cells(listener, breakpoint, theme)).height(if breakpoint == Breakpoint::Narrow {
            2
        } else {
            1
        })
    });
    let table = Table::new(rows, widths)
        .header(Row::new(header.iter().copied()).style(theme.table_header()))
        .block(block)
        .highlight_symbol("› ")
        .highlight_spacing(HighlightSpacing::Always)
        .row_highlight_style(theme.selected_row());

    frame.render_stateful_widget(table, area, &mut table_state);
    state.table_state = table_state;
}

fn columns(breakpoint: Breakpoint) -> (&'static [&'static str], &'static [Constraint]) {
    use Constraint::{Fill, Length};
    match breakpoint {
        Breakpoint::Full => (
            &[
                "PORT", "APP", "PROJECT", "ORIGIN", "BINDING", "ADDRESS", "PID", "PROCESS", "MEM",
                "AGE",
            ],
            &[
                Length(10),
                Fill(1),
                Fill(1),
                Length(17),
                Length(14),
                Constraint::Percentage(16),
                Length(9),
                Fill(1),
                Length(8),
                Length(8),
            ],
        ),
        Breakpoint::Wide => (
            &["PORT", "APP", "PROJECT", "ORIGIN", "BINDING", "MEM", "AGE"],
            &[
                Length(10),
                Fill(1),
                Fill(1),
                Length(17),
                Length(14),
                Length(8),
                Length(8),
            ],
        ),
        Breakpoint::Medium => (
            &["PORT", "APP", "PROJECT", "ORIGIN", "BINDING"],
            &[Length(10), Fill(1), Fill(1), Length(14), Length(14)],
        ),
        Breakpoint::Narrow => (
            &["PORT / BINDING", "APP / PROJECT / ORIGIN"],
            &[Length(14), Fill(1)],
        ),
    }
}

fn cells(listener: &Listener, breakpoint: Breakpoint, theme: Theme) -> Vec<Cell<'_>> {
    let port = Line::from(vec![
        Span::styled(format!(":{}", listener.port), theme.accent_text()),
        Span::raw(format!(
            " {}",
            listener.protocol.as_str().to_ascii_uppercase()
        )),
    ]);
    let scope_style = match Exposure::of(listener.address) {
        Exposure::Loopback => theme.muted_text(),
        Exposure::Interface | Exposure::AllInterfaces => theme.warning_text(),
    };
    let scope = Span::styled(address::scope(listener.address), scope_style);
    let known_owner = listener.pid().as_u32() != 0;
    let app = Span::raw(if known_owner {
        listener.application.label.as_str()
    } else {
        "Unknown owner"
    });
    let project = listener.project.name().unwrap_or("—");
    let mut origin = if known_owner {
        Line::from(listener.origin.label.as_str())
    } else {
        Line::from(Span::styled("—", theme.muted_text()))
    };
    if listener.is_duplicate {
        origin.push_span(Span::styled(" dup", theme.warning_text()));
    }
    if listener.is_orphan {
        origin.push_span(Span::styled(" orphan", theme.warning_text()));
    }
    if breakpoint == Breakpoint::Narrow {
        return vec![
            Cell::from(Text::from(vec![port, Line::from(scope)])),
            Cell::from(Text::from(vec![
                Line::from(vec![
                    app,
                    Span::styled(" · ", theme.muted_text()),
                    Span::raw(project),
                ]),
                origin,
            ])),
        ];
    }

    let mut cells = vec![
        Cell::from(port),
        Cell::from(app),
        Cell::from(project),
        Cell::from(origin),
        Cell::from(scope),
    ];
    if breakpoint == Breakpoint::Full {
        cells.push(Cell::from(listener.address.to_string()));
        if known_owner {
            cells.push(Cell::from(listener.pid().to_string()));
            cells.push(Cell::from(listener.process.display_name()));
        } else {
            cells.extend([Cell::from("—"), Cell::from("—")]);
        }
    }
    if matches!(breakpoint, Breakpoint::Wide | Breakpoint::Full) {
        cells.push(Cell::from(memory::memory(listener.process.memory_bytes)));
        cells.push(Cell::from(duration::uptime(
            listener.process.started_at,
            std::time::SystemTime::now(),
        )));
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::{cells, columns};
    use crate::input::actions::Action;
    use crate::state::{AppMode, AppState};
    use crate::ui;
    use crate::ui::layout::Breakpoint;
    use crate::ui::theme::Theme;
    use portspy_core::domain::application::{ApplicationInfo, ApplicationKind};
    use portspy_core::domain::confidence::Confidence;
    use portspy_core::domain::listener::{Listener, ListenerId};
    use portspy_core::domain::origin::{OriginInfo, OriginKind};
    use portspy_core::domain::process::{ProcessId, ProcessInfo};
    use portspy_core::domain::project::{ProjectInfo, ProjectKind};
    use portspy_core::domain::protocol::Protocol;
    use ratatui::{Terminal, backend::TestBackend};
    use std::net::{IpAddr, Ipv4Addr, TcpListener};

    fn listener(port: u16, pid: ProcessId, origin: OriginKind) -> Listener {
        let address = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        Listener {
            id: ListenerId::new(Protocol::Tcp, address, port, pid, None),
            protocol: Protocol::Tcp,
            address,
            port,
            process: ProcessInfo {
                pid,
                parent_pid: None,
                name: "node".to_owned(),
                executable: None,
                command_line: None,
                working_directory: None,
                started_at: None,
                memory_bytes: Some(1_048_576),
                cpu_percent: None,
                ancestors: Vec::new(),
            },
            application: ApplicationInfo::new(
                ApplicationKind::NextJs,
                ApplicationKind::NextJs.label().to_owned(),
                Confidence::Exact,
            ),
            origin: OriginInfo::new(origin, origin.label().to_owned(), Confidence::High, None),
            project: ProjectInfo::new(ProjectKind::Named("atlas-web".to_owned()), Confidence::High),
            is_duplicate: false,
            is_orphan: false,
        }
    }

    #[test]
    fn row_matches_its_columns_at_every_width() -> std::io::Result<()> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let listener = listener(
            socket.local_addr()?.port(),
            ProcessId(42),
            OriginKind::Codex,
        );
        for breakpoint in [
            Breakpoint::Narrow,
            Breakpoint::Medium,
            Breakpoint::Wide,
            Breakpoint::Full,
        ] {
            let (header, widths) = columns(breakpoint);
            assert_eq!(header.len(), widths.len());
            assert_eq!(
                header.len(),
                cells(&listener, breakpoint, Theme::default()).len()
            );
        }
        Ok(())
    }

    fn rendered(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn inline_port_search_and_compact_inspection_keep_the_same_target()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = TcpListener::bind("127.0.0.1:0")?;
        let second = TcpListener::bind("127.0.0.1:0")?;
        let first = listener(first.local_addr()?.port(), ProcessId(42), OriginKind::Codex);
        let port = second.local_addr()?.port();
        let second = listener(port, ProcessId(43), OriginKind::ClaudeCode);
        let selected = second.id;
        let mut state = AppState::new();
        state.snapshot.listeners = vec![first, second];
        state.apply(Action::Search);
        for character in format!(":{port}").chars() {
            state.push_search_char(character);
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 20))?;
        terminal.draw(|frame| ui::render(frame, &mut state))?;
        let search_screen = rendered(&terminal);
        assert!(search_screen.contains(&format!("/ :{port}")));
        assert!(search_screen.contains(&format!(":{port} TCP")));
        assert!(search_screen.contains("atlas-web"));
        assert!(search_screen.contains("Claude Code"));
        assert!(search_screen.contains("All interfaces"));
        assert!(!search_screen.contains("Codex"));
        assert_eq!(state.selected, Some(selected));

        state.apply(Action::Inspect);
        terminal.draw(|frame| ui::render(frame, &mut state))?;
        let inspect_screen = rendered(&terminal);
        assert!(inspect_screen.contains("inspector"));
        assert!(inspect_screen.contains("Claude Code · high confidence"));
        assert!(!inspect_screen.contains("listeners 1"));

        state.apply(Action::Back);
        assert_eq!(state.mode, AppMode::Browse);
        assert_eq!(state.selected, Some(selected));

        state.apply(Action::Inspect);
        let mut wide_terminal = Terminal::new(TestBackend::new(132, 20))?;
        wide_terminal.draw(|frame| ui::render(frame, &mut state))?;
        let wide_screen = rendered(&wide_terminal);
        assert!(wide_screen.contains("listeners 1"));
        assert!(wide_screen.contains("inspector"));
        assert_eq!(state.selected, Some(selected));
        Ok(())
    }

    #[test]
    fn inaccessible_owner_keeps_binding_visible_without_an_exited_label()
    -> Result<(), Box<dyn std::error::Error>> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let port = socket.local_addr()?.port();
        let mut listener = listener(port, ProcessId(0), OriginKind::Unknown);
        listener.application.label = "<exited>".to_owned();
        listener.process.name = "<exited>".to_owned();
        listener.project = ProjectInfo::default();
        let mut state = AppState::new();
        state.snapshot.listeners.push(listener);
        state.apply(Action::MoveDown);

        let mut terminal = Terminal::new(TestBackend::new(80, 20))?;
        for view in [Action::Back, Action::Inspect] {
            state.apply(view);
            terminal.draw(|frame| ui::render(frame, &mut state))?;
            let screen = rendered(&terminal);
            assert!(screen.contains("Unknown owner"));
            assert!(screen.contains("All interfaces"));
            assert!(!screen.contains("<exited>"));
            assert!(!screen.contains("PID 0"));
        }
        Ok(())
    }
}
