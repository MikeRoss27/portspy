//! Details for the selected listener, with origin and binding presented first.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};

use portspy_core::domain::confidence::Confidence;
use portspy_core::domain::listener::Listener;
use portspy_core::domain::origin::OriginKind;

use crate::format::{address, duration, memory};
use crate::state::AppState;
use crate::ui::components::process_tree;
use crate::ui::theme::Theme;

/// Renders the details of the selected listener at its current scroll position.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &mut AppState) {
    let theme = Theme::default();
    let block = Block::bordered().title(" inspector ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    state.inspection_page_size = inner.height.saturating_sub(1).clamp(1, 6);
    if inner.is_empty() {
        return;
    }

    let Some(listener) = state.selected_listener() else {
        state.inspection_scroll = 0;
        frame.render_widget(
            Paragraph::new("No listener selected.").style(theme.muted_text()),
            inner,
        );
        return;
    };

    let paragraph = Paragraph::new(details(listener, state.snapshot.wall_time, theme))
        .wrap(Wrap { trim: false });
    let line_count = paragraph.line_count(inner.width);
    let max_scroll = line_count.saturating_sub(usize::from(inner.height));
    let scroll = usize::from(state.inspection_scroll).min(max_scroll);
    state.inspection_scroll = scroll as u16;
    frame.render_widget(paragraph.scroll((state.inspection_scroll, 0)), inner);
    if max_scroll > 0 {
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll.saturating_add(1)).position(scroll);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area,
            &mut scrollbar_state,
        );
    }
}

fn details(listener: &Listener, now: std::time::SystemTime, theme: Theme) -> Vec<Line<'static>> {
    let known_owner = listener.pid().as_u32() != 0;
    let application = if known_owner {
        listener.application.label.as_str()
    } else {
        "Unknown owner"
    };
    let title = Line::from(Span::styled(
        format!(
            ":{} {} · {application}",
            listener.port,
            listener.protocol.as_str().to_ascii_uppercase(),
        ),
        theme.header(),
    ));
    let binding = field(
        "Binding",
        format!(
            "{} · {}",
            address::full(listener.address, listener.port),
            address::scope(listener.address),
        ),
        theme,
    );
    if !known_owner {
        return vec![
            title,
            binding,
            Line::from(Span::styled(
                "Owner metadata unavailable",
                theme.muted_text(),
            )),
        ];
    }

    let project = listener.project.name().unwrap_or("Unknown");
    let origin = if listener.origin.kind == OriginKind::Unknown {
        "Unknown".to_owned()
    } else {
        format!(
            "{} · {}",
            listener.origin.label,
            confidence_label(listener.origin.confidence)
        )
    };
    let mut lines = vec![
        title,
        field("Project", project, theme),
        field("Origin", origin, theme),
        binding,
    ];

    if listener.is_duplicate {
        lines.push(Line::from(Span::styled(
            "Possible duplicate dev server",
            theme.warning_text(),
        )));
    }
    if listener.is_orphan {
        lines.push(Line::from(Span::styled(
            "Parent process gone",
            theme.warning_text(),
        )));
    }

    lines.extend([
        Line::default(),
        Line::from(Span::styled("Process chain", theme.table_header())),
    ]);
    lines.extend(process_tree::lines(listener, theme));
    lines.extend([
        Line::default(),
        Line::from(Span::styled("Process details", theme.table_header())),
        field(
            "Memory",
            memory::memory(listener.process.memory_bytes),
            theme,
        ),
        field(
            "Age",
            duration::uptime(listener.process.started_at, now),
            theme,
        ),
        field(
            "App match",
            confidence_label(listener.application.confidence),
            theme,
        ),
        field(
            "Folder",
            listener.process.working_directory.as_ref().map_or_else(
                || "Unavailable".to_owned(),
                |path| path.display().to_string(),
            ),
            theme,
        ),
        field(
            "Command",
            listener
                .process
                .command_line
                .as_deref()
                .unwrap_or("Unavailable"),
            theme,
        ),
    ]);
    lines
}

fn field(label: &str, value: impl Into<String>, theme: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<10} "), theme.muted_text()),
        Span::raw(value.into()),
    ])
}

fn confidence_label(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Exact => "exact match",
        Confidence::High => "high confidence",
        Confidence::Medium => "medium confidence",
        Confidence::Low => "low confidence",
        Confidence::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, TcpListener};

    use ratatui::{Terminal, backend::TestBackend};

    use portspy_core::domain::application::{ApplicationInfo, ApplicationKind};
    use portspy_core::domain::listener::ListenerId;
    use portspy_core::domain::origin::OriginInfo;
    use portspy_core::domain::process::{ProcessId, ProcessInfo};
    use portspy_core::domain::{project::ProjectInfo, protocol::Protocol};

    use super::*;
    use crate::input::actions::Action;

    #[test]
    fn short_terminals_can_scroll_without_passing_the_last_line()
    -> Result<(), Box<dyn std::error::Error>> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let port = socket.local_addr()?.port();
        let address = Ipv4Addr::UNSPECIFIED.into();
        let pid = ProcessId(42);
        let mut state = AppState::new();
        let id = ListenerId::new(Protocol::Tcp, address, port, pid, None);
        state.snapshot.listeners.push(Listener {
            id,
            protocol: Protocol::Tcp,
            address,
            port,
            process: ProcessInfo {
                pid,
                parent_pid: None,
                name: "node".to_owned(),
                executable: None,
                command_line: Some("node --watch app.js".to_owned()),
                working_directory: None,
                started_at: None,
                memory_bytes: None,
                cpu_percent: None,
                ancestors: Vec::new(),
            },
            application: ApplicationInfo::new(
                ApplicationKind::Node,
                "Node.js".to_owned(),
                Confidence::Exact,
            ),
            origin: OriginInfo::default(),
            project: ProjectInfo::default(),
            is_duplicate: false,
            is_orphan: false,
        });
        state.selected = Some(id);
        state.apply(Action::Inspect);
        let mut terminal = Terminal::new(TestBackend::new(52, 8))?;
        terminal.draw(|frame| render(frame, frame.area(), &mut state))?;
        let top: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(top.contains(&format!(":{port} TCP")));
        assert!(top.contains("All interfaces"));

        state.inspection_scroll = u16::MAX;
        terminal.draw(|frame| render(frame, frame.area(), &mut state))?;
        assert!((1..u16::MAX).contains(&state.inspection_scroll));
        assert_eq!(state.inspection_page_size, 5);
        let bottom: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(bottom.contains("node --watch app.js"));
        let bottom_offset = state.inspection_scroll;
        state.apply(Action::ScrollDetailsUp);
        assert!(state.inspection_scroll < bottom_offset);
        Ok(())
    }
}
