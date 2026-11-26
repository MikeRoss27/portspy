//! Query primitives over a snapshot: search and sorting.
//!
//! These are pure functions over immutable snapshots. The TUI and the CLI
//! share exactly the same behavior.

use crate::domain::application::ApplicationKind;
use crate::domain::listener::Listener;
use crate::domain::origin::OriginKind;

/// Field a list can be sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortField {
    Port,
    Application,
    Origin,
    Process,
    Memory,
    Uptime,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub const fn is_ascending(self) -> bool {
        matches!(self, Self::Ascending)
    }
}

/// Sorts listeners by `field` in the given `order`.
///
/// The sort is stable: listeners sharing a sort key keep their snapshot
/// order (which is port-ordered), so the list never looks random.
pub fn sort_listeners<'a>(
    listeners: &'a [Listener],
    field: SortField,
    order: SortOrder,
) -> Vec<&'a Listener> {
    let mut sorted: Vec<&'a Listener> = listeners.iter().collect();
    let cmp: fn(&Listener, &Listener) -> std::cmp::Ordering = match field {
        SortField::Port => |a, b| a.port.cmp(&b.port),
        SortField::Application => {
            |a, b| application_rank(a.application.kind).cmp(&application_rank(b.application.kind))
        }
        SortField::Origin => |a, b| origin_rank(a.origin.kind).cmp(&origin_rank(b.origin.kind)),
        SortField::Process => |a, b| a.process.name.cmp(&b.process.name),
        SortField::Memory => |a, b| a.process.memory_bytes.cmp(&b.process.memory_bytes),
        SortField::Uptime => |a, b| a.process.started_at.cmp(&b.process.started_at),
    };
    match order {
        SortOrder::Ascending => sorted.sort_by(|a, b| cmp(a, b)),
        SortOrder::Descending => sorted.sort_by(|a, b| cmp(b, a)),
    }
    sorted
}

/// Sorts an already-filtered list of listener references.
///
/// [`sort_listeners`] takes a slice of owned listeners; this variant is for
/// pipelines that filter first (search) and then sort the survivors.
pub fn sort_listener_refs<'s, 'a>(
    listeners: &'s [&'a Listener],
    field: SortField,
    order: SortOrder,
) -> Vec<&'a Listener> {
    let mut sorted: Vec<&'a Listener> = listeners.to_vec();
    let cmp: fn(&Listener, &Listener) -> std::cmp::Ordering = match field {
        SortField::Port => |a, b| a.port.cmp(&b.port),
        SortField::Application => {
            |a, b| application_rank(a.application.kind).cmp(&application_rank(b.application.kind))
        }
        SortField::Origin => |a, b| origin_rank(a.origin.kind).cmp(&origin_rank(b.origin.kind)),
        SortField::Process => |a, b| a.process.name.cmp(&b.process.name),
        SortField::Memory => |a, b| a.process.memory_bytes.cmp(&b.process.memory_bytes),
        SortField::Uptime => |a, b| a.process.started_at.cmp(&b.process.started_at),
    };
    match order {
        SortOrder::Ascending => sorted.sort_by(|a, b| cmp(a, b)),
        SortOrder::Descending => sorted.sort_by(|a, b| cmp(b, a)),
    }
    sorted
}

/// Filters listeners by a case-insensitive substring query.
///
/// Searchable fields include the port, protocol, application, project,
/// origin, process name, PID and command line. An empty query matches
/// everything.
pub fn search<'a>(listeners: &'a [Listener], query: &str) -> Vec<&'a Listener> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return listeners.iter().collect();
    }
    listeners
        .iter()
        .filter(|l| {
            let port = l.port.to_string();
            port.contains(&needle)
                || needle
                    .strip_prefix(':')
                    .is_some_and(|prefix| !prefix.is_empty() && port.starts_with(prefix))
                || l.application.label.to_lowercase().contains(&needle)
                || l.process.name.to_lowercase().contains(&needle)
                || l.origin.label.to_lowercase().contains(&needle)
                || l.pid().to_string().contains(&needle)
                || l.protocol.as_str().contains(&needle)
                || l.project
                    .name()
                    .is_some_and(|n| n.to_lowercase().contains(&needle))
                || l.process
                    .command_line
                    .as_deref()
                    .is_some_and(|c| c.to_lowercase().contains(&needle))
        })
        .collect()
}

