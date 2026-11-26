//! Snapshot engine: atomic views of the machine state.

pub mod builder;
pub mod diff;
pub mod query;

use std::time::SystemTime;

use crate::domain::listener::Listener;
use crate::domain::process::ProcessInfo;

/// An immutable view of the system at one instant.
///
/// The TUI never assembles this information itself; it receives ready-made
/// snapshots produced by [`builder::SnapshotBuilder`].
#[derive(Debug, Clone, PartialEq)]
pub struct SystemSnapshot {
    /// Instant the snapshot was captured (monotonic, for uptime math).
    pub captured_at: std::time::Instant,
    /// Wall-clock time of capture (for display).
    pub wall_time: SystemTime,
    pub listeners: Vec<Listener>,
    /// Every process observed during collection, sorted by PID.
    ///
    /// Needed for parent→children traversal (tree kill) and future v0.2
    /// features (duplicate and orphan detection).
    pub processes: Vec<ProcessInfo>,
}

impl SystemSnapshot {
    /// Empty snapshot, used before the first collection completes.
    pub fn empty() -> Self {
        Self {
            captured_at: std::time::Instant::now(),
            wall_time: SystemTime::now(),
            listeners: Vec::new(),
            processes: Vec::new(),
        }
    }
}
