//! Windows process enumeration and metadata collection.
//!
//! Strategy:
//!
//! 1. One [`CreateToolhelp32Snapshot`] pass gives PID, parent PID and image
//!    name for every process — the backbone for ancestry reconstruction.
//! 2. Per-process metadata (path, start time, memory) is read through a
//!    handle opened with `PROCESS_QUERY_LIMITED_INFORMATION`.
//! 3. Command lines are read with `NtQueryInformationProcess` +
//!    `ReadProcessMemory` (requires `PROCESS_QUERY_INFORMATION`); when the
//!    extra access right is denied the command line stays `None` — that is
//!    valid partial information, never a failure.
//!
//! The whole collection is O(processes) with a small constant, well within
//! the v0.1 refresh budget; it never goes through WMI.

use portspy_core::domain::process::ProcessId;
use portspy_core::platform::{PlatformError, RawProcess};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use windows::core::Error as WindowsError;
use windows::Win32::Foundation::{
    ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_FILES, FILETIME, HANDLE,
    STILL_ACTIVE, UNICODE_STRING,
};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    QueryFullProcessImageNameW,
};
use windows::Wdk::System::Threading::{
    NtQueryInformationProcess, ProcessCommandLineInformation,
};

/// Snapshot of every process: names and parents from the toolhelp pass,
/// then metadata for each PID.
///
/// # Errors
///
/// Fails only when the toolhelp snapshot itself cannot be taken. Missing
/// per-process metadata never fails the whole collection.
pub fn processes() -> Result<Vec<RawProcess>, PlatformError> {
    let entries = process_entries()?;

    let mut processes = Vec::with_capacity(entries.len());
    for (pid, parent_pid, name) in entries {
        let mut raw = RawProcess {
            pid,
            parent_pid,
            name,
            executable: None,
            command_line: None,
            working_directory: None,
            started_at: None,
            memory_bytes: None,
            cpu_percent: None,
        };
        inspect(pid, &mut raw);
        processes.push(raw);
    }
    Ok(processes)
}

/// Enumerates every process: PID, parent PID and image name.
///
/// # Errors
///
/// Returns [`PlatformError::CallFailed`] when the snapshot cannot be taken.
fn process_entries() -> Result<Vec<(ProcessId, Option<ProcessId>, String)>, PlatformError> {
    // Safety: TH32CS_SNAPPROCESS takes a plain kernel snapshot; the handle
    // returned must be closed by the caller.
    let snapshot = unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| windows_call("CreateToolhelp32Snapshot", &e))?
    };

    let mut entries = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    // Safety: `entry` is sized and initialized; the handle is valid until
    // closed below. Access is guarded by the `Ok/Err` results.
    let first = unsafe { Process32FirstW(snapshot, &mut entry) };
    if first.is_ok() {
        loop {
            entries.push((
                ProcessId(entry.th32ProcessID),
                (entry.th32ParentProcessID != 0).then_some(ProcessId(entry.th32ParentProcessID)),
                wide_to_string(&entry.szExeFile),
            ));
            // Safety: same contract as Process32FirstW; the terminal error
            // is ERROR_NO_MORE_FILES, which is expected.
            match unsafe { Process32NextW(snapshot, &mut entry) } {
                Ok(()) => continue,
                Err(e) if super::is_win32_error(&e, ERROR_NO_MORE_FILES) => break,
                Err(_) => break,
            }
        }
    }
    // Safety: closes the snapshot handle created above.
    super::close(snapshot);
    Ok(entries)
}

/// Best-effort metadata collection for one process. Never fails.
fn inspect(pid: ProcessId, raw: &mut RawProcess) {
    // Safety: limited-information handles work for most processes and give
    // path, times and memory; opening a second handle with query rights is
    // only needed for the command line.
    let handle = match unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
            false,
            pid.as_u32(),
        )
    } {
        Ok(handle) => handle,
        Err(e) => {
            // Access denied or exited: keep the toolhelp facts, drop the rest.
            let _ = e;
            return;
        }
    };

    if !is_alive(handle) {
        // Safety: close the handle before returning.
        super::close(handle);
        return;
    }

    raw.executable = executable_path(handle);
    raw.started_at = creation_time(handle);
    raw.memory_bytes = memory_bytes(handle);

    // Safety: close the limited handle; the command-line pass opens its own.
    super::close(handle);

    raw.command_line = command_line(pid);
}

/// `true` when the process is still running (`STILL_ACTIVE`).
fn is_alive(handle: HANDLE) -> bool {
    let mut exit_code = 0u32;
    // Safety: `exit_code` is a valid out-parameter of a live handle.
    match unsafe { GetExitCodeProcess(handle, &mut exit_code) } {
        Ok(()) => exit_code as i32 == STILL_ACTIVE.0,
        Err(_) => true,
    }
}

fn executable_path(handle: HANDLE) -> Option<PathBuf> {
    let mut buffer = vec![0u16; 4096];
    loop {
        let mut size = buffer.len() as u32;
        // Safety: `buffer` is writable for `size` words; the API writes at
        // most that many characters plus a null terminator.
        let result = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buffer.as_mut_ptr()),
                &mut size,
            )
        };
        match result {
            Ok(()) => {
                buffer.truncate(size as usize);
                return Some(PathBuf::from(String::from_utf16_lossy(&buffer)));
            }
            Err(e) if super::is_win32_error(&e, ERROR_INSUFFICIENT_BUFFER) => {
                buffer.resize(buffer.len() * 2, 0);
            }
            Err(_) => return None,
        }
    }
}

