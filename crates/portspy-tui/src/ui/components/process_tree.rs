//! Process chain: the ancestor tree of the selected listener.

use ratatui::text::{Line, Span};

use portspy_core::domain::listener::Listener;

use crate::ui::theme::Theme;

/// Builds the rendered lines of the ancestor chain, leaf first.
pub fn lines(listener: &Listener, theme: Theme) -> Vec<Line<'static>> {
    let mut out = vec![Line::from(Span::styled(
        format!(
            " ├── {} (pid {})",
            listener.process.display_name(),
            listener.pid()
        ),
        theme.accent_text(),
    ))];
    for (index, ancestor) in listener.process.ancestors.iter().enumerate() {
        let is_last = index == listener.process.ancestors.len() - 1;
        let branch = if is_last { " └──" } else { " ├──" };
        out.push(Line::from(Span::styled(
            format!(" {branch} {} (pid {})", ancestor.name, ancestor.identity.pid),
            theme.muted_text(),
        )));
    }
    if listener.process.ancestors.is_empty() {
        out.push(Line::from(Span::styled(
            "   (ancestors unknown)",
            theme.muted_text(),
        )));
    }
    out
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
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn chain_lines_are_leaf_first() {
        let listener = Listener {
            id: ListenerId::new(Protocol::Tcp, IpAddr::V4(Ipv4Addr::LOCALHOST), 3000, ProcessId(42), None),
            protocol: Protocol::Tcp,
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 3000,
            process: ProcessInfo {
                pid: ProcessId(42),
                parent_pid: Some(ProcessId(7)),
                name: "node.exe".to_owned(),
                executable: None,
                command_line: None,
                working_directory: None,
                started_at: None,
                memory_bytes: None,
                cpu_percent: None,
                ancestors: vec![
                    ProcessAncestor::new(ProcessIdentity::new(ProcessId(7), None), "pwsh.exe".to_owned()),
                    ProcessAncestor::new(ProcessIdentity::new(ProcessId(1), None), "code.exe".to_owned()),
                ],
            },
            application: ApplicationInfo::new(
                ApplicationKind::Node,
                "Node".to_owned(),
                Confidence::Exact,
            ),
            origin: OriginInfo::new(
                OriginKind::VsCode,
                "VS Code".to_owned(),
                Confidence::High,
                Some(ProcessId(1)),
            ),
        };
        let rendered = lines(&listener, Theme::default());
        assert_eq!(rendered.len(), 3);
        assert!(rendered[0].to_string().contains("node.exe"));
        assert!(rendered[2].to_string().contains("code.exe"));
    }
}
