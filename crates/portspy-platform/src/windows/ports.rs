//! Windows TCP listener discovery.
//!
//! Uses [`GetExtendedTcpTable`] with `TCP_TABLE_OWNER_PID_LISTENER` for both
//! address families. Two calls per family: the first probes the required
//! buffer size, the second fills it.
//!
//! Addresses and ports arrive in network byte order and are converted with
//! `Ipv4Addr::from(u32)` / `u16::from_be`, which interpret their input as
//! big-endian octets.

use portspy_core::domain::process::ProcessId;
use portspy_core::domain::protocol::Protocol;
use portspy_core::platform::{PlatformError, RawListener};
use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID,
    MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, MIB_TCP_STATE_LISTEN, TCP_TABLE_CLASS,
    TCP_TABLE_OWNER_PID_LISTENER,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

/// Collects every local TCP listener (IPv4 and IPv6) with its owning PID.
///
/// # Errors
///
/// Returns [`PlatformError::CallFailed`] when the OS refuses to provide the
/// table.
pub fn tcp_listeners() -> Result<Vec<RawListener>, PlatformError> {
    let mut listeners = collect(AF_INET.0 as u32, TCP_TABLE_OWNER_PID_LISTENER)?;
    listeners.extend(collect(AF_INET6.0 as u32, TCP_TABLE_OWNER_PID_LISTENER)?);
    Ok(listeners)
}

fn collect(family: u32, class: TCP_TABLE_CLASS) -> Result<Vec<RawListener>, PlatformError> {
    let mut size: u32 = 0;
    // Safety: the first call only probes `size`; no buffer is touched.
    let status = unsafe { GetExtendedTcpTable(None, &mut size, false, family, class, 0) };
    if status != 0 && status != ERROR_INSUFFICIENT_BUFFER.0 {
        return Err(PlatformError::CallFailed { api: "GetExtendedTcpTable(probe)", code: status });
    }

    let mut buffer = vec![0u8; size as usize];
    // Safety: `buffer` is owned, aligned for the table structs (u32) and
    // large enough for `size` bytes; the OS writes at most `size` bytes.
    let status = unsafe {
        GetExtendedTcpTable(
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
            false,
            family,
            class,
            0,
        )
    };
    if status != 0 {
        return Err(PlatformError::CallFailed { api: "GetExtendedTcpTable", code: status });
    }

    let mut listeners = Vec::new();
    // Safety: after a successful call `buffer` holds a valid table for
    // `family`; the flexible `table` array contains `dwNumEntries` rows.
    match family {
        _ if family == AF_INET.0 as u32 => {
            let table = unsafe { &*buffer.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>() };
            let rows = unsafe {
                std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
            };
            listeners.extend(
                rows.iter()
                    .filter(|row| row.dwState as i32 == MIB_TCP_STATE_LISTEN.0)
                    .filter_map(row_to_listener),
            );
        }
        _ if family == AF_INET6.0 as u32 => {
            let table = unsafe { &*buffer.as_ptr().cast::<MIB_TCP6TABLE_OWNER_PID>() };
            let rows = unsafe {
                std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
            };
            listeners.extend(
                rows.iter()
                    .filter(|row| row.dwState as i32 == MIB_TCP_STATE_LISTEN.0)
                    .filter_map(row6_to_listener),
            );
        }
        _ => {
            return Err(PlatformError::CallFailed {
                api: "GetExtendedTcpTable",
                code: family,
            });
        }
    }

    Ok(listeners)
}

/// Converts one IPv4 table row. Returns `None` for rows without an owner.
fn row_to_listener(row: &MIB_TCPROW_OWNER_PID) -> Option<RawListener> {
    let pid = row.dwOwningPid;
    if pid == 0 {
        return None;
    }
    Some(RawListener {
        protocol: Protocol::Tcp,
        // Safety-free conversion: dwLocalAddr is network byte order.
        address: std::net::Ipv4Addr::from(row.dwLocalAddr).into(),
        port: u16::from_be(row.dwLocalPort as u16),
        pid: ProcessId(pid),
    })
}

/// Converts one IPv6 table row. Returns `None` for rows without an owner.
fn row6_to_listener(row: &MIB_TCP6ROW_OWNER_PID) -> Option<RawListener> {
    let pid = row.dwOwningPid;
    if pid == 0 {
        return None;
    }
    Some(RawListener {
        protocol: Protocol::Tcp,
        // ucLocalAddr is plain network-order bytes.
        address: std::net::Ipv6Addr::from(row.ucLocalAddr).into(),
        port: u16::from_be(row.dwLocalPort as u16),
        pid: ProcessId(pid),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::{row_to_listener, row6_to_listener};
    use portspy_core::domain::process::ProcessId;
    use std::net::IpAddr;
    use windows::Win32::NetworkManagement::IpHelper::{
        MIB_TCP6ROW_OWNER_PID, MIB_TCP_STATE_LISTEN, MIB_TCPROW_OWNER_PID,
    };

    #[test]
    fn ipv4_row_converts_network_order() {
        // 127.0.0.1 = 0x0100007F in network byte order.
        let row = MIB_TCPROW_OWNER_PID {
            dwLocalAddr: 0x0100_007f,
            dwLocalPort: u32::from(u16::to_be(3000)),
            dwOwningPid: 18432,
            dwState: MIB_TCP_STATE_LISTEN.0 as u32,
            ..Default::default()
        };
        let listener = row_to_listener(&row).expect("row should convert");
        assert_eq!(listener.address, IpAddr::V4("127.0.0.1".parse().expect("valid")));
        assert_eq!(listener.port, 3000);
        assert_eq!(listener.pid, ProcessId(18432));
    }

    #[test]
    fn ipv4_row_without_owner_is_skipped() {
        let row = MIB_TCPROW_OWNER_PID {
            dwOwningPid: 0,
            ..Default::default()
        };
        assert!(row_to_listener(&row).is_none());
    }

    #[test]
    fn ipv6_row_converts_bytes() {
        // ::1
        let mut row = MIB_TCP6ROW_OWNER_PID {
            dwLocalPort: u32::from(u16::to_be(5173)),
            dwOwningPid: 42,
            dwState: MIB_TCP_STATE_LISTEN.0 as u32,
            ..Default::default()
        };
        row.ucLocalAddr[15] = 1;
        let listener = row6_to_listener(&row).expect("row should convert");
        assert_eq!(listener.address, IpAddr::V6("::1".parse().expect("valid")));
        assert_eq!(listener.port, 5173);
        assert_eq!(listener.pid, ProcessId(42));
    }

    #[test]
    fn port_conversion_handles_high_ports() {
        let row = MIB_TCPROW_OWNER_PID {
            dwLocalAddr: 0x0100_007f,
            dwLocalPort: u32::from(u16::to_be(65535)),
            dwOwningPid: 1,
            ..Default::default()
        };
        let listener = row_to_listener(&row).expect("row should convert");
        assert_eq!(listener.port, 65535);
    }
}