/// Finds a listener by port in the latest snapshot.
///
/// When several protocols listen on the same port, the first match wins
/// (snapshot order: TCP IPv4, TCP IPv6, UDP IPv4, UDP IPv6).
pub fn find_by_port(listeners: &[Listener], port: u16) -> Option<&Listener> {
    listeners.iter().find(|l| l.port == port)
}

/// Finds a listener by port and protocol in the latest snapshot.
///
/// Use this when the caller must not be ambiguous about which socket is
/// meant (e.g. `portspy inspect/kill` on a port shared by TCP and UDP).
pub fn find_by_port_and_protocol(
    listeners: &[Listener],
    port: u16,
    protocol: crate::domain::protocol::Protocol,
) -> Option<&Listener> {
    listeners
        .iter()
        .find(|l| l.port == port && l.protocol == protocol)
}

/// Stable display index of a listener id within a sorted list.
pub fn index_of(listeners: &[&Listener], id: crate::domain::listener::ListenerId) -> Option<usize> {
    listeners.iter().position(|l| l.id == id)
}

fn application_rank(kind: ApplicationKind) -> u8 {
    // Ranked so unknown kinds sort last.
    match kind {
        ApplicationKind::NextJs => 0,
        ApplicationKind::Vite => 1,
        ApplicationKind::Node => 2,
        ApplicationKind::Python => 3,
        ApplicationKind::Rust => 4,
        ApplicationKind::PostgreSql => 5,
        ApplicationKind::Redis => 6,
        ApplicationKind::Docker => 7,
        ApplicationKind::Unknown => 8,
    }
}

