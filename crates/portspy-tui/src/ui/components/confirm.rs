//! Confirmation overlay showing the exact process and impact of a stop.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};

use crate::format::address;
use crate::state::AppState;
use crate::ui::theme::Theme;

/// Whether the terminal can display the target and allow review of its scope.
pub(crate) const fn can_review(width: u16, height: u16) -> bool {
    width >= 32 && height >= 8
}

/// Renders the exact stop action under review.
pub fn overlay(frame: &mut Frame<'_>, area: Rect, state: &mut AppState) {
    let Some(plan) = &state.stop_plan else {
        return;
    };
    let theme = Theme::default();
    if !can_review(area.width, area.height) {
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new("Enlarge terminal to review · Esc Cancel")
                .wrap(Wrap { trim: true })
                .style(theme.danger_text()),
            area,
        );
        return;
    }
    let title = if plan.tree {
        "Stop process tree?"
    } else {
        "Stop process?"
    };
    let mut lines = vec![
        Line::from(format!(
            "{} · {} (PID {})",
            plan.application, plan.process.name, plan.process.identity.pid
        )),
        Line::from(Span::styled("Affected listeners", theme.table_header())),
    ];
    lines.extend(plan.listeners.iter().map(|id| {
        Line::from(format!(
            "  {} {} · PID {}",
            id.protocol.as_str().to_ascii_uppercase(),
            address::full(id.address, id.port),
            id.pid
        ))
    }));
    if plan.tree {
        lines.push(Line::from(Span::styled(
            format!("Reviewed descendants ({})", plan.descendants.len()),
            theme.table_header(),
        )));
        lines.extend(plan.descendants.iter().map(|process| {
            Line::from(format!("  {} · PID {}", process.name, process.identity.pid))
        }));
    }

    let width = area.width.saturating_sub(4).min(76);
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let line_count = paragraph.line_count(width.saturating_sub(2));
    let max_height = area.height.saturating_sub(2);
    let height = u16::try_from(line_count.saturating_add(2))
        .unwrap_or(u16::MAX)
        .min(max_height);
    if width < 4 || height < 3 {
        return;
    }
    let hidden_lines = line_count.saturating_sub(usize::from(height.saturating_sub(2)));
    state.stop_review_scroll = state
        .stop_review_scroll
        .min(u16::try_from(hidden_lines).unwrap_or(u16::MAX));
    let scroll_help = " ↑↓ Review · y Stop · Esc Cancel ";
    let help = if hidden_lines == 0 {
        " y Stop · Esc Cancel "
    } else if usize::from(width.saturating_sub(2)) < scroll_help.chars().count() {
        " ↑↓ · y Stop · Esc Cancel "
    } else {
        scroll_help
    };
    let popup = centered_rect(area, width, height);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        paragraph
            .block(
                theme
                    .panel("△", title)
                    .borders(ratatui::widgets::Borders::ALL)
                    .padding(ratatui::widgets::Padding::horizontal(0))
                    .border_style(theme.danger_text())
                    .title_bottom(Span::styled(help, theme.footer_hint())),
            )
            .scroll((state.stop_review_scroll, 0)),
        popup,
    );
}

/// Centering is kept independent of the panel's content measurements.
fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use portspy_core::domain::listener::ListenerId;
    use portspy_core::domain::process::{ProcessId, ProcessIdentity};
    use portspy_core::domain::protocol::Protocol;
    use portspy_core::processes::stop::{StopPlan, StopProcess};
    use ratatui::{Terminal, backend::TestBackend};
    use std::net::{TcpListener, UdpSocket};
    use std::time::UNIX_EPOCH;

    #[test]
    fn review_shows_the_exact_affected_sockets_and_descendants()
    -> Result<(), Box<dyn std::error::Error>> {
        let tcp = TcpListener::bind("127.0.0.1:0")?;
        let udp = UdpSocket::bind("127.0.0.1:0")?;
        let tcp_addr = tcp.local_addr()?;
        let udp_addr = udp.local_addr()?;
        let owner = ProcessId(71);
        let child = ProcessId(72);
        let start = Some(UNIX_EPOCH);
        let chosen = ListenerId::new(Protocol::Tcp, tcp_addr.ip(), tcp_addr.port(), owner, start);
        let other = ListenerId::new(Protocol::Udp, udp_addr.ip(), udp_addr.port(), child, start);

        let mut state = AppState::new();
        state.stop_plan = Some(StopPlan {
            target: chosen,
            application: "Next.js".to_owned(),
            process: StopProcess {
                identity: ProcessIdentity::new(owner, start),
                name: "node".to_owned(),
            },
            tree: true,
            descendants: vec![StopProcess {
                identity: ProcessIdentity::new(child, start),
                name: "worker".to_owned(),
            }],
            listeners: vec![chosen, other],
        });
        let mut terminal = Terminal::new(TestBackend::new(80, 18))?;
        terminal.draw(|frame| overlay(frame, frame.area(), &mut state))?;
        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(content.contains("Next.js · node (PID 71)"));
        assert!(content.contains(&format!("TCP {tcp_addr} · PID 71")));
        assert!(content.contains(&format!("UDP {udp_addr} · PID 72")));
        assert!(content.contains("worker · PID 72"));
        assert!(content.contains("y Stop · Esc Cancel"));

        let mut compact = Terminal::new(TestBackend::new(32, 8))?;
        compact.draw(|frame| overlay(frame, frame.area(), &mut state))?;
        let content: String = compact
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(content.contains("↑↓ · y Stop · Esc Cancel"));
        assert!(!can_review(22, 5));
        Ok(())
    }
}
