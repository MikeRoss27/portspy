//! Parent-process chain reconstruction.
//!
//! Given the process owning a listener, PortSpy walks upward through the
//! parent chain until it reaches a system process, a process outside the
//! snapshot, a cycle, or the configured depth limit.
//!
//! The result feeds origin detection ("what caused this process to exist?")
//! and the TUI details view.

use std::collections::HashMap;

use crate::domain::process::{ProcessAncestor, ProcessId, ProcessInfo};

/// Maximum depth of a reconstructed ancestry. Windows process trees are
/// shallow in practice; the cap protects against corrupted data.
pub const MAX_ANCESTRY_DEPTH: usize = 32;

/// A reconstructed parent chain, leaf first.
///
/// `chain[0]` is the parent of the leaf process; the last element is the
/// oldest process PortSpy could reach.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProcessAncestry {
    pub chain: Vec<ProcessAncestor>,
}

/// Builds ancestor chains from a set of process facts.
///
/// The builder is created once per snapshot and reused for every listener,
/// which avoids repeated table lookups.
#[derive(Debug, Clone)]
pub struct AncestryBuilder {
    /// PID → process facts.
    by_pid: HashMap<ProcessId, ProcessInfo>,
}

impl AncestryBuilder {
    /// Creates a builder from a bulk process snapshot.
    pub fn new(processes: impl IntoIterator<Item = ProcessInfo>) -> Self {
        Self {
            by_pid: processes.into_iter().map(|p| (p.pid, p)).collect(),
        }
    }

    /// Number of known processes.
    pub fn len(&self) -> usize {
        self.by_pid.len()
    }

    /// Whether no process is known.
    pub fn is_empty(&self) -> bool {
        self.by_pid.is_empty()
    }

    /// Reconstructs the ancestry of `pid`, leaf's parent first.
    ///
    /// The chain is empty when the parent is unknown. The walk stops at the
    /// configured depth, when a parent disappears from the snapshot, or when
    /// a cycle is detected — whichever happens first. The walk never returns
    /// an error for messy data; partial information is valid.
    pub fn ancestry(&self, pid: ProcessId) -> ProcessAncestry {
        let mut chain = Vec::new();
        let mut seen = std::collections::HashSet::new();
        seen.insert(pid);

        let mut current = pid;
        for _ in 0..MAX_ANCESTRY_DEPTH {
            let Some(process) = self.by_pid.get(&current) else {
                break;
            };
            let Some(parent) = process.parent_pid else {
                break;
            };
            if parent == ProcessId(0) {
                break;
            }
            if !seen.insert(parent) {
                // Cycle in parent pointers (corrupted snapshot or a PID
                // recycled mid-collection). Stop before looping forever.
                break;
            }
            let Some(parent_info) = self.by_pid.get(&parent) else {
                // Parent known by PID but absent from the snapshot: keep the
                // reference with its PID and name, then stop.
                chain.push(ProcessAncestor::new(
                    crate::domain::process::ProcessIdentity::new(parent, None),
                    format!("<{parent}>"),
                ));
                break;
            };
            chain.push(ProcessAncestor::new(
                parent_info.identity(),
                parent_info.name.clone(),
            ));
            current = parent;
        }

        ProcessAncestry { chain }
    }
}

#[cfg(test)]
mod tests {
    use super::{AncestryBuilder, MAX_ANCESTRY_DEPTH};
    use crate::domain::process::{ProcessId, ProcessInfo};
    use std::time::{Duration, SystemTime};

    fn process(pid: u32, parent: Option<u32>, name: &str) -> ProcessInfo {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(u64::from(pid) * 10);
        ProcessInfo {
            pid: ProcessId(pid),
            parent_pid: parent.map(ProcessId),
            name: name.to_owned(),
            executable: None,
            command_line: None,
            working_directory: None,
            started_at: Some(start),
            memory_bytes: None,
            cpu_percent: None,
            ancestors: Vec::new(),
        }
    }

