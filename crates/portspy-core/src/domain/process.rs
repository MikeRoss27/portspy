//! Process facts: what the operating system reports about a process.
//!
//! Everything in this module is factual. Nothing here is inferred.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::SystemTime;

/// Operating-system process identifier.
///
/// PIDs are recycled by Windows, so a PID alone is *not* a stable identity.
/// See [`ProcessIdentity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessId(pub u32);

impl ProcessId {
    /// Underlying OS value.
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// Stable process identity: PID plus process creation time.
///
/// Windows can recycle a PID between two snapshots. Combining the PID with
/// the process creation time makes the identity survive the common recycling
/// case: a recycled PID almost never shares both the number and the exact
/// creation instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessIdentity {
    pub pid: ProcessId,
    pub started_at: Option<SystemTime>,
}

impl ProcessIdentity {
    /// Creates an identity. `started_at` is `None` when the platform could
    /// not retrieve the creation time; identity then degrades to PID only.
    pub const fn new(pid: ProcessId, started_at: Option<SystemTime>) -> Self {
        Self { pid, started_at }
    }

    /// Whether this identity is strong enough to distinguish two processes
    /// that may share a PID.
    pub const fn is_stable(&self) -> bool {
        self.started_at.is_some()
    }
}

impl fmt::Display for ProcessIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.pid, f)
    }
}

/// One node of a reconstructed ancestor chain.
///
/// Ancestors are displayed in the TUI (`Process chain` panel) and used by the
/// detection engine. Only the information needed for those two jobs is kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessAncestor {
    pub identity: ProcessIdentity,
    pub name: String,
}

impl ProcessAncestor {
    /// Creates a new ancestor node.
    pub const fn new(identity: ProcessIdentity, name: String) -> Self {
        Self { identity, name }
    }
}

/// Factual process information collected from the operating system.
///
/// Every optional field exists because Windows regularly refuses to provide
/// the data (permissions, exited processes, elevated processes, ...).
/// Missing information is a fact about the machine, not a bug.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: ProcessId,
    pub parent_pid: Option<ProcessId>,
    /// Image base name, e.g. `node.exe`.
    pub name: String,
    pub executable: Option<PathBuf>,
    pub command_line: Option<String>,
    pub working_directory: Option<PathBuf>,
    pub started_at: Option<SystemTime>,
    /// Working set size in bytes.
    pub memory_bytes: Option<u64>,
    /// Instantaneous CPU usage in percent (0.0 - 100.0), when measurable.
    pub cpu_percent: Option<f32>,
    /// Reconstructed parent chain, leaf's parent first.
    ///
    /// Filled by the snapshot builder; empty when the parent is unknown.
    pub ancestors: Vec<ProcessAncestor>,
}

impl ProcessInfo {
    /// Stable identity of this process.
    pub fn identity(&self) -> ProcessIdentity {
        ProcessIdentity::new(self.pid, self.started_at)
    }

    /// Display name: the image name, or a placeholder when unknown.
    pub fn display_name(&self) -> &str {
        &self.name
    }
}
