//! Tracks active listeners that have appeared since collection began.

use std::collections::HashSet;

use portspy_core::domain::listener::ListenerId;
use portspy_core::snapshot::SystemSnapshot;

#[derive(Debug, Clone, Default)]
pub(crate) struct RecentListeners {
    baseline: Option<HashSet<ListenerId>>,
    recent: HashSet<ListenerId>,
}

impl RecentListeners {
    pub(crate) fn has_baseline(&self) -> bool {
        self.baseline.is_some()
    }

    pub(crate) fn observe(&mut self, snapshot: &SystemSnapshot) {
        let mut active: HashSet<_> = snapshot
            .listeners
            .iter()
            .map(|listener| listener.id)
            .collect();
        let Some(baseline) = self.baseline.as_mut() else {
            self.baseline = Some(active);
            return;
        };

        // Once a baseline listener disappears, a future opening is recent
        // even if the same process rebinds the same endpoint.
        baseline.retain(|id| active.remove(id));
        self.recent = active;
    }

    pub(crate) fn contains(&self, id: ListenerId) -> bool {
        self.recent.contains(&id)
    }

    pub(crate) fn len(&self) -> usize {
        self.recent.len()
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::net::{IpAddr, Ipv4Addr, TcpListener};
    use std::time::{Duration, UNIX_EPOCH};

    use portspy_core::domain::listener::Listener;
    use portspy_core::domain::process::{ProcessId, ProcessInfo};
    use portspy_core::domain::protocol::Protocol;

    use super::{ListenerId, RecentListeners, SystemSnapshot};

    fn identity(socket: &TcpListener, pid: ProcessId) -> io::Result<ListenerId> {
        Ok(ListenerId::new(
            Protocol::Tcp,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            socket.local_addr()?.port(),
            pid,
            Some(UNIX_EPOCH),
        ))
    }

    fn snapshot(ids: &[ListenerId]) -> SystemSnapshot {
        let mut snapshot = SystemSnapshot::empty();
        snapshot.listeners = ids
            .iter()
            .map(|&id| Listener {
                id,
                protocol: id.protocol,
                address: id.address,
                port: id.port,
                socket_attribution: None,
                process: ProcessInfo {
                    pid: id.pid,
                    parent_pid: None,
                    name: "node".to_owned(),
                    executable: None,
                    command_line: None,
                    working_directory: None,
                    started_at: id.started_at,
                    memory_bytes: None,
                    cpu_percent: None,
                    ancestors: Vec::new(),
                },
                application: Default::default(),
                origin: Default::default(),
                project: Default::default(),
                is_duplicate: false,
                is_orphan: false,
            })
            .collect();
        snapshot
    }

    #[test]
    fn tracks_only_active_listeners_opened_after_baseline() -> io::Result<()> {
        let original_socket = TcpListener::bind("127.0.0.1:0")?;
        let new_socket = TcpListener::bind("127.0.0.1:0")?;
        let original = identity(&original_socket, ProcessId(1))?;
        let new = identity(&new_socket, ProcessId(1))?;
        let mut recent = RecentListeners::default();

        recent.observe(&snapshot(&[original]));
        assert_eq!(recent.len(), 0);
        recent.observe(&snapshot(&[new, original]));
        assert!(recent.contains(new));
        assert!(!recent.contains(original));
        assert_eq!(recent.len(), 1);

        let mut refresh = snapshot(&[original, new]);
        refresh.listeners[1].process.memory_bytes = Some(4096);
        recent.observe(&refresh);
        assert_eq!(recent.len(), 1);
        assert!(recent.contains(new));

        recent.observe(&snapshot(&[original]));
        assert!(!recent.contains(new));
        assert_eq!(recent.len(), 0);
        Ok(())
    }

    #[test]
    fn empty_first_snapshot_still_establishes_baseline() -> io::Result<()> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let id = identity(&socket, ProcessId(1))?;
        let mut recent = RecentListeners::default();

        recent.observe(&SystemSnapshot::empty());
        recent.observe(&snapshot(&[id]));
        assert!(recent.contains(id));
        Ok(())
    }

    #[test]
    fn a_baseline_socket_reopened_by_same_process_is_recent() -> io::Result<()> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let id = identity(&socket, ProcessId(1))?;
        let mut recent = RecentListeners::default();

        recent.observe(&snapshot(&[id]));
        recent.observe(&SystemSnapshot::empty());
        recent.observe(&snapshot(&[id]));
        assert!(recent.contains(id));
        Ok(())
    }

    #[test]
    fn both_pid_changes_and_pid_reuse_are_new_openings() -> io::Result<()> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let initial = identity(&socket, ProcessId(1))?;
        let updated_pid = identity(&socket, ProcessId(2))?;
        let reused_pid = ListenerId {
            started_at: Some(UNIX_EPOCH + Duration::from_secs(1)),
            ..updated_pid
        };
        let mut recent = RecentListeners::default();
        recent.observe(&snapshot(&[initial]));

        recent.observe(&snapshot(&[updated_pid]));
        assert!(recent.contains(updated_pid));
        assert!(!recent.contains(initial));
        recent.observe(&snapshot(&[reused_pid]));
        assert!(recent.contains(reused_pid));
        assert!(!recent.contains(updated_pid));
        assert_eq!(recent.len(), 1);
        Ok(())
    }
}
