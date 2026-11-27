//! Non-interactive commands: `list`, `inspect`, `kill`.

use std::io::Write;

use portspy_core::domain::listener::Listener;
use portspy_core::domain::protocol::Protocol;
use portspy_core::error::PortSpyError;
use portspy_core::platform::{Platform, PlatformError};
use portspy_core::processes::stop::{StopPlan, StopReviewError};
use portspy_core::snapshot::builder::SnapshotBuilder;
use portspy_core::snapshot::query;
use portspy_tui::format::{address as fmt_address, duration as fmt_duration, memory as fmt_memory};

use crate::output::{ListenerJson, Output};

/// Renders the `list` command.
///
/// # Errors
///
/// Returns when the snapshot cannot be captured.
pub fn list<P: Platform + ?Sized>(
    builder: &mut SnapshotBuilder<'_, P>,
    json: bool,
) -> Result<(), PortSpyError> {
    let snapshot = builder.snapshot()?;
    let listeners = query::sort_listeners(
        &snapshot.listeners,
        query::SortField::Port,
        query::SortOrder::Ascending,
    );
    let mut out = std::io::stdout().lock();

    if json {
        let output = Output::List {
            listeners: listeners
                .iter()
                .map(|listener| ListenerJson::from(*listener))
                .collect(),
        };
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&output).map_err(json_error)?
        )
        .map_err(io_error)?;
        return Ok(());
    }

    writeln!(
        out,
        "PORT     APPLICATION / SERVICE   ORIGIN            PID        MEMORY   UPTIME"
    )
    .map_err(io_error)?;
    for listener in listeners {
        writeln!(out, "{}", table_row(listener, &snapshot)).map_err(io_error)?;
    }
    Ok(())
}

/// Renders the `inspect` command.
///
/// # Errors
///
/// Returns when the snapshot cannot be captured.
pub fn inspect<P: Platform + ?Sized>(
    builder: &mut SnapshotBuilder<'_, P>,
    port: u16,
    protocol: Option<Protocol>,
    json: bool,
) -> Result<(), PortSpyError> {
    let snapshot = builder.snapshot()?;
    let listener = match protocol {
        Some(protocol) => query::find_by_port_and_protocol(&snapshot.listeners, port, protocol),
        None => query::find_by_port(&snapshot.listeners, port),
    };
    let mut out = std::io::stdout().lock();

    if json {
        let output = Output::Inspect {
            listener: listener.map(ListenerJson::from),
        };
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&output).map_err(json_error)?
        )
        .map_err(io_error)?;
        return Ok(());
    }

    match listener {
        Some(listener) => print_details(&mut out, listener, &snapshot).map_err(io_error)?,
        None => writeln!(out, "no listener on port {port}").map_err(io_error)?,
    }
    Ok(())
}

