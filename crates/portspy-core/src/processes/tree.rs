//! Process-tree operations: descendants and tree termination.
//!
//! The descendant computation is pure and testable; termination is executed
//! through the [`Platform`] contract so the OS calls stay inside
//! `portspy-platform`.

use std::collections::{HashMap, HashSet};

use crate::domain::process::{ProcessId, ProcessInfo};
use crate::platform::{KillOutcome, Platform, PlatformError};

/// Kills a process and every one of its descendants, deepest first.
///
/// Each process is terminated independently through [`Platform::kill_checked`];
/// a failure (access denied, exited, or unavailable identity) never aborts the remaining kills.
/// The owning process itself is killed last, after its children.
///
/// # Errors
///
/// Never fails as a whole: per-process failures are reported in the returned
/// report instead.
pub fn kill_tree<P: Platform + ?Sized>(
    platform: &P,
    processes: &[ProcessInfo],
    root: ProcessId,
) -> TreeKillReport {
    let identities: HashMap<_, _> = processes
        .iter()
        .map(|process| (process.pid, process.identity()))
        .collect();
    let mut report = TreeKillReport::default();
    for pid in kill_order(processes, root) {
        let result = identities
            .get(&pid)
            .filter(|identity| identity.is_stable())
            .ok_or(PlatformError::IdentityChanged { pid })
            .and_then(|identity| platform.kill_checked(*identity));
        match result {
            Ok(KillOutcome::Terminated) => report.terminated.push(pid),
            Ok(KillOutcome::AlreadyExited) => report.already_exited.push(pid),
            Err(error) => report.failed.push((pid, error)),
        }
    }
    report
}

/// Ordered list of PIDs to terminate for a tree kill: descendants before
/// their parents, deepest first, root last. Siblings are visited by ascending PID.
///
/// Descendants are discovered by walking `parent_pid` links. Cycles and
/// unknown parents are tolerated: a process is never visited twice and an
/// unreachable process is simply not part of the tree.
pub fn kill_order(processes: &[ProcessInfo], root: ProcessId) -> Vec<ProcessId> {
    let children = children_map(processes);

    // Depth-first walk from the root; the post-order (children before
    // parents) gives the desired kill sequence.
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    walk(root, &children, &mut visited, &mut order);
    order
}

/// `parent_pid → child pids`, for processes with a known parent.
fn children_map(processes: &[ProcessInfo]) -> HashMap<ProcessId, Vec<ProcessId>> {
    let started: HashMap<_, _> = processes
        .iter()
        .map(|process| (process.pid, process.started_at))
        .collect();
    let mut map: HashMap<ProcessId, Vec<ProcessId>> = HashMap::new();
    for process in processes {
        if let Some(parent) = process.parent_pid {
            if let (Some(Some(parent_started)), Some(child_started)) =
                (started.get(&parent), process.started_at)
                && *parent_started > child_started
            {
                continue;
            }
            map.entry(parent).or_default().push(process.pid);
        }
    }
    for siblings in map.values_mut() {
        siblings.sort_unstable();
    }
    map
}

/// Post-order traversal: visits `pid`, then its children, then re-emits
/// `pid` after them.
fn walk(
    pid: ProcessId,
    children: &HashMap<ProcessId, Vec<ProcessId>>,
    visited: &mut HashSet<ProcessId>,
    order: &mut Vec<ProcessId>,
) {
    if !visited.insert(pid) {
        return;
    }
    if let Some(child_list) = children.get(&pid) {
        for child in child_list {
            walk(*child, children, visited, order);
        }
    }
    order.push(pid);
}

/// Outcome of a tree kill.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeKillReport {
    /// Processes successfully terminated.
    pub terminated: Vec<ProcessId>,
    /// Processes that had already exited before termination.
    pub already_exited: Vec<ProcessId>,
    /// Processes that refused termination, with the platform error.
    pub failed: Vec<(ProcessId, PlatformError)>,
}

impl TreeKillReport {
    /// Total number of processes this report covers.
    pub fn total(&self) -> usize {
        self.terminated.len() + self.already_exited.len() + self.failed.len()
    }

    /// One-line human summary, e.g. `3 processes, 2 killed, 1 refused`.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!("{} processes", self.total()));
        if !self.terminated.is_empty() {
            parts.push(format!("{} killed", self.terminated.len()));
        }
        if !self.already_exited.is_empty() {
            parts.push(format!("{} already exited", self.already_exited.len()));
        }
        if !self.failed.is_empty() {
            parts.push(format!("{} failed", self.failed.len()));
        }
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::{TreeKillReport, kill_order, kill_tree};
    use crate::domain::process::{ProcessId, ProcessIdentity, ProcessInfo};
    use crate::platform::{KillOutcome, Platform, PlatformError, RawListener, RawProcess};
    use std::sync::Mutex;
    use std::time::{Duration, UNIX_EPOCH};

    fn process(pid: u32, parent: Option<u32>) -> ProcessInfo {
        ProcessInfo {
            pid: ProcessId(pid),
            parent_pid: parent.map(ProcessId),
            name: format!("p{pid}.exe"),
            executable: None,
            command_line: None,
            working_directory: None,
            started_at: Some(UNIX_EPOCH + Duration::from_secs(u64::from(pid))),
            memory_bytes: None,
            cpu_percent: None,
            ancestors: Vec::new(),
        }
    }

