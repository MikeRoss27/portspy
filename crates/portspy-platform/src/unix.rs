//! Native socket and process discovery shared by Linux and macOS.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use portspy_core::domain::process::{ProcessId, ProcessIdentity};
use portspy_core::platform::{KillOutcome, Platform, PlatformError, RawListener, RawProcess};
use rustix::process::{Pid, Signal, kill_process};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[cfg_attr(target_os = "linux", path = "unix/linux.rs")]
#[cfg_attr(target_os = "macos", path = "unix/macos.rs")]
mod sockets;

pub(crate) struct UnixPlatform;

impl Platform for UnixPlatform {
    fn listeners(&self) -> Result<Vec<RawListener>, PlatformError> {
        sockets::listeners()
    }

    fn processes(&self) -> Result<Vec<RawProcess>, PlatformError> {
        let refresh = ProcessRefreshKind::nothing()
            .with_memory()
            .with_exe(UpdateKind::Always)
            .with_cwd(UpdateKind::Always)
            .with_cmd(UpdateKind::Always);
        let mut system = System::new().map_err(|error| PlatformError::OperationFailed {
            operation: "process discovery",
            message: error.to_string(),
        })?;
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh);

        Ok(system
            .processes()
            .values()
            .map(|process| {
                let command_line: Vec<_> = process
                    .cmd()
                    .iter()
                    .map(|arg| arg.to_string_lossy())
                    .collect();
                RawProcess {
                    pid: ProcessId(process.pid().as_u32()),
                    parent_pid: process.parent().map(|parent| ProcessId(parent.as_u32())),
                    name: process.name().to_string_lossy().into_owned(),
                    executable: process.exe().map(ToOwned::to_owned),
                    command_line: (!command_line.is_empty()).then(|| command_line.join(" ")),
                    working_directory: process.cwd().map(ToOwned::to_owned),
                    started_at: i32::try_from(process.pid().as_u32())
                        .ok()
                        .and_then(process_start_time),
                    memory_bytes: Some(process.memory()),
                    cpu_percent: None,
                }
            })
            .collect())
    }

    fn kill(&self, pid: ProcessId) -> Result<KillOutcome, PlatformError> {
        let Some(target) = signal_target(pid)? else {
            return Ok(KillOutcome::AlreadyExited);
        };
        signal_outcome(pid, kill_process(target, Signal::KILL))
    }

    fn kill_checked(&self, identity: ProcessIdentity) -> Result<KillOutcome, PlatformError> {
        let pid = identity.pid;
        if !identity.is_stable() {
            return Err(PlatformError::IdentityChanged { pid });
        }
        let Some(target) = signal_target(pid)? else {
            return Ok(KillOutcome::AlreadyExited);
        };
        kill_checked(target, identity)
    }
}

fn signal_target(pid: ProcessId) -> Result<Option<Pid>, PlatformError> {
    if pid.as_u32() <= 1 || pid.as_u32() == std::process::id() {
        return Err(PlatformError::AccessDenied { pid });
    }
    Ok(i32::try_from(pid.as_u32()).ok().and_then(Pid::from_raw))
}

fn signal_outcome(
    pid: ProcessId,
    result: rustix::io::Result<()>,
) -> Result<KillOutcome, PlatformError> {
    match result {
        Ok(()) => Ok(KillOutcome::Terminated),
        Err(rustix::io::Errno::SRCH) => Ok(KillOutcome::AlreadyExited),
        Err(rustix::io::Errno::PERM | rustix::io::Errno::ACCESS) => {
            Err(PlatformError::AccessDenied { pid })
        }
        Err(error) => Err(PlatformError::OperationFailed {
            operation: "kill",
            message: error.to_string(),
        }),
    }
}

#[cfg(target_os = "linux")]
fn process_start_time(pid: i32) -> Option<SystemTime> {
    let boot = procfs::boot_time_secs().ok()?;
    let rate = procfs::ticks_per_second();
    let stat = procfs::process::Process::new(pid).ok()?.stat().ok()?;
    let elapsed_secs = stat.starttime.checked_div(rate)?;
    let remaining_ticks = stat.starttime % rate;
    let nanos = remaining_ticks.checked_mul(1_000_000_000)? / rate;
    UNIX_EPOCH.checked_add(Duration::new(boot.checked_add(elapsed_secs)?, nanos as u32))
}

