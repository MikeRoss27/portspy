//! Windows process enumeration and metadata collection.
//!
//! Strategy:
//!
//! 1. One [`CreateToolhelp32Snapshot`] pass gives PID, parent PID and image
//!    name for every process — the backbone for ancestry reconstruction.
//! 2. Per-process metadata (path, start time, memory) is read through a
//!    handle opened with `PROCESS_QUERY_LIMITED_INFORMATION`.
//! 3. Command lines are read with `NtQueryInformationProcess`; when query
//!    access is denied the command line stays `None` — that is valid partial
//!    information, never a failure.
//! 4. Working directories are read through the PEB
//!    (`ProcessBasicInformation` → `ProcessParameters.CurrentDirectory`),
//!    again `None` when access is denied.
//!
//! The whole collection is O(processes) with a small constant, well within
//! the v0.1 refresh budget; it never goes through WMI.

use portspy_core::domain::process::ProcessId;
use portspy_core::platform::{PlatformError, RawProcess};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use windows::Wdk::System::Threading::{
    NtQueryInformationProcess, ProcessBasicInformation, ProcessCommandLineInformation,
};
use windows::Win32::Foundation::{
    ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_FILES, FILETIME, HANDLE, STILL_ACTIVE, UNICODE_STRING,
};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    QueryFullProcessImageNameW,
};
use windows::core::Error as WindowsError;

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

struct AutoHandle(HANDLE);

impl Drop for AutoHandle {
    fn drop(&mut self) {
        super::close(self.0);
    }
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
    let _auto_snapshot = AutoHandle(snapshot);

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
    Ok(entries)
}

/// Best-effort metadata collection for one process. Never fails.
fn inspect(pid: ProcessId, raw: &mut RawProcess) {
    // Safety: this read-only handle asks only for the minimum metadata access.
    // Requiring termination rights here would make ordinary inspection fail
    // for processes that Windows allows us to query but not terminate.
    let handle =
        match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid.as_u32()) } {
            Ok(handle) => AutoHandle(handle),
            Err(e) => {
                // Access denied or exited: keep the toolhelp facts, drop the rest.
                let _ = e;
                return;
            }
        };

    if !is_alive(handle.0) {
        return;
    }

    raw.executable = executable_path(handle.0);
    raw.started_at = creation_time(handle.0);
    raw.memory_bytes = memory_bytes(handle.0);

    drop(handle);

    raw.command_line = command_line(pid);
    raw.working_directory = working_directory(pid);
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
    const MAX_BUFFER_SIZE: usize = 1_048_576; // 1MB cap
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
                let new_len = (buffer.len() * 2).min(MAX_BUFFER_SIZE);
                if new_len == buffer.len() {
                    return None;
                }
                buffer.resize(new_len, 0);
            }
            Err(_) => return None,
        }
    }
}

pub(super) fn creation_time(handle: HANDLE) -> Option<SystemTime> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // Safety: all four FILETIMEs are valid out-parameters.
    let ok = unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) };
    ok.ok().and_then(|_| {
        // Zero FILETIME means the time is not available.
        if creation.dwLowDateTime == 0 && creation.dwHighDateTime == 0 {
            None
        } else {
            Some(filetime_to_system_time(creation))
        }
    })
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

/// Reads the working directory through the PEB.
///
/// The path goes `ProcessBasicInformation.PebBaseAddress` →
/// `PEB.ProcessParameters` (offset `0x20` on x64) →
/// `RTL_USER_PROCESS_PARAMETERS.CurrentDirectory` (offset `0x38` on x64).
/// Reading the 64-bit PEB works for WOW64 processes too, because every
/// process on a 64-bit system carries a full 64-bit PEB.
///
/// Requires `PROCESS_QUERY_INFORMATION`; returns `None` when access is
/// denied or the layout does not match the expectations above.
fn working_directory(pid: ProcessId) -> Option<PathBuf> {
    // Safety: query-information handles are opened and closed here.
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            pid.as_u32(),
        )
        .ok()
        .map(AutoHandle)?
    };

    // PROCESS_BASIC_INFORMATION on x64: ExitStatus(i32) + pad + PebBaseAddress
    // (ptr, offset 0x08) + AffinityMask + BasePriority + two PIDs = 48 bytes.
    let mut basic = [0u8; 48];
    let mut return_length = 0u32;
    // Safety: `basic` is owned and sized; the kernel writes at most its size
    // and returns the structure at its start.
    let status = unsafe {
        NtQueryInformationProcess(
            handle.0,
            ProcessBasicInformation,
            basic.as_mut_ptr().cast(),
            basic.len() as u32,
            &mut return_length,
        )
    };
    if status.0 < 0 {
        return None;
    }
    let peb = usize::from_le_bytes(basic[8..16].try_into().ok()?);

    read_peb_directory(handle.0, peb)
}

