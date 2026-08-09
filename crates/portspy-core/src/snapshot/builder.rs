//! Snapshot construction pipeline.
//!
//! ```text
//! scan listeners
//!         ↓
//! collect process metadata (bulk)
//!         ↓
//! resolve ancestry
//!         ↓
//! detect application
//!         ↓
//! detect origin
//!         ↓
//! SystemSnapshot
//! ```

use std::collections::HashMap;

use crate::detection::application::ApplicationDetector;
use crate::detection::origin::OriginDetector;
use crate::detection::DetectionContext;
use crate::domain::listener::{Listener, ListenerId};
use crate::domain::process::{ProcessId, ProcessInfo};
use crate::error::PortSpyError;
use crate::platform::Platform;
use crate::processes::ancestry::AncestryBuilder;
use crate::snapshot::SystemSnapshot;

/// Collects and enriches one system snapshot.
///
/// The builder is stateless between calls: every [`snapshot`](Self::snapshot)
/// invocation starts from the operating system again. A background collector
/// owns a builder and feeds snapshots to the TUI through a channel.
pub struct SnapshotBuilder<'p, P: ?Sized> {
    platform: &'p P,
    origin_detector: OriginDetector,
    application_detector: ApplicationDetector,
}

impl<'p, P: Platform + ?Sized> SnapshotBuilder<'p, P> {
    /// Creates a builder over a platform implementation.
    pub fn new(platform: &'p P) -> Self {
        Self {
            platform,
            origin_detector: OriginDetector::new(),
            application_detector: ApplicationDetector::new(),
        }
    }

    /// Captures the current system state.
    ///
    /// # Errors
    ///
    /// Returns [`PortSpyError::Platform`] when the underlying operating
    /// system calls fail. Partial process information is *not* an error:
    /// processes that disappeared mid-collection simply carry less metadata.
    pub fn snapshot(&self) -> Result<SystemSnapshot, PortSpyError> {
        let wall_time = std::time::SystemTime::now();
        let captured_at = std::time::Instant::now();

        let raw_listeners = self.platform.listeners()?;
        let raw_processes = self.platform.processes()?;

        let mut processes: HashMap<ProcessId, ProcessInfo> = HashMap::with_capacity(
            raw_processes.len(),
        );
        for raw in raw_processes {
            processes.insert(raw.pid, raw.into());
        }

        let ancestry_builder = AncestryBuilder::new(processes.values().cloned());
        let context = DetectionContext::new(&processes, &ancestry_builder);

        let mut listeners: Vec<Listener> = raw_listeners
            .into_iter()
            .map(|raw| {
                let mut process = processes
                    .get(&raw.pid)
                    .cloned()
                    .unwrap_or_else(|| missing_process(raw.pid));
                let ancestry = context.ancestry(raw.pid);
                process.ancestors = ancestry.chain.clone();
                let origin = self
                    .origin_detector
                    .detect(&context, &process, &ancestry);
                let application = self.application_detector.detect(&process);

                Listener {
                    id: ListenerId::new(
                        raw.protocol,
                        raw.address,
                        raw.port,
                        raw.pid,
                        process.started_at,
                    ),
                    protocol: raw.protocol,
                    address: raw.address,
                    port: raw.port,
                    process,
                    application,
                    origin,
                }
            })
            .collect();

        listeners.sort_by_key(|l| (l.port, l.address));
        listeners.dedup_by_key(|l| l.id);

        Ok(SystemSnapshot { captured_at, wall_time, listeners })
    }
}