#[cfg(target_os = "macos")]
fn process_start_time(pid: i32) -> Option<SystemTime> {
    use libproc::bsd_info::BSDInfo;
    use libproc::proc_pid::pidinfo;

    let info = pidinfo::<BSDInfo>(pid, 0).ok()?;
    if info.pbi_start_tvsec == 0 {
        return None;
    }
    UNIX_EPOCH
        .checked_add(Duration::from_secs(info.pbi_start_tvsec))?
        .checked_add(Duration::from_micros(info.pbi_start_tvusec))
}

#[cfg(target_os = "linux")]
fn kill_checked(target: Pid, identity: ProcessIdentity) -> Result<KillOutcome, PlatformError> {
    use rustix::process::{PidfdFlags, pidfd_open, pidfd_send_signal};

    // Open the process descriptor first, so PID reuse after the identity
    // check cannot redirect the signal to another process.
    let handle = match pidfd_open(target, PidfdFlags::empty()) {
        Ok(handle) => handle,
        Err(error) => return signal_outcome(identity.pid, Err(error)),
    };
    if process_start_time(target.as_raw_pid()) != identity.started_at {
        return Err(PlatformError::IdentityChanged { pid: identity.pid });
    }
    signal_outcome(identity.pid, pidfd_send_signal(handle, Signal::KILL))
}

#[cfg(target_os = "macos")]
fn kill_checked(target: Pid, identity: ProcessIdentity) -> Result<KillOutcome, PlatformError> {
    if process_start_time(target.as_raw_pid()) != identity.started_at {
        if rustix::process::test_kill_process(target) == Err(rustix::io::Errno::SRCH) {
            return Ok(KillOutcome::AlreadyExited);
        }
        return Err(PlatformError::IdentityChanged { pid: identity.pid });
    }
    signal_outcome(identity.pid, kill_process(target, Signal::KILL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use portspy_core::domain::protocol::Protocol;
    use std::net::{TcpListener, UdpSocket};
    use std::process::Command;

    #[test]
    fn discovers_tcp_and_udp_for_both_address_families() -> std::io::Result<()> {
        let tcp4 = TcpListener::bind("127.0.0.1:0")?;
        let tcp6 = TcpListener::bind("[::]:0")?;
        let udp4 = UdpSocket::bind("127.0.0.1:0")?;
        let udp6 = UdpSocket::bind("[::]:0")?;

        let platform = crate::platform();
        let pid = ProcessId(std::process::id());
        let listeners = platform.listeners().map_err(std::io::Error::other)?;
        for (protocol, address) in [
            (Protocol::Tcp, tcp4.local_addr()?),
            (Protocol::Tcp, tcp6.local_addr()?),
            (Protocol::Udp, udp4.local_addr()?),
            (Protocol::Udp, udp6.local_addr()?),
        ] {
            assert!(listeners.contains(&RawListener {
                protocol,
                address: address.ip(),
                port: address.port(),
                pid,
            }));
        }
        let processes = platform.processes().map_err(std::io::Error::other)?;
        assert!(processes.iter().any(|process| process.pid == pid
            && process.started_at.is_some()
            && process.working_directory.is_some()
            && process.executable.is_some()));
        Ok(())
    }

    #[test]
    fn terminates_a_child_and_rejects_special_pid() -> std::io::Result<()> {
        let platform = crate::platform();
        let mut child = Command::new("/bin/sleep").arg("30").spawn()?;
        let pid = ProcessId(child.id());
        let outcome = platform.kill(pid);
        if !matches!(outcome, Ok(KillOutcome::Terminated)) {
            let _ = child.kill();
        }
        assert!(!child.wait()?.success());
        assert_eq!(outcome, Ok(KillOutcome::Terminated));
        assert_eq!(
            platform.kill(ProcessId(0)),
            Err(PlatformError::AccessDenied { pid: ProcessId(0) })
        );
        Ok(())
    }

    #[test]
    fn checked_stop_rejects_the_wrong_start_time() -> std::io::Result<()> {
        let platform = crate::platform();
        let mut child = Command::new("/bin/sleep").arg("30").spawn()?;
        let pid = ProcessId(child.id());
        let identity = ProcessIdentity::new(pid, Some(UNIX_EPOCH));
        let stale = platform.kill_checked(identity);
        let survived = child.try_wait();
        let valid = i32::try_from(pid.as_u32())
            .ok()
            .and_then(process_start_time);
        let checked = platform.kill_checked(ProcessIdentity::new(pid, valid));
        if !matches!(checked, Ok(KillOutcome::Terminated)) {
            let _ = child.kill();
        }
        assert!(!child.wait()?.success());
        assert_eq!(stale, Err(PlatformError::IdentityChanged { pid }));
        assert!(survived?.is_none());
        assert_eq!(checked, Ok(KillOutcome::Terminated));
        Ok(())
    }
}
