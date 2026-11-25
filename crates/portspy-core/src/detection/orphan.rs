//! Orphan process detection.
//!
//! A listener process is an *orphan* when its declared parent process is no
//! longer alive: the terminal was closed, the shell exited, or the parent was
//! killed while the process kept running. Orphan dev servers are a classic
//! source of "port already in use" surprises, so PortSpy surfaces them.

use crate::detection::DetectionContext;
use crate::domain::process::{ProcessId, ProcessInfo};

/// Whether `process` has outlived its parent.
///
/// The parent is dead when its PID is not present in the process table of
/// the snapshot. Processes without a declared parent (or with the system
/// idle PID 0) are never orphans.
pub(crate) fn is_orphan(process: &ProcessInfo, context: &DetectionContext) -> bool {
    match process.parent_pid {
        Some(pid) if pid != ProcessId(0) => context.process(pid).is_none(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::is_orphan;
    use crate::detection::DetectionContext;
    use crate::domain::process::{ProcessId, ProcessInfo};
    use crate::processes::ancestry::AncestryBuilder;
    use std::collections::HashMap;

    fn process(parent: Option<ProcessId>) -> ProcessInfo {
        ProcessInfo {
            pid: ProcessId(100),
            parent_pid: parent,
            name: "node.exe".to_owned(),
            executable: None,
            command_line: None,
            working_directory: None,
            started_at: None,
            memory_bytes: None,
            cpu_percent: None,
            ancestors: Vec::new(),
        }
    }

    fn context(alive: &[ProcessId]) -> (HashMap<ProcessId, ProcessInfo>, AncestryBuilder) {
        let processes: HashMap<ProcessId, ProcessInfo> = alive
            .iter()
            .map(|pid| (*pid, process(Some(*pid))))
            .collect();
        let ancestry = AncestryBuilder::new(processes.values().cloned());
        (processes, ancestry)
    }

    #[test]
    fn parent_missing_from_table_is_orphan() {
        let (processes, ancestry) = context(&[]);
        let ctx = DetectionContext::new(&processes, &ancestry);
        assert!(is_orphan(&process(Some(ProcessId(42))), &ctx));
    }

    #[test]
    fn living_parent_is_not_orphan() {
        let (processes, ancestry) = context(&[ProcessId(42)]);
        let ctx = DetectionContext::new(&processes, &ancestry);
        assert!(!is_orphan(&process(Some(ProcessId(42))), &ctx));
    }

    #[test]
    fn no_declared_parent_is_not_orphan() {
        let (processes, ancestry) = context(&[]);
        let ctx = DetectionContext::new(&processes, &ancestry);
        assert!(!is_orphan(&process(None), &ctx));
    }

    #[test]
    fn system_idle_parent_is_not_orphan() {
        let (processes, ancestry) = context(&[]);
        let ctx = DetectionContext::new(&processes, &ancestry);
        assert!(!is_orphan(&process(Some(ProcessId(0))), &ctx));
    }
}