    /// WindowsTerminal(10) → pwsh(11) → codex(12) → node(13)
    fn fixture_tree() -> Vec<ProcessInfo> {
        vec![
            process(10, None, "WindowsTerminal.exe"),
            process(11, Some(10), "pwsh.exe"),
            process(12, Some(11), "codex.exe"),
            process(13, Some(12), "node.exe"),
        ]
    }

    fn names(chain: &[crate::domain::process::ProcessAncestor]) -> Vec<String> {
        chain.iter().map(|a| a.name.clone()).collect()
    }

    #[test]
    fn reconstructs_full_chain_leaf_first() {
        let builder = AncestryBuilder::new(fixture_tree());
        let ancestry = builder.ancestry(ProcessId(13));
        assert_eq!(
            names(&ancestry.chain),
            ["codex.exe", "pwsh.exe", "WindowsTerminal.exe"]
        );
    }

    #[test]
    fn chain_stops_at_unknown_parent() {
        let mut tree = fixture_tree();
        tree[0].parent_pid = Some(ProcessId(99));
        let builder = AncestryBuilder::new(tree);
        let ancestry = builder.ancestry(ProcessId(13));
        assert_eq!(
            names(&ancestry.chain),
            ["codex.exe", "pwsh.exe", "WindowsTerminal.exe", "<99>"]
        );
    }

    #[test]
    fn missing_parent_yields_empty_chain() {
        let builder = AncestryBuilder::new(vec![process(7, None, "server.exe")]);
        let ancestry = builder.ancestry(ProcessId(7));
        assert!(ancestry.chain.is_empty());
    }

    #[test]
    fn unknown_pid_yields_empty_chain() {
        let builder = AncestryBuilder::new(fixture_tree());
        let ancestry = builder.ancestry(ProcessId(9999));
        assert!(ancestry.chain.is_empty());
    }

    #[test]
    fn parent_without_metadata_is_kept_by_pid() {
        let tree = vec![process(13, Some(12), "node.exe")];
        let builder = AncestryBuilder::new(tree);
        let ancestry = builder.ancestry(ProcessId(13));
        assert_eq!(ancestry.chain.len(), 1);
        assert_eq!(ancestry.chain[0].identity.pid, ProcessId(12));
    }

    #[test]
    fn cycle_terminates_without_panicking() {
        // 5 → 6 → 7 → 5. The chain ends at the cycle without repeating it.
        let tree = vec![
            process(5, Some(6), "a.exe"),
            process(6, Some(7), "b.exe"),
            process(7, Some(5), "c.exe"),
        ];
        let builder = AncestryBuilder::new(tree);
        let ancestry = builder.ancestry(ProcessId(5));
        assert_eq!(names(&ancestry.chain), ["b.exe", "c.exe"]);
    }

    #[test]
    fn self_parent_cycle_terminates() {
        let tree = vec![process(5, Some(5), "weird.exe")];
        let builder = AncestryBuilder::new(tree);
        let ancestry = builder.ancestry(ProcessId(5));
        assert!(ancestry.chain.is_empty());
    }

    #[test]
    fn depth_limit_is_enforced() {
        // A linear chain longer than the cap: each process is its parent's
        // child with a distinct PID.
        let mut tree = Vec::new();
        for i in 0..(MAX_ANCESTRY_DEPTH + 16) {
            tree.push(process(i as u32, Some(i as u32 + 1), "chain.exe"));
        }
        let builder = AncestryBuilder::new(tree);
        let ancestry = builder.ancestry(ProcessId(0));
        assert!(ancestry.chain.len() <= MAX_ANCESTRY_DEPTH);
    }

    #[test]
    fn pid_zero_parent_does_not_insert_placeholder() {
        // If a process has parent_pid = Some(ProcessId(0)), the chain should
        // stop without inserting "<0>".
        let tree = vec![process(13, Some(0), "node.exe")];
        let builder = AncestryBuilder::new(tree);
        let ancestry = builder.ancestry(ProcessId(13));
        assert!(ancestry.chain.is_empty());
    }
}
