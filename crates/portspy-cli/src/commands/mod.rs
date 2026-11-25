//! Non-interactive commands: `list`, `inspect`, `kill`.

use std::io::Write;

use portspy_core::domain::listener::Listener;
use portspy_core::error::PortSpyError;
use portspy_core::platform::{Platform, PlatformError};
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
        "PORT     APPLICATION      ORIGIN            PID        MEMORY   UPTIME"
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
    protocol: Option<portspy_core::domain::protocol::Protocol>,
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
/// Returns when the snapshot cannot be captured or the process refuses
/// termination (single-process mode).
pub fn kill<P: Platform + ?Sized>(
    builder: &mut SnapshotBuilder<'_, P>,
    platform: &P,
    port: u16,
    protocol: Option<portspy_core::domain::protocol::Protocol>,
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
    let mut out = std::io::stdout().lock();

    if !assume_yes {
        let scope = if tree {
            let count =
                portspy_core::processes::tree::kill_order(&snapshot.processes, listener.pid())
                    .len();
            if count > 1 {
                format!(" and {} descendant(s)", count - 1)
            } else {
                String::new()
            }
        } else {
            String::new()
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
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(|e| PortSpyError::Internal(e.to_string()))?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            writeln!(out, "aborted").map_err(io_error)?;
            return Ok(());
        }
    }

    if tree {
        let report =
            portspy_core::processes::tree::kill_tree(platform, &snapshot.processes, listener.pid());
        let mut line = format!(
            "stopped tree of {} :{} (PID {}) — {}",
            listener.application.label,
            listener.port,
            listener.pid(),
            report.summary()
        );
        if let Some((pid, _)) = report.failed.first() {
            line.push_str(&format!("; first failure at PID {pid}"));
        }
        writeln!(out, "{line}").map_err(io_error)?;
        return Ok(());
    }

    match platform.kill(listener.pid()) {
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
        "{:<9} {:<17} {:<17} {:<10} {:<9} {}",
        fmt_address::short(listener.address, listener.port),
        listener.application.label,
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