/// Renders the `kill` command.
///
/// With `tree` the owning process and every descendant are terminated.
///
/// # Errors
///
/// Returns when the scope cannot be reviewed or has changed, or when the
/// platform cannot stop the reviewed process.
pub fn kill<P: Platform + ?Sized>(
    builder: &mut SnapshotBuilder<'_, P>,
    platform: &P,
    port: u16,
    protocol: Option<Protocol>,
    tree: bool,
    assume_yes: bool,
) -> Result<(), PortSpyError> {
    let snapshot = builder.snapshot()?;
    let listener = match protocol {
        Some(protocol) => query::find_by_port_and_protocol(&snapshot.listeners, port, protocol),
        None => query::find_by_port(&snapshot.listeners, port),
    };
    let Some(listener) = listener else {
        writeln!(std::io::stdout().lock(), "no listener on port {port}").map_err(io_error)?;
        return Ok(());
    };
    let plan = StopPlan::review(&snapshot, listener.id, tree).map_err(|error| match error {
        StopReviewError::ListenerMissing => PortSpyError::ListenerNotFound(port),
        StopReviewError::IdentityUnavailable(pid) => PlatformError::IdentityChanged { pid }.into(),
    })?;
    let mut out = std::io::stdout().lock();

    if !assume_yes {
        let scope = if plan.descendants.is_empty() {
            String::new()
        } else {
            format!(" and {} descendant(s)", plan.descendants.len())
        };
        writeln!(
            out,
            "kill {} :{}{} (PID {})? [y/N]",
            listener.application.label,
            listener.port,
            scope,
            listener.pid()
        )
        .map_err(io_error)?;
        out.flush().map_err(io_error)?;
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(|e| PortSpyError::Internal(e.to_string()))?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            writeln!(out, "aborted").map_err(io_error)?;
            return Ok(());
        }
    }

    let fresh = SnapshotBuilder::new(platform).snapshot()?;
    if !plan.matches(&fresh) {
        return Err(PortSpyError::StopScopeChanged);
    }

    if tree {
        let report = plan.execute(platform);
        if report.terminated.is_empty()
            && report.already_exited.is_empty()
            && let Some((_, error)) = report.failed.first()
        {
            return Err(error.clone().into());
        }
        let mut line = format!(
            "stopped tree of {} :{} (PID {}) — {}",
            listener.application.label,
            listener.port,
            listener.pid(),
            report.summary()
        );
        if let Some((pid, error)) = report.failed.first() {
            line.push_str(&format!("; first failure at PID {pid}: {error}"));
        }
        writeln!(out, "{line}").map_err(io_error)?;
        return Ok(());
    }

    match platform.kill_checked(plan.process.identity) {
        Ok(outcome) => {
            let message = format!(
                "stopped {} :{} (PID {}) — {}",
                listener.application.label,
                listener.port,
                listener.pid(),
                outcome.as_str()
            );
            writeln!(out, "{message}").map_err(io_error)?;
        }
        Err(PlatformError::AccessDenied { pid }) => {
            writeln!(out, "could not stop PID {pid} — access denied").map_err(io_error)?;
        }
        Err(e) => return Err(PortSpyError::Platform(e)),
    }
    Ok(())
}

fn table_row(listener: &Listener, snapshot: &portspy_core::snapshot::SystemSnapshot) -> String {
    format!(
        "{:<9} {:<23} {:<17} {:<10} {:<9} {}",
        fmt_address::short(listener.address, listener.port),
        listener.owner_label(),
        listener.origin.label,
        listener.pid(),
        fmt_memory::memory(listener.process.memory_bytes),
        fmt_duration::uptime(listener.process.started_at, snapshot.wall_time),
    )
}

fn print_details(
    out: &mut impl Write,
    listener: &Listener,
    snapshot: &portspy_core::snapshot::SystemSnapshot,
) -> std::io::Result<()> {
    writeln!(
        out,
        "Port        {}",
        fmt_address::short(listener.address, listener.port)
    )?;
    if listener.pid().as_u32() == 0 {
        writeln!(
            out,
            "Binding     {}",
            fmt_address::full(listener.address, listener.port)
        )?;
        if let Some(service) = listener.socket_service() {
            writeln!(out, "Service     {service}")?;
        }
        if let Some(uid) = listener
            .socket_attribution
            .as_ref()
            .and_then(|context| context.uid)
        {
            writeln!(out, "Socket UID  {uid}")?;
        }
        return writeln!(out, "PID         unavailable");
    }
    writeln!(out, "Application {}", listener.application.label)?;
    writeln!(out, "Origin      {}", listener.origin.label)?;
    writeln!(out, "Project     {}", listener.project.label())?;
    if listener.is_duplicate {
        writeln!(out, "Flags       duplicate dev server")?;
    }
    if listener.is_orphan {
        writeln!(out, "Flags       orphan (parent process gone)")?;
    }
    writeln!(out, "PID         {}", listener.pid())?;
    writeln!(
        out,
        "Memory      {}",
        fmt_memory::memory(listener.process.memory_bytes)
    )?;
    writeln!(
        out,
        "Uptime      {}",
        fmt_duration::uptime(listener.process.started_at, snapshot.wall_time)
    )?;
    writeln!(out, "Scope       {}", fmt_address::scope(listener.address))?;
    if let Some(command_line) = &listener.process.command_line {
        writeln!(out, "Command     {command_line}")?;
    }
    if let Some(directory) = &listener.process.working_directory {
        writeln!(out, "Directory   {}", directory.display())?;
    }
    writeln!(out, "Process chain")?;
    writeln!(out, "  {}", listener.process.name)?;
    for ancestor in &listener.process.ancestors {
        writeln!(out, "  └─ {}", ancestor.name)?;
    }
    Ok(())
}

