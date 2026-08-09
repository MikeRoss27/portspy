//! Process termination.

use portspy_core::domain::process::ProcessId;
use portspy_core::platform::{KillOutcome, PlatformError};
use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, STILL_ACTIVE,
};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE,
};

/// Terminates one process, distinguishing "already exited" from real kills.
///
/// # Errors
///
/// Returns [`PlatformError::AccessDenied`] for protected/elevated processes
/// and [`PlatformError::ProcessGone`] when the PID disappeared.
pub fn kill(pid: ProcessId) -> Result<KillOutcome, PlatformError> {
    // Safety: opening a handle for terminate+query; closed on every path.
    let handle = unsafe {
        OpenProcess(PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION, false, pid.as_u32())
    };
    let handle = match handle {
        Ok(handle) => handle,
        Err(e) if super::is_win32_error(&e, ERROR_INVALID_PARAMETER) => {
            return Ok(KillOutcome::AlreadyExited);
        }
        Err(e) if super::is_win32_error(&e, ERROR_ACCESS_DENIED) => {
            return Err(PlatformError::AccessDenied { pid });
        }
        Err(e) => return Err(PlatformError::CallFailed { api: "OpenProcess", code: super::win32_code(&e) }),
    };

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
        Err(e) => Err(PlatformError::CallFailed { api: "TerminateProcess", code: super::win32_code(&e) }),
    }
}
