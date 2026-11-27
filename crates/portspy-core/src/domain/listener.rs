//! Listener: a local listening socket with its owning process and inference.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::IpAddr;
use std::time::SystemTime;

use super::application::ApplicationInfo;
use super::origin::OriginInfo;
use super::process::{ProcessId, ProcessInfo};
use super::project::ProjectInfo;
use super::protocol::Protocol;

/// Stable identity of a listener.
///
/// A listener is identified by what makes it unique while it lives:
///
/// ```text
/// protocol + local address + port + pid + process start time
/// ```
///
/// The PID and its creation time are included because one process can own
/// several ports and a PID can be recycled after a restart. This identity is
/// what the TUI uses to keep the selection stable across refreshes — never a
/// row index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ListenerId {
    pub protocol: Protocol,
    pub address: IpAddr,
    pub port: u16,
    pub pid: ProcessId,
    pub started_at: Option<SystemTime>,
}

impl ListenerId {
    /// Creates a new listener identity.
    pub const fn new(
        protocol: Protocol,
        address: IpAddr,
        port: u16,
        pid: ProcessId,
        started_at: Option<SystemTime>,
    ) -> Self {
        Self {
            protocol,
            address,
            port,
            pid,
            started_at,
        }
    }
}

impl fmt::Display for ListenerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ":{}", self.port)
    }
}

/// Kernel-reported socket context when its owning PID is inaccessible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocketAttribution {
    pub uid: Option<u32>,
    /// Service unit associated with the socket's cgroup.
    pub service: Option<String>,
}

/// A local listener enriched with process facts and inferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Listener {
    pub id: ListenerId,
    pub protocol: Protocol,
    pub address: IpAddr,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_attribution: Option<SocketAttribution>,
    /// Factual process information.
    pub process: ProcessInfo,
    /// Inferred application.
    pub application: ApplicationInfo,
    /// Inferred origin.
    pub origin: OriginInfo,
    /// Inferred project membership.
    pub project: ProjectInfo,
    /// Whether this listener is one of several dev servers of the same
    /// project (see `detection::duplicates`).
    pub is_duplicate: bool,
    /// Whether the owning process outlived its parent (see
    /// `detection::orphan`).
    pub is_orphan: bool,
}

impl Listener {
    /// The service unit associated with an unattributed socket, when available.
    pub fn socket_service(&self) -> Option<&str> {
        self.socket_attribution.as_ref()?.service.as_deref()
    }

    /// The application name, or the best available context for an unattributed socket.
    pub fn owner_label(&self) -> std::borrow::Cow<'_, str> {
        if self.pid().as_u32() != 0 {
            return self.application.label.as_str().into();
        }
        if let Some(service) = self.socket_service() {
            return service.into();
        }
        if let Some(uid) = self
            .socket_attribution
            .as_ref()
            .and_then(|context| context.uid)
        {
            return format!("UID {uid}").into();
        }
        "Unknown owner".into()
    }

    /// The PID owning this listener.
    pub fn pid(&self) -> ProcessId {
        self.process.pid
    }
}
