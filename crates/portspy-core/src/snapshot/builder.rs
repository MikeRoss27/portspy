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
//! detect project + orphan (per listener)
//!         ↓
//! mark duplicates (across listeners)
//!         ↓
//! SystemSnapshot
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

use crate::detection::DetectionContext;
use crate::detection::application::ApplicationDetector;
use crate::detection::origin::OriginDetector;
use crate::detection::project::ProjectDetector;
use crate::domain::listener::{Listener, ListenerId};
use crate::domain::process::{ProcessId, ProcessInfo};
use crate::error::PortSpyError;
use crate::platform::Platform;
use crate::processes::ancestry::AncestryBuilder;
use crate::snapshot::SystemSnapshot;

/// Maximum age of a cached process entry before it's considered stale.
const CACHE_TTL: Duration = Duration::from_secs(30);

/// Cache key: PID + process start time (handles PID recycling).
type CacheKey = (ProcessId, Option<SystemTime>);

/// Cached process metadata with timestamp.
#[derive(Debug, Clone)]
struct CachedProcess {
    info: ProcessInfo,
    cached_at: Instant,
}

/// Collects and enriches one system snapshot.
///
/// The builder caches process metadata keyed by (PID, start_time) to avoid
/// re-querying the OS for processes that haven't changed between snapshots.
/// The cache has a TTL to handle process restarts and PID recycling.
pub struct SnapshotBuilder<'p, P: ?Sized> {
    platform: &'p P,
    origin_detector: OriginDetector,
    application_detector: ApplicationDetector,
    project_detector: ProjectDetector,
    process_cache: HashMap<CacheKey, CachedProcess>,
}

impl<'p, P: Platform + ?Sized> SnapshotBuilder<'p, P> {
    /// Creates a builder over a platform implementation.
    pub fn new(platform: &'p P) -> Self {
        Self {
            platform,
            origin_detector: OriginDetector::new(),
            application_detector: ApplicationDetector::new(),
            project_detector: ProjectDetector::new(),
            process_cache: HashMap::new(),
        }
    }

