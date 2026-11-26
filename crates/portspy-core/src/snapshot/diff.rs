//! Difference between two snapshots, keyed by stable listener identity.

use std::collections::{HashMap, HashSet};

use crate::domain::listener::ListenerId;
use crate::snapshot::SystemSnapshot;

/// What changed between two snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotDiff {
    /// Listeners present in the new snapshot but not in the old one.
    pub appeared: Vec<ListenerId>,
    /// Listeners present in the old snapshot but gone in the new one.
    pub disappeared: Vec<ListenerId>,
    /// Listeners still present whose process facts changed (memory, ...).
    pub changed: Vec<ListenerId>,
}

impl SnapshotDiff {
    /// Computes the difference between `before` and `after`.
    pub fn between(before: &SystemSnapshot, after: &SystemSnapshot) -> Self {
        let before_map: HashMap<ListenerId, _> =
            before.listeners.iter().map(|l| (l.id, l)).collect();
        let after_map: HashMap<ListenerId, _> = after.listeners.iter().map(|l| (l.id, l)).collect();

        let before_ids: HashSet<ListenerId> = before_map.keys().copied().collect();
        let after_ids: HashSet<ListenerId> = after_map.keys().copied().collect();

        let mut appeared: Vec<ListenerId> = after_ids.difference(&before_ids).copied().collect();
        let mut disappeared: Vec<ListenerId> = before_ids.difference(&after_ids).copied().collect();
        let mut changed: Vec<ListenerId> = before_ids
            .intersection(&after_ids)
            .filter(|id| before_map[id] != after_map[id])
            .copied()
            .collect();

        appeared.sort_by_key(|id| id.port);
        disappeared.sort_by_key(|id| id.port);
        changed.sort_by_key(|id| id.port);

        Self {
            appeared,
            disappeared,
            changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SnapshotDiff;
    use crate::domain::application::{ApplicationInfo, ApplicationKind};
    use crate::domain::confidence::Confidence;
    use crate::domain::listener::{Listener, ListenerId};
    use crate::domain::origin::OriginInfo;
    use crate::domain::process::{ProcessId, ProcessInfo};
    use crate::domain::protocol::Protocol;
    use crate::snapshot::SystemSnapshot;
    use std::net::{IpAddr, Ipv4Addr};

    fn listener(port: u16, memory: u64) -> Listener {
        let pid = ProcessId(port as u32);
        let process = ProcessInfo {
            pid,
            parent_pid: None,
            name: "node.exe".to_owned(),
            executable: None,
            command_line: None,
            working_directory: None,
            started_at: None,
            memory_bytes: Some(memory),
            cpu_percent: None,
            ancestors: Vec::new(),
        };
        Listener {
            id: ListenerId::new(
                Protocol::Tcp,
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                port,
                pid,
                None,
            ),
            protocol: Protocol::Tcp,
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
            process,
            application: ApplicationInfo::new(
                ApplicationKind::Unknown,
                ApplicationKind::Unknown.label().to_owned(),
                Confidence::Unknown,
            ),
            origin: OriginInfo::default(),
            project: crate::domain::project::ProjectInfo::default(),
            is_duplicate: false,
            is_orphan: false,
        }
    }

    fn snapshot(listeners: Vec<Listener>) -> SystemSnapshot {
        SystemSnapshot {
            captured_at: std::time::Instant::now(),
            wall_time: std::time::SystemTime::now(),
            listeners,
            processes: Vec::new(),
        }
    }

    #[test]
    fn detects_appearance() {
        let diff = SnapshotDiff::between(&snapshot(vec![]), &snapshot(vec![listener(3000, 1)]));
        assert_eq!(diff.appeared.len(), 1);
        assert!(diff.disappeared.is_empty());
        assert!(diff.changed.is_empty());
    }

    #[test]
    fn detects_disappearance() {
        let diff = SnapshotDiff::between(&snapshot(vec![listener(3000, 1)]), &snapshot(vec![]));
        assert!(diff.appeared.is_empty());
        assert_eq!(diff.disappeared.len(), 1);
        assert!(diff.changed.is_empty());
    }

    #[test]
    fn detects_metadata_change() {
        let diff = SnapshotDiff::between(
            &snapshot(vec![listener(3000, 1)]),
            &snapshot(vec![listener(3000, 2)]),
        );
        assert!(diff.appeared.is_empty());
        assert!(diff.disappeared.is_empty());
        assert_eq!(diff.changed.len(), 1);
    }

    #[test]
    fn identical_snapshots_are_empty() {
        let a = snapshot(vec![listener(3000, 1), listener(8080, 9)]);
        let diff = SnapshotDiff::between(&a, &a);
        assert!(diff.appeared.is_empty());
        assert!(diff.disappeared.is_empty());
        assert!(diff.changed.is_empty());
    }

    #[test]
    fn pid_recycling_is_treated_as_replacement() {
        // Same port, same PID, but a different start time: a recycled PID.
        // The identity changes, so the TUI sees one listener leave and a
        // new one arrive — the selection must not silently stick to the
        // recycled process.
        let before = listener(3000, 1);
        let mut after = listener(3000, 1);
        after.process.started_at = Some(std::time::SystemTime::now());
        after.id.started_at = after.process.started_at;
        let diff = SnapshotDiff::between(&snapshot(vec![before]), &snapshot(vec![after]));
        assert_eq!(diff.appeared.len(), 1);
        assert_eq!(diff.disappeared.len(), 1);
        assert!(diff.changed.is_empty());
    }
}
