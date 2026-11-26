//! Process chain for the selected listener.

use ratatui::text::{Line, Span};

use portspy_core::domain::listener::Listener;
use portspy_core::domain::process::ProcessId;

use crate::ui::theme::Theme;

/// Builds the visible process chain with its origin match identified in text.
pub fn lines(listener: &Listener, theme: Theme) -> Vec<Line<'static>> {
    let leaf = (listener.process.display_name(), listener.pid());
    let ancestors = listener
        .process
        .ancestors
        .iter()
        .rev()
        .map(|ancestor| (ancestor.name.as_str(), ancestor.identity.pid));
    let mut lines: Vec<_> = ancestors
        .chain(std::iter::once(leaf))
        .enumerate()
        .map(|(index, (name, pid))| line(name, pid, index != 0, listener.origin.matched_pid, theme))
        .collect();

    if listener.process.ancestors.is_empty() {
        lines.push(Line::from(Span::styled(
            "No parent information",
            theme.muted_text(),
        )));
    }
    lines
}

fn line(
    name: &str,
    pid: ProcessId,
    connected: bool,
    matched: Option<ProcessId>,
    theme: Theme,
) -> Line<'static> {
    let prefix = if connected { "  ↳ " } else { "" };
    let mut spans = vec![Span::raw(format!("{prefix}{name} (PID {pid})"))];
    if matched == Some(pid) {
        spans.push(Span::styled(" · origin match", theme.accent_text()));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::lines;
    use crate::ui::theme::Theme;
    use portspy_core::domain::application::{ApplicationInfo, ApplicationKind};
    use portspy_core::domain::confidence::Confidence;
    use portspy_core::domain::listener::{Listener, ListenerId};
    use portspy_core::domain::origin::{OriginInfo, OriginKind};
    use portspy_core::domain::process::{ProcessAncestor, ProcessId, ProcessIdentity, ProcessInfo};
    use portspy_core::domain::protocol::Protocol;
    use std::net::{IpAddr, Ipv4Addr, TcpListener};

    #[test]
    fn shows_parent_to_listener_with_only_the_matched_process_marked() -> std::io::Result<()> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let port = socket.local_addr()?.port();
        let listener = Listener {
            id: ListenerId::new(
                Protocol::Tcp,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                port,
                ProcessId(42),
                None,
            ),
            protocol: Protocol::Tcp,
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
            process: ProcessInfo {
                pid: ProcessId(42),
                parent_pid: Some(ProcessId(7)),
                name: "node".to_owned(),
                executable: None,
                command_line: None,
                working_directory: None,
                started_at: None,
                memory_bytes: None,
                cpu_percent: None,
                ancestors: vec![
                    ProcessAncestor::new(
                        ProcessIdentity::new(ProcessId(7), None),
                        "codex".to_owned(),
                    ),
                    ProcessAncestor::new(
                        ProcessIdentity::new(ProcessId(1), None),
                        "zsh".to_owned(),
                    ),
                ],
            },
            application: ApplicationInfo::new(
                ApplicationKind::Node,
                "Node.js".to_owned(),
                Confidence::Exact,
            ),
            origin: OriginInfo::new(
                OriginKind::Codex,
                "Codex".to_owned(),
                Confidence::High,
                Some(ProcessId(7)),
            ),
            project: portspy_core::domain::project::ProjectInfo::default(),
            is_duplicate: false,
            is_orphan: false,
        };
        let rendered: Vec<_> = lines(&listener, Theme::default())
            .into_iter()
            .map(|line| line.to_string())
            .collect();

        assert_eq!(
            rendered,
            [
                "zsh (PID 1)",
                "  ↳ codex (PID 7) · origin match",
                "  ↳ node (PID 42)"
            ],
        );
        Ok(())
    }
}
