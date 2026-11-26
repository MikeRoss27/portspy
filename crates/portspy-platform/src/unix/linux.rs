//! Linux socket and process discovery through procfs.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::net::SocketAddr;
use std::os::fd::AsRawFd;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use portspy_core::domain::process::{ProcessId, ProcessIdentity};
use portspy_core::domain::protocol::Protocol;
use portspy_core::platform::{KillOutcome, PlatformError, RawListener, RawProcess};
use procfs::net::{self, TcpState};
use procfs::process::{FDTarget, Process, all_processes};
use procfs::{ProcError, ProcResult};
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};

pub(super) fn listeners() -> Result<Vec<RawListener>, PlatformError> {
    let tcp = net::tcp().map_err(discovery_error)?;
    let tcp6 = optional_table(net::tcp6())?;
    let udp = net::udp().map_err(discovery_error)?;
    let udp6 = optional_table(net::udp6())?;
    let sockets: Vec<_> = tcp
        .into_iter()
        .chain(tcp6)
        .filter(|row| row.state == TcpState::Listen)
        .map(|row| (row.inode, Protocol::Tcp, row.local_address))
        .chain(
            udp.into_iter()
                .chain(udp6)
                .map(|row| (row.inode, Protocol::Udp, row.local_address)),
        )
        .filter(|(_, _, address)| address.port() != 0)
        .collect();
    let mut owners: HashMap<_, HashSet<_>> = sockets
        .iter()
        .map(|(inode, _, _)| (*inode, HashSet::new()))
        .collect();

    for process in all_processes().map_err(discovery_error)?.flatten() {
        let Ok(pid) = u32::try_from(process.pid()) else {
            continue;
        };
        let Ok(descriptors) = process.fd() else {
            continue;
        };
        for descriptor in descriptors.flatten() {
            if let FDTarget::Socket(inode) = descriptor.target
                && let Some(pids) = owners.get_mut(&inode)
            {
                pids.insert(ProcessId(pid));
            }
        }
    }

    let mut seen = HashSet::new();
    let mut listeners = Vec::new();
    for (inode, protocol, address) in sockets {
        let pids = owners.get(&inode).into_iter().flatten();
        if pids.clone().next().is_none() {
            push_listener(&mut seen, &mut listeners, protocol, address, ProcessId(0));
        }
        for &pid in pids {
            push_listener(&mut seen, &mut listeners, protocol, address, pid);
        }
    }
    Ok(listeners)
}

fn push_listener(
    seen: &mut HashSet<(Protocol, SocketAddr, ProcessId)>,
    listeners: &mut Vec<RawListener>,
    protocol: Protocol,
    address: SocketAddr,
    pid: ProcessId,
) {
    if seen.insert((protocol, address, pid)) {
        listeners.push(RawListener {
            protocol,
            address: address.ip(),
            port: address.port(),
            pid,
        });
    }
}

fn optional_table<T>(result: ProcResult<Vec<T>>) -> Result<Vec<T>, PlatformError> {
    match result {
        Ok(rows) => Ok(rows),
        Err(ProcError::NotFound(_)) => Ok(Vec::new()),
        Err(error) => Err(discovery_error(error)),
    }
}

fn discovery_error(error: ProcError) -> PlatformError {
    PlatformError::OperationFailed {
        operation: "socket discovery",
        message: error.to_string(),
    }
}

pub(super) fn processes() -> Result<Vec<RawProcess>, PlatformError> {
    let processes = all_processes().map_err(|error| PlatformError::OperationFailed {
        operation: "process discovery",
        message: error.to_string(),
    })?;
    Ok(processes
        .flatten()
        .filter_map(|process| {
            // All reads are relative to this open procfs directory descriptor,
            // which cannot resolve to a replacement after the PID is recycled.
            let stat = process.stat().ok()?;
            let pid = ProcessId(u32::try_from(stat.pid).ok()?);
            let parent_pid = u32::try_from(stat.ppid).ok().filter(|pid| *pid > 0);
            Some(RawProcess {
                pid,
                parent_pid: parent_pid.map(ProcessId),
                name: stat.comm,
                executable: process.exe().ok(),
                command_line: command_line(&process),
                working_directory: process.cwd().ok(),
                started_at: creation_time(stat.starttime),
                memory_bytes: stat.rss.checked_mul(procfs::page_size()),
                cpu_percent: None,
            })
        })
        .collect())
}

fn command_line(process: &Process) -> Option<String> {
    let mut bytes = Vec::new();
    process
        .open_relative("cmdline")
        .ok()?
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.last() == Some(&0) {
        bytes.pop();
    }
    (!bytes.is_empty()).then(|| String::from_utf8_lossy(&bytes).replace('\0', " "))
}

fn creation_time(start_ticks: u64) -> Option<SystemTime> {
    // Use one epoch for all collectors and stops in this invocation. The
    // kernel's boot-relative start ticks remain unchanged across clock changes.
    static BOOT_EPOCH: OnceLock<u64> = OnceLock::new();
    let boot_secs = match BOOT_EPOCH.get() {
        Some(boot_secs) => *boot_secs,
        None => {
            let observed = procfs::boot_time_secs().ok()?;
            *BOOT_EPOCH.get_or_init(|| observed)
        }
    };
    let rate = procfs::ticks_per_second();
    let elapsed_secs = start_ticks.checked_div(rate)?;
    let nanos = (start_ticks % rate).checked_mul(1_000_000_000)? / rate;
    UNIX_EPOCH.checked_add(Duration::new(
        boot_secs.checked_add(elapsed_secs)?,
        nanos as u32,
    ))
}

pub(super) fn kill_checked(
    target: Pid,
    identity: ProcessIdentity,
) -> Result<KillOutcome, PlatformError> {
    let pid = identity.pid;
    let process = match Process::new(target.as_raw_pid()) {
        Ok(process) => process,
        Err(ProcError::NotFound(_)) => return Ok(KillOutcome::AlreadyExited),
        Err(ProcError::PermissionDenied(_)) => return Err(PlatformError::AccessDenied { pid }),
        Err(_) => return Err(PlatformError::IdentityChanged { pid }),
    };
    let handle = match pidfd_open(target, PidfdFlags::empty()) {
        Ok(handle) => handle,
        Err(error) => return super::signal_outcome(pid, Err(error)),
    };
    // Unlike pidfd_open, fdinfo reports the PID in the procfs mount's PID
    // namespace, which may differ from the caller's namespace in containers.
    let fdinfo = std::fs::read_to_string(format!("/proc/self/fdinfo/{}", handle.as_raw_fd()))
        .map_err(|_| PlatformError::IdentityChanged { pid })?;
    let bound_pid = fdinfo.lines().find_map(|line| {
        line.strip_prefix("Pid:")
            .and_then(|value| value.trim().parse::<i32>().ok())
    });
    match bound_pid {
        Some(-1) => return Ok(KillOutcome::AlreadyExited),
        Some(bound_pid) if bound_pid == target.as_raw_pid() => {}
        _ => return Err(PlatformError::IdentityChanged { pid }),
    }
    // Verify that the original procfs handle still resolves after opening the
    // pidfd. The subsequent signal is sent through the pidfd, never the PID.
    let started_at = process
        .stat()
        .ok()
        .and_then(|stat| creation_time(stat.starttime));
    if started_at != identity.started_at {
        return Err(PlatformError::IdentityChanged { pid });
    }
    super::signal_outcome(pid, pidfd_send_signal(handle, Signal::KILL))
}
