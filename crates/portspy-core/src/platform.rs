//! The platform contract.
//!
//! `portspy-core` defines what the rest of the product needs from the
//! operating system. `portspy-platform` provides the Windows implementation;
//! the CLI wires them together. The domain never calls Win32 directly.

use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::domain::process::{ProcessId, ProcessIdentity, ProcessInfo};
use crate::domain::protocol::Protocol;

/// A listening socket as reported by the operating system.
///
/// This is raw data: no enrichment, no inference, no ancestry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawListener {
    pub protocol: Protocol,
    pub address: IpAddr,
    pub port: u16,
    pub pid: ProcessId,
}

/// Process facts as reported by the operating system.
///
/// Every optional field is optional because Windows may refuse to provide
/// it. An inaccessible process is not a failure of the snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct RawProcess {
    pub pid: ProcessId,
    pub parent_pid: Option<ProcessId>,
    pub name: String,
    pub executable: Option<PathBuf>,
    pub command_line: Option<String>,
    pub working_directory: Option<PathBuf>,
    pub started_at: Option<SystemTime>,
    pub memory_bytes: Option<u64>,
    pub cpu_percent: Option<f32>,
}

/// Outcome of a kill request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillOutcome {
    /// The process was terminated by PortSpy.
    Terminated,
    /// The process had already exited; nothing was terminated.
    AlreadyExited,
}

impl KillOutcome {
    /// Human-readable description for notifications.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terminated => "terminated",
            Self::AlreadyExited => "already exited",
        }
    }
}

impl fmt::Display for KillOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A platform failure, mapped to domain vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlatformError {
    /// A native call failed with an unexpected error code.
    #[error("windows API `{api}` failed with code {code:#x}")]
    CallFailed { api: &'static str, code: u32 },
    /// The target process refused access (elevated, protected, system).
    #[error("access denied for process {pid}")]
    AccessDenied { pid: ProcessId },
    /// The process disappeared between enumeration and inspection.
    #[error("process {pid} has exited")]
    ProcessGone { pid: ProcessId },
    /// This platform has no implementation for the requested operation.
    #[error("unsupported on this platform: {0}")]
    Unsupported(&'static str),
}

/// Operating-system integration boundary.
///
/// The implementation is injected at the executable level. The Windows
/// implementation lives in `portspy-platform`; nothing else in the product
/// may talk to the OS.
///
/// Implementations must be `Send + Sync`: snapshots are built on background
/// threads, and the TUI shares one platform between its collector thread and
/// the UI thread.
pub trait Platform: Send + Sync {    /// Returns the current listening sockets.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the underlying system call fails.
    fn listeners(&self) -> Result<Vec<RawListener>, PlatformError>;

    /// Returns one snapshot of every process on the system.
    ///
    /// A bulk snapshot is preferred over per-PID queries: it is one native
    /// call instead of one per process, and it lets the core reconstruct
    /// ancestry for processes that do not own any listener.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the underlying system call fails.
    fn processes(&self) -> Result<Vec<RawProcess>, PlatformError>;

    /// Terminates a process. See [`KillOutcome`] for the semantics.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::AccessDenied`] when the process refuses
    /// termination and [`PlatformError::ProcessGone`] when it disappeared.
    fn kill(&self, pid: ProcessId) -> Result<KillOutcome, PlatformError>;

    /// Returns the full process information for one PID, when available.
    ///
    /// Used for the details view of a single listener when a full snapshot
    /// was not available.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::ProcessGone`] when the process exited.
    fn process(&self, pid: ProcessId) -> Result<Option<ProcessInfo>, PlatformError> {
        // Default implementation: look the PID up in a fresh bulk snapshot.
        // Platform implementations may provide a cheaper direct path.
        let snapshot = self.processes()?;
        Ok(snapshot.into_iter().find(|p| p.pid == pid).map(Into::into))
    }
}

impl From<RawProcess> for ProcessInfo {
    fn from(raw: RawProcess) -> Self {
        Self {
            pid: raw.pid,
            parent_pid: raw.parent_pid,
            name: raw.name,
            executable: raw.executable,
            command_line: raw.command_line,
            working_directory: raw.working_directory,
            started_at: raw.started_at,
            memory_bytes: raw.memory_bytes,
            cpu_percent: raw.cpu_percent,
            ancestors: Vec::new(),
        }
    }
}

impl From<RawProcess> for ProcessIdentity {
    fn from(raw: RawProcess) -> Self {
        ProcessIdentity::new(raw.pid, raw.started_at)
    }
}