/// Placeholder facts for a listener whose process disappeared between the
/// port scan and the process snapshot. The listener itself is still real.
fn missing_process(pid: ProcessId) -> ProcessInfo {
    ProcessInfo {
        pid,
        parent_pid: None,
        name: "<exited>".to_owned(),
        executable: None,
        command_line: None,
        working_directory: None,
        started_at: None,
        memory_bytes: None,
        cpu_percent: None,
        ancestors: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::SnapshotBuilder;
    use crate::domain::process::ProcessId;
    use crate::platform::{KillOutcome, Platform, PlatformError, RawListener, RawProcess};
    use crate::PortSpyError;
    use std::net::{IpAddr, Ipv4Addr};

    struct FakePlatform {
        listeners: Vec<RawListener>,
        processes: Vec<RawProcess>,
    }

    impl Platform for FakePlatform {
        fn listeners(&self) -> Result<Vec<RawListener>, PlatformError> {
            Ok(self.listeners.clone())
        }

        fn processes(&self) -> Result<Vec<RawProcess>, PlatformError> {
            Ok(self.processes.clone())
        }

        fn kill(&self, _pid: ProcessId) -> Result<KillOutcome, PlatformError> {
            Ok(KillOutcome::Terminated)
        }
    }

    fn raw_listener(port: u16, pid: u32) -> RawListener {
        RawListener {
            protocol: crate::domain::protocol::Protocol::Tcp,
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
            pid: ProcessId(pid),
        }
    }

    fn raw_process(pid: u32, name: &str) -> RawProcess {
        RawProcess {
            pid: ProcessId(pid),
            parent_pid: None,
            name: name.to_owned(),
            executable: None,
            command_line: None,
            working_directory: None,
            started_at: None,
            memory_bytes: Some(1024),
            cpu_percent: None,
        }
    }

    #[test]
    fn snapshot_enriches_listeners() {
        let platform = FakePlatform {
            listeners: vec![raw_listener(3000, 42)],
            processes: vec![raw_process(42, "node.exe")],
        };
        let builder = SnapshotBuilder::new(&platform);
        let snapshot = builder.snapshot().expect("snapshot should succeed");
        assert_eq!(snapshot.listeners.len(), 1);
        let listener = &snapshot.listeners[0];
        assert_eq!(listener.port, 3000);
        assert_eq!(listener.pid(), ProcessId(42));
        assert_eq!(listener.process.name, "node.exe");
        assert_eq!(listener.process.memory_bytes, Some(1024));
    }

    #[test]
    fn listener_without_process_still_appears() {
        let platform = FakePlatform {
            listeners: vec![raw_listener(3000, 999)],
            processes: vec![],
        };
        let builder = SnapshotBuilder::new(&platform);
        let snapshot = builder.snapshot().expect("snapshot should succeed");
        assert_eq!(snapshot.listeners.len(), 1);
        assert_eq!(snapshot.listeners[0].process.name, "<exited>");
    }

    #[test]
    fn snapshot_sorts_by_port_then_address() {
        let platform = FakePlatform {
            listeners: vec![raw_listener(8080, 1), raw_listener(3000, 2), raw_listener(443, 3)],
            processes: vec![raw_process(1, "a.exe"), raw_process(2, "b.exe"), raw_process(3, "c.exe")],
        };
        let builder = SnapshotBuilder::new(&platform);
        let snapshot = builder.snapshot().expect("snapshot should succeed");
        let ports: Vec<u16> = snapshot.listeners.iter().map(|l| l.port).collect();
        assert_eq!(ports, [443, 3000, 8080]);
    }

    #[test]
    fn platform_failure_propagates() {
        struct BrokenPlatform;
        impl Platform for BrokenPlatform {
            fn listeners(&self) -> Result<Vec<RawListener>, PlatformError> {
                Err(PlatformError::Unsupported("no platform"))
            }
            fn processes(&self) -> Result<Vec<RawProcess>, PlatformError> {
                Ok(vec![])
            }
            fn kill(&self, _pid: ProcessId) -> Result<KillOutcome, PlatformError> {
                Ok(KillOutcome::Terminated)
            }
        }
        let builder = SnapshotBuilder::new(&BrokenPlatform);
        let err = builder.snapshot().expect_err("must fail");
        assert!(matches!(err, PortSpyError::Platform(_)));
    }
}