/// Follows the PEB pointers and reads `CurrentDirectory.DosPath`.
///
/// `peb == 0` (protected processes) or any failed read yields `None`; that is
/// partial information, not a failure.
fn read_peb_directory(handle: HANDLE, peb: usize) -> Option<PathBuf> {
    if peb == 0 {
        return None;
    }

    // PEB.ProcessParameters lives at offset 0x20 on x64.
    let mut parameters_ptr = [0u8; 8];
    if !read_remote(handle, peb + 0x20, &mut parameters_ptr) {
        return None;
    }
    let parameters = usize::from_le_bytes(parameters_ptr);
    if parameters == 0 {
        return None;
    }

    // RTL_USER_PROCESS_PARAMETERS.CurrentDirectory.DosPath is a
    // UNICODE_STRING at offset 0x38 on x64: Length(u16) + MaximumLength(u16)
    // + pad + Buffer(ptr).
    let mut current_directory = [0u8; 16];
    if !read_remote(handle, parameters + 0x38, &mut current_directory) {
        return None;
    }
    let length = u16::from_le_bytes([current_directory[0], current_directory[1]]) as usize;
    let buffer = usize::from_le_bytes(current_directory[8..16].try_into().ok()?);
    if length == 0 || buffer == 0 {
        return None;
    }

    let mut bytes = vec![0u8; length];
    if !read_remote(handle, buffer, &mut bytes) {
        return None;
    }
    let words: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(u16::from_le_bytes)
        .collect();
    let end = words.iter().position(|c| *c == 0).unwrap_or(words.len());
    Some(PathBuf::from(String::from_utf16_lossy(&words[..end])))
}

/// Reads `size` bytes from the remote process at `address`.
fn read_remote(handle: HANDLE, address: usize, destination: &mut [u8]) -> bool {
    let mut read = 0usize;
    // Safety: `destination` is writable for its length; the source address
    // comes from the PEB of the process identified by `handle`.
    unsafe {
        ReadProcessMemory(
            handle,
            address as *const core::ffi::c_void,
            destination.as_mut_ptr().cast(),
            destination.len(),
            Some(&mut read),
        )
    }
    .is_ok()
        && read == destination.len()
}

/// Reads the command line through `ProcessCommandLineInformation`.
///
/// Requires query access, which elevated or protected processes may refuse;
/// `None` is then reported.
/// `[u8; N]` whose start is aligned for `UNICODE_STRING` (8 on x64).
#[repr(align(8))]
struct AlignedBytes<const N: usize>([u8; N]);

fn command_line(pid: ProcessId) -> Option<String> {
    // Safety: query-information handles are opened and closed here.
    let handle = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid.as_u32())
            .ok()
            .map(AutoHandle)?
    };

    // `NtQueryInformationProcess` writes a UNICODE_STRING at the start of
    // the buffer; a plain `[u8; N]` is only 1-aligned, so keep the buffer
    // 8-aligned to make that dereference valid.
    let mut buffer = AlignedBytes([0u8; 65_536]);
    let mut return_length: u32 = 0;
    // Safety: `buffer` is owned and sized; the kernel writes at most its
    // size and returns the structure at its start.
    let status = unsafe {
        NtQueryInformationProcess(
            handle.0,
            ProcessCommandLineInformation,
            buffer.0.as_mut_ptr().cast(),
            buffer.0.len() as u32,
            &mut return_length as *mut u32,
        )
    };
    if status.0 < 0 {
        return None;
    }

    // Safety: after a successful call the buffer starts with a properly
    // aligned UNICODE_STRING. For ProcessCommandLineInformation its Buffer
    // points at the copied string inside our caller-owned result buffer.
    let unicode = unsafe { &*buffer.0.as_ptr().cast::<UNICODE_STRING>() };
    let byte_len = usize::from(unicode.Length);
    if byte_len == 0 || unicode.Buffer.is_null() {
        return None;
    }

    (|| {
        let start = buffer.0.as_ptr() as usize;
        let string_start = unicode.Buffer.as_ptr() as usize;
        let offset = string_start.checked_sub(start)?;
        let end = offset.checked_add(byte_len)?;
        let valid_len = usize::try_from(return_length)
            .ok()
            .map_or(buffer.0.len(), |length| length.min(buffer.0.len()));
        if end > valid_len {
            return None;
        }
        decode_utf16_bytes(buffer.0.get(offset..end)?)
    })()
}

fn decode_utf16_bytes(bytes: &[u8]) -> Option<String> {
    let (pairs, remainder) = bytes.as_chunks::<2>();
    if pairs.is_empty() || !remainder.is_empty() {
        return None;
    }
    let words: Vec<u16> = pairs.iter().copied().map(u16::from_ne_bytes).collect();
    Some(String::from_utf16_lossy(&words))
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
    PlatformError::CallFailed {
        api,
        code: super::win32_code(error),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::{decode_utf16_bytes, filetime_to_system_time, wide_to_string};
    use std::time::SystemTime;
    use windows::Win32::Foundation::FILETIME;

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
        assert_eq!(filetime_to_system_time(filetime), SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn command_line_buffer_decodes_utf16() {
        let bytes: Vec<u8> = "node next dev"
            .encode_utf16()
            .flat_map(u16::to_ne_bytes)
            .collect();
        assert_eq!(decode_utf16_bytes(&bytes).as_deref(), Some("node next dev"));
    }

    #[test]
    fn command_line_buffer_rejects_partial_code_unit() {
        assert!(decode_utf16_bytes(b"n").is_none());
    }
}
