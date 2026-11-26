//! Native socket and process discovery shared by Linux and macOS.

use portspy_core::domain::process::{ProcessId, ProcessIdentity};
use portspy_core::platform::{KillOutcome, Platform, PlatformError, RawListener, RawProcess};
use rustix::process::{Pid, Signal, kill_process};

#[cfg_attr(target_os = "linux", path = "unix/linux.rs")]
#[cfg_attr(target_os = "macos", path = "unix/macos.rs")]
mod native;

pub(crate) struct UnixPlatform;

impl Platform for UnixPlatform {
    fn listeners(&self) -> Result<Vec<RawListener>, PlatformError> {
        native::listeners()
    }

    fn processes(&self) -> Result<Vec<RawProcess>, PlatformError> {
        native::processes()
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
        native::kill_checked(target, identity)
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

    #[cfg(target_os = "linux")]
    #[test]
    fn checked_stop_rejects_the_wrong_start_time() -> std::io::Result<()> {
        use std::time::UNIX_EPOCH;

        let platform = crate::platform();
        let mut child = Command::new("/bin/sleep").arg("30").spawn()?;
        let pid = ProcessId(child.id());
        let identity = ProcessIdentity::new(pid, Some(UNIX_EPOCH));
        let stale = platform.kill_checked(identity);
        let survived = child.try_wait();
        let checked = match std::thread::scope(|scope| scope.spawn(|| platform.process(pid)).join())
        {
            Ok(Ok(Some(process))) => platform.kill_checked(process.identity()),
            _ => Err(PlatformError::IdentityChanged { pid }),
        };
        if !matches!(checked, Ok(KillOutcome::Terminated)) {
            let _ = child.kill();
        }
        assert!(!child.wait()?.success());
        assert_eq!(stale, Err(PlatformError::IdentityChanged { pid }));
        assert!(survived?.is_none());
        assert_eq!(checked, Ok(KillOutcome::Terminated));
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn checked_stop_reports_unsupported_without_signaling_the_process() -> std::io::Result<()> {
        let platform = crate::platform();
        let mut child = Command::new("/bin/sleep").arg("30").spawn()?;
        let pid = ProcessId(child.id());
        let result = platform
            .process(pid)
            .and_then(|process| process.ok_or(PlatformError::IdentityChanged { pid }))
            .and_then(|process| platform.kill_checked(process.identity()));
        let survived = child.try_wait();
        let _ = child.kill();
        child.wait()?;

        assert!(matches!(result, Err(PlatformError::Unsupported(_))));
        assert!(survived?.is_none());
        Ok(())
    }
}
