//! Detection engine: turns raw process facts into useful inference.
//!
//! Two independent engines live here:
//!
//! * [`origin`]: "what caused this process to exist?" (Codex, Claude Code, ...)
//! * [`application`]: "what application is this?" (Next.js, Vite, ...)
//!
//! Both engines are rule-based, declarative and fully unit-tested. No
//! detection rule may exist without a test.

pub mod application;
pub mod origin;

use std::collections::HashMap;

use crate::domain::process::{ProcessId, ProcessInfo};
use crate::processes::ancestry::{AncestryBuilder, ProcessAncestry};

/// Context shared by detection engines during one snapshot.
///
/// Provides access to the process table and prebuilt ancestry lookups so
/// every detector walks the same consistent view of the machine.
pub struct DetectionContext<'a> {
    processes: &'a HashMap<ProcessId, ProcessInfo>,
    ancestry_builder: &'a AncestryBuilder,
}

impl<'a> DetectionContext<'a> {
    /// Creates a context over a process table and its ancestry builder.
    pub const fn new(
        processes: &'a HashMap<ProcessId, ProcessInfo>,
        ancestry_builder: &'a AncestryBuilder,
    ) -> Self {
        Self { processes, ancestry_builder }
    }

    /// Looks up process facts by PID.
    pub fn process(&self, pid: ProcessId) -> Option<&ProcessInfo> {
        self.processes.get(&pid)
    }

    /// Reconstructs the ancestor chain of a process.
    pub fn ancestry(&self, pid: ProcessId) -> ProcessAncestry {
        self.ancestry_builder.ancestry(pid)
    }
}
