//! Process termination.

use std::time::SystemTime;

use portspy_core::domain::process::{ProcessId, ProcessIdentity};
use portspy_core::platform::{KillOutcome, PlatformError};
use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, STILL_ACTIVE};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, IsProcessCritical, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE, TerminateProcess,
};
use windows::core::BOOL;

/// Terminates one process, distinguishing "already exited" from real kills.
///
/// # Errors
///
/// Returns [`PlatformError::AccessDenied`] for protected/elevated processes
/// and [`PlatformError::ProcessGone`] when the PID disappeared.
pub fn kill(pid: ProcessId) -> Result<KillOutcome, PlatformError> {
    kill_pid(pid, None)
}

pub(super) fn kill_checked(identity: ProcessIdentity) -> Result<KillOutcome, PlatformError> {
    let pid = identity.pid;
    let started_at = identity
        .started_at
        .ok_or(PlatformError::IdentityChanged { pid })?;
    kill_pid(pid, Some(started_at))
}

fn kill_pid(pid: ProcessId, expected: Option<SystemTime>) -> Result<KillOutcome, PlatformError> {
    if expected.is_some() && (pid.as_u32() == 0 || pid.as_u32() == std::process::id()) {
        return Err(PlatformError::AccessDenied { pid });
    }
    // Safety: opening a handle for terminate+query; closed on every path.
    let handle = unsafe {
        OpenProcess(
            PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid.as_u32(),
        )
    };

    let handle = match handle {
        Ok(handle) => handle,
        Err(e) if super::is_win32_error(&e, ERROR_INVALID_PARAMETER) => {
            return Ok(KillOutcome::AlreadyExited);
        }
        Err(e) if super::is_win32_error(&e, ERROR_ACCESS_DENIED) => {
            return Err(PlatformError::AccessDenied { pid });
        }
        Err(e) => {
            return Err(PlatformError::CallFailed {
                api: "OpenProcess",
                code: super::win32_code(&e),
            });
        }
    };

    if let Some(started_at) = expected {
        if super::processes::creation_time(handle) != Some(started_at) {
            super::close(handle);
            return Err(PlatformError::IdentityChanged { pid });
        }
        let mut critical = BOOL::default();
        // Safety: the open handle supports querying and critical is a valid out parameter.
        if unsafe { IsProcessCritical(handle, &mut critical) }.is_err() || critical.as_bool() {
            super::close(handle);
            return Err(PlatformError::AccessDenied { pid });
        }
    }

    let mut exit_code = 0u32;
    // Safety: `exit_code` is a valid out-parameter.
    if unsafe { GetExitCodeProcess(handle, &mut exit_code) }.is_err() {
        // Safety: close the handle before returning.
        super::close(handle);
        return Ok(KillOutcome::AlreadyExited);
    }
    if exit_code as i32 != STILL_ACTIVE.0 {
        // Safety: close the handle before returning.
        super::close(handle);
        return Ok(KillOutcome::AlreadyExited);
    }

    // Safety: the handle was opened with PROCESS_TERMINATE; exit code 1 is
    // the conventional "killed" marker.
    let result = unsafe { TerminateProcess(handle, 1) };
    // Safety: close the handle on every path.
    super::close(handle);

    match result {
        Ok(()) => Ok(KillOutcome::Terminated),
        Err(e) if super::is_win32_error(&e, ERROR_ACCESS_DENIED) => {
            Err(PlatformError::AccessDenied { pid })
        }
        // If the process exited between the check and TerminateProcess,
        // TerminateProcess can fail with ERROR_INVALID_PARAMETER.
        Err(e) if super::is_win32_error(&e, ERROR_INVALID_PARAMETER) => {
            Ok(KillOutcome::AlreadyExited)
        }
        Err(e) => Err(PlatformError::CallFailed {
            api: "TerminateProcess",
            code: super::win32_code(&e),
        }),
    }
}