fn origin_rank(kind: OriginKind) -> u8 {
    match kind {
        OriginKind::Codex => 0,
        OriginKind::ClaudeCode => 1,
        OriginKind::Cursor => 2,
        OriginKind::Zed => 3,
        OriginKind::VsCode => 4,
        OriginKind::WindowsTerminal => 5,
        OriginKind::PowerShell => 6,
        OriginKind::Cmd => 7,
        OriginKind::Wsl => 8,
        OriginKind::Docker => 9,
        OriginKind::Unknown => 10,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::{SortField, SortOrder, find_by_port_and_protocol, search, sort_listeners};
    use crate::domain::application::{ApplicationInfo, ApplicationKind};
    use crate::domain::confidence::Confidence;
    use crate::domain::listener::{Listener, ListenerId};
    use crate::domain::origin::{OriginInfo, OriginKind};
    use crate::domain::process::{ProcessId, ProcessInfo};
    use crate::domain::protocol::Protocol;
    use std::net::{IpAddr, Ipv4Addr};

    fn listener(
        port: u16,
        pid: u32,
        name: &str,
        memory: Option<u64>,
        application: ApplicationKind,
        origin: OriginKind,
    ) -> Listener {
        listener_for(Protocol::Tcp, port, pid, name, memory, application, origin)
    }

    #[allow(clippy::too_many_arguments)]
    fn listener_for(
        protocol: Protocol,
        port: u16,
        pid: u32,
        name: &str,
        memory: Option<u64>,
        application: ApplicationKind,
        origin: OriginKind,
    ) -> Listener {
        let process = ProcessInfo {
            pid: ProcessId(pid),
            parent_pid: None,
            name: name.to_owned(),
            executable: None,
            command_line: None,
            working_directory: None,
            started_at: None,
            memory_bytes: memory,
            cpu_percent: None,
            ancestors: Vec::new(),
        };
        Listener {
            id: ListenerId::new(
                protocol,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                port,
                ProcessId(pid),
                None,
            ),
            protocol,
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
            process,
            application: ApplicationInfo::new(
                application,
                application.label().to_owned(),
                Confidence::High,
            ),
            origin: OriginInfo::new(origin, origin.label().to_owned(), Confidence::High, None),
            project: crate::domain::project::ProjectInfo::default(),
            is_duplicate: false,
            is_orphan: false,
        }
    }

    fn fixture() -> Vec<Listener> {
        vec![
            listener(
                3000,
                1,
                "node.exe",
                Some(100),
                ApplicationKind::NextJs,
                OriginKind::Codex,
            ),
            listener(
                5173,
                2,
                "node.exe",
                Some(200),
                ApplicationKind::Vite,
                OriginKind::Cursor,
            ),
            listener(
                5432,
                3,
                "postgres.exe",
                Some(300),
                ApplicationKind::PostgreSql,
                OriginKind::Docker,
            ),
        ]
    }

    #[test]
    fn sorts_by_port_ascending() {
        let data = fixture();
        let sorted = sort_listeners(&data, SortField::Port, SortOrder::Ascending);
        let ports: Vec<u16> = sorted.iter().map(|l| l.port).collect();
        assert_eq!(ports, [3000, 5173, 5432]);
    }

    #[test]
    fn sorts_by_memory_descending() {
        let data = fixture();
        let sorted = sort_listeners(&data, SortField::Memory, SortOrder::Descending);
        let ports: Vec<u16> = sorted.iter().map(|l| l.port).collect();
        assert_eq!(ports, [5432, 5173, 3000]);
    }

    #[test]
    fn memory_unknown_sorts_first_ascending() {
        let mut data = fixture();
        data.push(listener(
            9999,
            4,
            "weird.exe",
            None,
            ApplicationKind::Unknown,
            OriginKind::Unknown,
        ));
        let sorted = sort_listeners(&data, SortField::Memory, SortOrder::Ascending);
        assert_eq!(sorted.first().expect("non-empty").port, 9999);
    }

    #[test]
    fn sorts_by_application_rank() {
        let data = fixture();
        let sorted = sort_listeners(&data, SortField::Application, SortOrder::Ascending);
        let apps: Vec<ApplicationKind> = sorted.iter().map(|l| l.application.kind).collect();
        assert_eq!(
            apps,
            [
                ApplicationKind::NextJs,
                ApplicationKind::Vite,
                ApplicationKind::PostgreSql
            ]
        );
    }

    #[test]
    fn search_matches_port() {
        assert_eq!(search(&fixture(), "5173").len(), 1);
    }

    #[test]
    fn search_matches_application_and_origin_labels() {
        assert_eq!(search(&fixture(), "next").len(), 1);
        assert_eq!(search(&fixture(), "cursor").len(), 1);
    }

    #[test]
    fn finds_listener_by_project_origin_or_displayed_port() -> std::io::Result<()> {
        use crate::domain::project::{ProjectInfo, ProjectKind};

        let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = socket.local_addr()?.port();
        let mut data = [listener(
            port,
            1,
            "node",
            None,
            ApplicationKind::NextJs,
            OriginKind::ClaudeCode,
        )];
        data[0].project =
            ProjectInfo::new(ProjectKind::Named("atlas-web".to_owned()), Confidence::High);

        for query in ["ATLAS-WEB", "Claude Code", &format!(":{port}")] {
            assert_eq!(search(&data, query), [&data[0]]);
        }
        assert!(search(&data, "atlas-api").is_empty());
        assert!(search(&data, ":").is_empty());
        Ok(())
    }

    #[test]
    fn search_matches_process_name_and_pid() {
        assert_eq!(search(&fixture(), "postgres").len(), 1);
        assert_eq!(search(&fixture(), "5432").len(), 1);
    }

    #[test]
    fn search_is_case_insensitive() {
        assert_eq!(search(&fixture(), "NEXT").len(), 1);
        assert_eq!(search(&fixture(), "Codex").len(), 1);
    }

    #[test]
    fn empty_search_matches_everything() {
        assert_eq!(search(&fixture(), "").len(), 3);
        assert_eq!(search(&fixture(), "   ").len(), 3);
    }

    #[test]
    fn no_match_yields_empty() {
        assert!(search(&fixture(), "zzz-not-there").is_empty());
    }

    #[test]
    fn find_by_port_and_protocol_disambiguates_shared_port() {
        let mut data = fixture();
        data.push(listener_for(
            Protocol::Udp,
            3000,
            9,
            "node.exe",
            None,
            ApplicationKind::Node,
            OriginKind::Unknown,
        ));
        let udp = find_by_port_and_protocol(&data, 3000, Protocol::Udp).expect("udp match");
        assert_eq!(udp.pid(), ProcessId(9));
        let tcp = find_by_port_and_protocol(&data, 3000, Protocol::Tcp).expect("tcp match");
        assert_eq!(tcp.pid(), ProcessId(1));
        assert!(find_by_port_and_protocol(&data, 9999, Protocol::Udp).is_none());
    }
}