    /// Captures the current system state.
    ///
    /// # Errors
    ///
    /// Returns [`PortSpyError::Platform`] when the underlying operating
    /// system calls fail. Partial process information is *not* an error:
    /// processes that disappeared mid-collection simply carry less metadata.
    pub fn snapshot(&mut self) -> Result<SystemSnapshot, PortSpyError> {
        let wall_time = std::time::SystemTime::now();
        let captured_at = std::time::Instant::now();

        let raw_listeners = self.platform.listeners()?;
        let raw_processes = self.platform.processes()?;

        let mut processes: HashMap<ProcessId, ProcessInfo> =
            HashMap::with_capacity(raw_processes.len());

        // Populate processes from cache or fresh collection
        let now = Instant::now();
        for raw in raw_processes {
            let key = (raw.pid, raw.started_at);

            // Check cache first
            if let Some(cached) = self.process_cache.get(&key)
                && now.duration_since(cached.cached_at) < CACHE_TTL
            {
                processes.insert(raw.pid, cached.info.clone());
                continue;
            }

            // Cache miss or stale - collect fresh metadata
            let process_info: ProcessInfo = raw.into();
            processes.insert(process_info.pid, process_info.clone());

            // Update cache
            self.process_cache.insert(
                key,
                CachedProcess {
                    info: process_info,
                    cached_at: now,
                },
            );
        }

        // Evict stale cache entries
        self.process_cache
            .retain(|_, v| now.duration_since(v.cached_at) < CACHE_TTL);

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
                let origin = self.origin_detector.detect(&context, &process, &ancestry);
                let application = self.application_detector.detect(&process);
                let project = self.project_detector.detect(&process);
                let is_orphan = crate::detection::orphan::is_orphan(&process, &context);

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
                    project,
                    is_duplicate: false,
                    is_orphan,
                }
            })
            .collect();

        listeners.sort_by_key(|l| (l.port, l.address, l.protocol, l.pid()));
        listeners.dedup_by_key(|l| l.id);
        crate::detection::duplicates::mark_duplicates(&mut listeners);

        let mut processes: Vec<ProcessInfo> = processes.into_values().collect();
        processes.sort_by_key(|p| p.pid);

        Ok(SystemSnapshot {
            captured_at,
            wall_time,
            listeners,
            processes,
        })
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
    use crate::PortSpyError;
    use crate::domain::process::ProcessId;
    use crate::platform::{KillOutcome, Platform, PlatformError, RawListener, RawProcess};
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

    fn dev_process(pid: u32, parent: Option<u32>, cwd: &str) -> RawProcess {
        RawProcess {
            pid: ProcessId(pid),
            parent_pid: parent.map(ProcessId),
            name: "node.exe".to_owned(),
            executable: None,
            command_line: Some("node next dev".to_owned()),
            working_directory: Some(std::path::PathBuf::from(cwd)),
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
        let mut builder = SnapshotBuilder::new(&platform);
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
        let mut builder = SnapshotBuilder::new(&platform);
        let snapshot = builder.snapshot().expect("snapshot should succeed");
        assert_eq!(snapshot.listeners.len(), 1);
        assert_eq!(snapshot.listeners[0].process.name, "<exited>");
    }

    #[test]
    fn snapshot_sorts_by_port_then_address() {
        let platform = FakePlatform {
            listeners: vec![
                raw_listener(8080, 1),
                raw_listener(3000, 2),
                raw_listener(443, 3),
            ],
            processes: vec![
                raw_process(1, "a.exe"),
                raw_process(2, "b.exe"),
                raw_process(3, "c.exe"),
            ],
        };
        let mut builder = SnapshotBuilder::new(&platform);
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
        let mut builder = SnapshotBuilder::new(&BrokenPlatform);
        let err = builder.snapshot().expect_err("must fail");
        assert!(matches!(err, PortSpyError::Platform(_)));
    }

    #[test]
    fn project_is_detected_from_working_directory() {
        let platform = FakePlatform {
            listeners: vec![raw_listener(3000, 42)],
            processes: vec![dev_process(42, None, r"D:\dev\bengal")],
        };
        let mut builder = SnapshotBuilder::new(&platform);
        let snapshot = builder.snapshot().expect("snapshot should succeed");
        let listener = &snapshot.listeners[0];
        assert_eq!(
            listener.project.name(),
            Some("bengal"),
            "project should derive from the working directory"
        );
        assert_eq!(
            listener.application.kind,
            crate::domain::application::ApplicationKind::NextJs
        );
    }

    #[test]
    fn orphan_flag_is_set_when_parent_is_gone() {
        let platform = FakePlatform {
            listeners: vec![raw_listener(3000, 42)],
            processes: vec![dev_process(42, Some(7), r"D:\dev\bengal")],
        };
        let mut builder = SnapshotBuilder::new(&platform);
        let snapshot = builder.snapshot().expect("snapshot should succeed");
        assert!(
            snapshot.listeners[0].is_orphan,
            "parent PID 7 is not in the process table"
        );
    }

    #[test]
    fn living_parent_is_not_orphan() {
        let platform = FakePlatform {
            listeners: vec![raw_listener(3000, 42)],
            processes: vec![
                dev_process(42, Some(7), r"D:\dev\bengal"),
                raw_process(7, "pwsh.exe"),
            ],
        };
        let mut builder = SnapshotBuilder::new(&platform);
        let snapshot = builder.snapshot().expect("snapshot should succeed");
        assert!(!snapshot.listeners[0].is_orphan);
    }

    #[test]
    fn duplicates_are_marked_across_listeners() {
        let platform = FakePlatform {
            listeners: vec![raw_listener(3000, 42), raw_listener(3001, 43)],
            processes: vec![
                dev_process(42, None, r"D:\dev\bengal"),
                dev_process(43, None, r"D:\dev\bengal"),
            ],
        };
        let mut builder = SnapshotBuilder::new(&platform);
        let snapshot = builder.snapshot().expect("snapshot should succeed");
        assert_eq!(snapshot.listeners.len(), 2);
        assert!(
            snapshot.listeners.iter().all(|l| l.is_duplicate),
            "two dev servers of the same project must both be flagged"
        );
    }
}