fn creation_time(handle: HANDLE) -> Option<SystemTime> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // Safety: all four FILETIMEs are valid out-parameters.
    let ok = unsafe {
        GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user)
    };
    ok.ok().map(|_| filetime_to_system_time(creation))
}

fn memory_bytes(handle: HANDLE) -> Option<u64> {
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    // Safety: `counters` is sized and `cb` is set, as required by the API.
    unsafe { GetProcessMemoryInfo(handle, &mut counters, counters.cb) }
        .ok()
        .map(|_| counters.WorkingSetSize as u64)
}

/// Reads the command line through `ProcessCommandLineInformation`.
///
/// Requires `PROCESS_QUERY_INFORMATION`, which elevated or protected
/// processes may refuse; `None` is then reported.
/// `[u8; N]` whose start is aligned for `UNICODE_STRING` (8 on x64).
#[repr(align(8))]
struct AlignedBytes<const N: usize>([u8; N]);

fn command_line(pid: ProcessId) -> Option<String> {
    // Safety: query-information handles are opened and closed here.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, false, pid.as_u32()).ok()? };

    // `NtQueryInformationProcess` writes a UNICODE_STRING at the start of
    // the buffer; a plain `[u8; N]` is only 1-aligned, so keep the buffer
    // 8-aligned to make that dereference valid.
    let mut buffer = AlignedBytes([0u8; 65_536]);
    let mut return_length: u32 = 0;
    // Safety: `buffer` is owned and sized; the kernel writes at most its
    // size and returns the structure at its start.
    let status = unsafe {
        NtQueryInformationProcess(
            handle,
            ProcessCommandLineInformation,
            buffer.0.as_mut_ptr().cast(),
            buffer.0.len() as u32,
            &mut return_length as *mut u32,
        )
    };
    if status.0 < 0 {
        // Safety: close the query handle before returning.
        super::close(handle);
        return None;
    }

    // Safety: after a successful call the buffer holds a UNICODE_STRING
    // whose `Buffer` points into the *remote* process memory.
    let unicode = unsafe { &*buffer.0.as_ptr().cast::<UNICODE_STRING>() };
    let byte_len = usize::from(unicode.Length);
    if byte_len == 0 || unicode.Buffer.is_null() {
        // Safety: close the query handle before returning.
        super::close(handle);
        return None;
    }

    let mut words = vec![0u16; byte_len / 2 + 1];
    let mut read = 0usize;
    // Safety: `words` is writable for `byte_len` bytes; the source is the
    // remote pointer returned by the kernel for `handle`.
    let ok = unsafe {
        ReadProcessMemory(
            handle,
            unicode.Buffer.as_ptr().cast(),
            words.as_mut_ptr().cast(),
            byte_len,
            Some(&mut read),
        )
    };
    // Safety: close the query handle.
    super::close(handle);

    ok.ok()?;
    let count = read / 2;
    Some(String::from_utf16_lossy(&words[..count.min(words.len())]))
}

/// FILETIME (100 ns ticks since 1601-01-01) to `SystemTime`.
fn filetime_to_system_time(filetime: FILETIME) -> SystemTime {
    const TICKS_BETWEEN_1601_AND_1970: i64 = 116_444_736_000_000_000;
    let ticks = (i64::from(filetime.dwHighDateTime) << 32) | i64::from(filetime.dwLowDateTime);
    let unix_ticks = ticks - TICKS_BETWEEN_1601_AND_1970;
    let seconds = unix_ticks / 10_000_000;
    let nanos = (unix_ticks % 10_000_000) * 100;
    SystemTime::UNIX_EPOCH + Duration::new(seconds.max(0) as u64, nanos.max(0) as u32)
}

/// Converts a fixed-size `[u16; N]` wide-character array into a `String`,
/// stripping the null terminator.
fn wide_to_string<const N: usize>(buffer: &[u16; N]) -> String {
    let end = buffer.iter().position(|c| *c == 0).unwrap_or(N);
    String::from_utf16_lossy(&buffer[..end])
}

fn windows_call(api: &'static str, error: &WindowsError) -> PlatformError {
    PlatformError::CallFailed { api, code: super::win32_code(error) }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::{filetime_to_system_time, wide_to_string};
    use windows::Win32::Foundation::FILETIME;
    use std::time::SystemTime;

    #[test]
    fn wide_string_strips_terminator() {
        let buffer = [b'n' as u16, b'o' as u16, b'd' as u16, b'e' as u16, 0, 0];
        assert_eq!(wide_to_string(&buffer), "node");
    }

    #[test]
    fn filetime_converts_unix_epoch() {
        // 1970-01-01T00:00:00Z in FILETIME ticks.
        let filetime = FILETIME {
            dwLowDateTime: 0xd53e_8000,
            dwHighDateTime: 0x019d_b1de,
        };
        assert_eq!(
            filetime_to_system_time(filetime),
            SystemTime::UNIX_EPOCH
        );
    }
}
