//! An exact stop preview built from a listener and its process snapshot.

use std::collections::HashSet;

use crate::domain::listener::ListenerId;
use crate::domain::process::{ProcessId, ProcessIdentity, ProcessInfo};
use crate::platform::{KillOutcome, Platform};
use crate::snapshot::SystemSnapshot;

use super::tree::{TreeKillReport, kill_order};

/// A process shown in a stop preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopProcess {
    pub identity: ProcessIdentity,
    pub name: String,
}

impl From<&ProcessInfo> for StopProcess {
    fn from(process: &ProcessInfo) -> Self {
        Self {
            identity: process.identity(),
            name: process.display_name().to_owned(),
        }
    }
}

/// The reviewed target, affected sockets and exact descendant set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopPlan {
    pub target: ListenerId,
    pub application: String,
    pub process: StopProcess,
    pub tree: bool,
    /// Reviewed descendants in termination order, deepest first.
    pub descendants: Vec<StopProcess>,
    /// Observed listeners for the target and any reviewed descendants.
    pub listeners: Vec<ListenerId>,
}

/// Why an action could not be reviewed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StopReviewError {
    #[error("the selected listener is no longer present")]
    ListenerMissing,
    #[error("the identity of PID {0} is unavailable")]
    IdentityUnavailable(ProcessId),
}

impl StopPlan {
    /// Reviews the exact scope present in a snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected listener disappeared or a process
    /// in scope has no creation time.
    pub fn review(
        snapshot: &SystemSnapshot,
        target: ListenerId,
        tree: bool,
    ) -> Result<Self, StopReviewError> {
        let listener = snapshot
            .listeners
            .iter()
            .find(|listener| listener.id == target)
            .ok_or(StopReviewError::ListenerMissing)?;
        let process = StopProcess::from(&listener.process);
        if process.identity.pid == ProcessId(0) || !process.identity.is_stable() {
            return Err(StopReviewError::IdentityUnavailable(process.identity.pid));
        }
        let mut descendants = Vec::new();
        if tree {
            for pid in kill_order(&snapshot.processes, listener.pid()) {
                if pid == listener.pid() {
                    continue;
                }
                let candidate = snapshot
                    .processes
                    .iter()
                    .find(|process| process.pid == pid)
                    .ok_or(StopReviewError::IdentityUnavailable(pid))?;
                let descendant = StopProcess::from(candidate);
                if !descendant.identity.is_stable() {
                    return Err(StopReviewError::IdentityUnavailable(pid));
                }
                descendants.push(descendant);
            }
        }
        let identities: HashSet<_> = descendants
            .iter()
            .map(|process| process.identity)
            .chain([process.identity])
            .collect();
        let mut listeners: Vec<_> = snapshot
            .listeners
            .iter()
            .filter(|listener| identities.contains(&listener.process.identity()))
            .map(|listener| listener.id)
            .collect();
        listeners.sort_unstable_by_key(|id| (id.port, id.protocol, id.address, id.pid));
        Ok(Self {
            target,
            application: listener.application.label.clone(),
            process,
            tree,
            descendants,
            listeners,
        })
    }

    /// Whether the latest snapshot still exactly matches the reviewed scope.
    pub fn matches(&self, snapshot: &SystemSnapshot) -> bool {
        Self::review(snapshot, self.target, self.tree).as_ref() == Ok(self)
    }

    /// Stops only the reviewed identities and records each outcome.
    pub fn execute(&self, platform: &(impl Platform + ?Sized)) -> TreeKillReport {
        let mut report = TreeKillReport::default();
        for process in self.descendants.iter().chain([&self.process]) {
            let identity = process.identity;
            match platform.kill_checked(identity) {
                Ok(KillOutcome::Terminated) => report.terminated.push(identity.pid),
                Ok(KillOutcome::AlreadyExited) => report.already_exited.push(identity.pid),
                Err(error) => report.failed.push((identity.pid, error)),
            }
        }
        report
    }

    /// Counts reviewed port/protocol pairs with no listener in the latest scan.
    ///
    /// An occupied port is still reported even when another process or bind
    /// address took over the numeric port for the same protocol.
    pub fn port_counts(&self, snapshot: &SystemSnapshot) -> (usize, usize) {
        let ports: HashSet<_> = self
            .listeners
            .iter()
            .map(|id| (id.protocol, id.port))
            .collect();
        let free = ports
            .iter()
            .filter(|(protocol, port)| {
                !snapshot
                    .listeners
                    .iter()
                    .any(|listener| listener.protocol == *protocol && listener.port == *port)
            })
            .count();
        (free, ports.len())
    }
}