fn json_error(e: serde_json::Error) -> PortSpyError {
    PortSpyError::Internal(format!("json serialization failed: {e}"))
}

fn io_error(e: std::io::Error) -> PortSpyError {
    PortSpyError::Internal(format!("stdout write failed: {e}"))
}

#[cfg(test)]
mod kill_tests {
    #![allow(clippy::expect_used)]

    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;
    use portspy_core::domain::process::{ProcessId, ProcessIdentity};
    use portspy_core::platform::{KillOutcome, RawListener, RawProcess};

    struct FakePlatform {
        listener: RawListener,
        snapshots: AtomicUsize,
        change_identity: bool,
        stopped: Mutex<Vec<ProcessIdentity>>,
    }

    impl FakePlatform {
        fn new(address: std::net::SocketAddr, change_identity: bool) -> Self {
            Self {
                listener: RawListener {
                    protocol: Protocol::Tcp,
                    address: address.ip(),
                    port: address.port(),
                    pid: ProcessId(10),
                    socket_attribution: None,
                },
                snapshots: AtomicUsize::new(0),
                change_identity,
                stopped: Mutex::new(Vec::new()),
            }
        }
    }

    impl Platform for FakePlatform {
        fn listeners(&self) -> Result<Vec<RawListener>, PlatformError> {
            Ok(vec![self.listener.clone()])
        }

        fn processes(&self) -> Result<Vec<RawProcess>, PlatformError> {
            let scan = self.snapshots.fetch_add(1, Ordering::Relaxed);
            let root_started = if self.change_identity && scan > 0 {
                20
            } else {
                1
            };
            let process = |pid, parent_pid: Option<u32>, start| RawProcess {
                pid: ProcessId(pid),
                parent_pid: parent_pid.map(ProcessId),
                name: format!("test-process-{pid}"),
                executable: None,
                command_line: None,
                working_directory: None,
                started_at: Some(UNIX_EPOCH + Duration::from_secs(start)),
                memory_bytes: None,
                cpu_percent: None,
            };
            Ok(vec![
                process(10, None, root_started),
                process(11, Some(10), 2),
            ])
        }

        fn kill(&self, _: ProcessId) -> Result<KillOutcome, PlatformError> {
            unreachable!("raw PID termination")
        }

        fn kill_checked(&self, identity: ProcessIdentity) -> Result<KillOutcome, PlatformError> {
            self.stopped.lock().expect("lock").push(identity);
            Ok(KillOutcome::Terminated)
        }
    }

    #[test]
    fn cli_stop_signals_only_reviewed_processes() -> anyhow::Result<()> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let address = socket.local_addr()?;
        let root = ProcessIdentity::new(ProcessId(10), Some(UNIX_EPOCH + Duration::from_secs(1)));
        let child = ProcessIdentity::new(ProcessId(11), Some(UNIX_EPOCH + Duration::from_secs(2)));

        for (tree, expected) in [(false, vec![root]), (true, vec![child, root])] {
            let platform = FakePlatform::new(address, false);
            let mut builder = SnapshotBuilder::new(&platform);
            kill(&mut builder, &platform, address.port(), None, tree, true)?;
            assert_eq!(*platform.stopped.lock().expect("lock"), expected);
        }
        Ok(())
    }

    #[test]
    fn cli_stop_rejects_a_reused_pid_during_review() -> anyhow::Result<()> {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let address = socket.local_addr()?;
        for tree in [false, true] {
            let platform = FakePlatform::new(address, true);
            let mut builder = SnapshotBuilder::new(&platform);

            let result = kill(&mut builder, &platform, address.port(), None, tree, true);

            assert!(matches!(result, Err(PortSpyError::StopScopeChanged)));
            assert!(platform.stopped.lock().expect("lock").is_empty());
        }
        Ok(())
    }
}
