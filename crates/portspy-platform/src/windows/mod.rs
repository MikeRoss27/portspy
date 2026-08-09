//! Windows platform implementation.
//!
//! All native API usage in the project lives under this module. Every
//! `unsafe` block carries a `// Safety:` comment explaining its invariants.

pub mod kill;
pub mod ports;
pub mod processes;

use portspy_core::domain::process::ProcessId;
use portspy_core::platform::{KillOutcome, Platform, PlatformError, RawListener, RawProcess};
use windows::core::Error as WindowsError;
use windows::Win32::Foundation::WIN32_ERROR;

/// Windows implementation of the platform contract.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsPlatform;

impl Platform for WindowsPlatform {
    fn listeners(&self) -> Result<Vec<RawListener>, PlatformError> {
        ports::tcp_listeners()
    }

    fn processes(&self) -> Result<Vec<RawProcess>, PlatformError> {
        processes::processes()
    }

    fn kill(&self, pid: ProcessId) -> Result<KillOutcome, PlatformError> {
        kill::kill(pid)
    }
}

/// Extracts the raw Win32 error code from a windows-crate error.
///
/// The crate surfaces Win32 failures as `HRESULT`s of the form
/// `0x8007xxxx`; the low 16 bits carry the original `WIN32_ERROR`.
pub(crate) fn win32_code(error: &WindowsError) -> u32 {
    error.code().0 as u32 & 0x0000_ffff
}

/// Whether a windows-crate error corresponds to the given Win32 error.
pub(crate) fn is_win32_error(error: &WindowsError, code: WIN32_ERROR) -> bool {
    win32_code(error) == code.0
}

/// Closes a native handle, discarding the result.
///
/// A failing `CloseHandle` leaks the handle but is never a memory-safety
/// issue; the caller cannot do anything useful with the failure on the hot
/// snapshot path.
pub(crate) fn close(handle: windows::Win32::Foundation::HANDLE) {
    // Safety: the caller guarantees `handle` is a valid open handle (or
    // null); closing is allowed to fail silently.
    let _ = unsafe { windows::Win32::Foundation::CloseHandle(handle) };
}