    /// Records every identity passed to `kill_checked`, failing for a configurable set.
    struct RecordingPlatform {
        killed: Mutex<Vec<ProcessIdentity>>,
        denied: Vec<u32>,
    }

    impl Platform for RecordingPlatform {
        fn listeners(&self) -> Result<Vec<RawListener>, PlatformError> {
            Ok(Vec::new())
        }
        fn processes(&self) -> Result<Vec<RawProcess>, PlatformError> {
            Ok(Vec::new())
        }
        fn kill(&self, _: ProcessId) -> Result<KillOutcome, PlatformError> {
            unreachable!("raw PID termination")
        }
        fn kill_checked(&self, identity: ProcessIdentity) -> Result<KillOutcome, PlatformError> {
            self.killed.lock().expect("lock").push(identity);
            let pid = identity.pid;
            if self.denied.contains(&pid.as_u32()) {
                Err(PlatformError::AccessDenied { pid })
            } else {
                Ok(KillOutcome::Terminated)
            }
        }
    }

    fn tree() -> Vec<ProcessInfo> {
        //        1
        //       / \
        //      2   3
        //     / \   \
        //    4   5   6
        vec![
            process(1, None),
            process(2, Some(1)),
            process(3, Some(1)),
            process(4, Some(2)),
            process(5, Some(2)),
            process(6, Some(3)),
        ]
    }

    #[test]
    fn kill_order_is_children_first_root_last() {
        let order = kill_order(&tree(), ProcessId(1));
        // 4 and 5 are 2's children and must come before 2; 6 before 3.
        let index = |pid: u32| order.iter().position(|p| p.as_u32() == pid).unwrap();
        assert_eq!(order.len(), 6);
        assert!(index(4) < index(2));
        assert!(index(5) < index(2));
        assert!(index(6) < index(3));
        assert!(index(2) < index(1));
        assert!(index(3) < index(1));
        assert_eq!(order.last().unwrap().as_u32(), 1);
    }

    #[test]
    fn kill_order_stops_at_cycle() {
        // A process that reports itself as its own parent must not loop.
        let processes = vec![
            process(1, None),
            process(2, Some(1)),
            process(3, Some(2)),
            process(3, Some(3)),
        ];
        let order = kill_order(&processes, ProcessId(1));
        assert_eq!(order.len(), 3);
        let mut seen: Vec<u32> = order.iter().map(|p| p.as_u32()).collect();
        seen.sort_unstable();
        assert_eq!(seen, [1, 2, 3]);
    }

    #[test]
    fn kill_order_with_unknown_root_is_single() {
        assert_eq!(kill_order(&tree(), ProcessId(99)), vec![ProcessId(99)]);
    }

    #[test]
    fn kill_order_is_independent_of_snapshot_order() {
        let mut processes = tree();
        let expected = kill_order(&processes, ProcessId(1));
        processes.reverse();
        assert_eq!(kill_order(&processes, ProcessId(1)), expected);
    }

    #[test]
    fn kill_order_does_not_follow_a_recycled_parent_pid() {
        let mut parent = process(1, None);
        parent.started_at = Some(UNIX_EPOCH + Duration::from_secs(2));
        let mut child = process(2, Some(1));
        child.started_at = Some(UNIX_EPOCH + Duration::from_secs(1));

        assert_eq!(kill_order(&[parent, child], ProcessId(1)), [ProcessId(1)]);
    }

    #[test]
    fn kill_tree_reports_every_outcome() {
        let platform = RecordingPlatform {
            killed: Mutex::new(Vec::new()),
            denied: vec![3],
        };
        let report = kill_tree(&platform, &tree(), ProcessId(1));
        let killed = platform.killed.lock().expect("lock");
        assert_eq!(report.terminated.len(), 5);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].0, ProcessId(3));
        assert_eq!(report.total(), 6);
        assert!(report.summary().contains("1 failed"));
        assert!(report.summary().contains("5 killed"));
        // Everything was attempted exactly once, root last.
        assert_eq!(killed.len(), 6);
        assert_eq!(killed.last(), Some(&tree()[0].identity()));
        assert!(killed.iter().all(ProcessIdentity::is_stable));
    }

    #[test]
    fn kill_tree_continues_after_failure() {
        let platform = RecordingPlatform {
            killed: Mutex::new(Vec::new()),
            denied: vec![1, 2],
        };
        let report = kill_tree(&platform, &tree(), ProcessId(1));
        // 4 and 5 were killed before their parent 2 failed.
        let killed = platform.killed.lock().expect("lock");
        assert_eq!(report.terminated.len(), 4);
        assert_eq!(report.failed.len(), 2);
        assert_eq!(killed.len(), 6);
    }

    #[test]
    fn report_default_is_empty() {
        let report = TreeKillReport::default();
        assert_eq!(report.total(), 0);
        assert_eq!(report.summary(), "0 processes");
    }

    #[test]
    fn kill_tree_does_not_signal_a_pid_without_its_creation_time() {
        let platform = RecordingPlatform {
            killed: Mutex::new(Vec::new()),
            denied: Vec::new(),
        };
        let pid = ProcessId(1);
        let mut root = process(1, None);
        root.started_at = None;

        let report = kill_tree(&platform, &[root], pid);

        assert_eq!(
            report.failed,
            [(pid, PlatformError::IdentityChanged { pid })]
        );
        assert!(platform.killed.lock().expect("lock").is_empty());
    }
}
