//! Windows TCP and UDP listener discovery.
//!
//! Uses [`GetExtendedTcpTable`] with `TCP_TABLE_OWNER_PID_LISTENER` and
//! [`GetExtendedUdpTable`] with `UDP_TABLE_OWNER_PID` for both address
//! families. Two calls per family: the first probes the required buffer
//! size, the second fills it.
//!
//! Addresses and ports arrive in network byte order. The Win32 IPv4 address
//! fields are integers in host memory, so they must be converted from
//! big-endian before being passed to `Ipv4Addr`; ports use `u16::from_be`.

use portspy_core::domain::process::ProcessId;
use portspy_core::domain::protocol::Protocol;
use portspy_core::platform::{PlatformError, RawListener};
use std::mem::{offset_of, size_of, size_of_val};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_INVALID_DATA};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP_STATE_LISTEN, MIB_TCP6ROW_OWNER_PID,
    MIB_TCP6TABLE_OWNER_PID, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, MIB_UDP6ROW_OWNER_PID,
    MIB_UDP6TABLE_OWNER_PID, MIB_UDPROW_OWNER_PID, MIB_UDPTABLE_OWNER_PID, TCP_TABLE_CLASS,
    TCP_TABLE_OWNER_PID_LISTENER, UDP_TABLE_CLASS, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

/// Collects every local TCP and UDP listener (IPv4 and IPv6) with its owning
/// PID. A listener process can own both; no de-duplication is attempted here.
///
/// # Errors
///
/// Returns [`PlatformError::CallFailed`] if any table query fails or returns
/// an invalid table.
pub fn listeners() -> Result<Vec<RawListener>, PlatformError> {
    let mut listeners = tcp_listeners()?;
    listeners.extend(udp_listeners()?);
    Ok(listeners)
}

/// TCP discovery, kept for targeted use and tests.
///
/// # Errors
///
/// Returns [`PlatformError::CallFailed`] if either TCP table query fails or
/// returns an invalid table.
pub fn tcp_listeners() -> Result<Vec<RawListener>, PlatformError> {
    collect_families(|family| collect_tcp(family, TCP_TABLE_OWNER_PID_LISTENER))
}

/// UDP discovery, kept for targeted use and tests.
///
/// # Errors
///
/// Returns [`PlatformError::CallFailed`] if either UDP table query fails or
/// returns an invalid table.
pub fn udp_listeners() -> Result<Vec<RawListener>, PlatformError> {
    collect_families(|family| collect_udp(family, UDP_TABLE_OWNER_PID))
}

fn collect_families(
    collect: impl Fn(u32) -> Result<Vec<RawListener>, PlatformError>,
) -> Result<Vec<RawListener>, PlatformError> {
    let mut listeners = collect(AF_INET.0 as u32)?;
    listeners.extend(collect(AF_INET6.0 as u32)?);
    Ok(listeners)
}

fn checked_row_count(
    buffer: &[u32],
    rows_offset: usize,
    row_size: usize,
    api: &'static str,
) -> Result<usize, PlatformError> {
    let num_entries = buffer.first().copied().unwrap_or_default() as usize;
    let capacity = size_of_val(buffer).saturating_sub(rows_offset) / row_size;
    if num_entries > capacity {
        return Err(PlatformError::CallFailed {
            api,
            code: ERROR_INVALID_DATA.0,
        });
    }
    Ok(num_entries)
}

fn collect_tcp(family: u32, class: TCP_TABLE_CLASS) -> Result<Vec<RawListener>, PlatformError> {
    let mut size: u32 = 0;
    let mut buffer: Vec<u32> = Vec::new();
    let mut retries = 0;

    loop {
        let status = unsafe {
            GetExtendedTcpTable(
                if buffer.is_empty() {
                    None
                } else {
                    Some(buffer.as_mut_ptr().cast())
                },
                &mut size,
                false,
                family,
                class,
                0,
            )
        };

        if status == 0 {
            break;
        } else if status == ERROR_INSUFFICIENT_BUFFER.0 {
            if retries >= 5 {
                return Err(PlatformError::CallFailed {
                    api: "GetExtendedTcpTable",
                    code: status,
                });
            }
            retries += 1;
            if size == 0 {
                return Err(PlatformError::CallFailed {
                    api: "GetExtendedTcpTable",
                    code: status,
                });
            }
            buffer = vec![0u32; (size as usize).div_ceil(size_of::<u32>())];
        } else {
            return Err(PlatformError::CallFailed {
                api: "GetExtendedTcpTable",
                code: status,
            });
        }
    }

    if buffer.is_empty() || buffer[0] == 0 {
        return Ok(Vec::new());
    }

    let mut listeners = Vec::new();
    match family {
        _ if family == AF_INET.0 as u32 => {
            let num_entries = checked_row_count(
                &buffer,
                offset_of!(MIB_TCPTABLE_OWNER_PID, table),
                size_of::<MIB_TCPROW_OWNER_PID>(),
                "GetExtendedTcpTable",
            )?;
            let table_ptr = buffer.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
            let rows = unsafe {
                std::slice::from_raw_parts(
                    std::ptr::addr_of!((*table_ptr).table).cast::<MIB_TCPROW_OWNER_PID>(),
                    num_entries,
                )
            };
            listeners.extend(
                rows.iter()
                    .filter(|row| row.dwState as i32 == MIB_TCP_STATE_LISTEN.0)
                    .filter_map(row_to_listener),
            );
        }
        _ if family == AF_INET6.0 as u32 => {
            let num_entries = checked_row_count(
                &buffer,
                offset_of!(MIB_TCP6TABLE_OWNER_PID, table),
                size_of::<MIB_TCP6ROW_OWNER_PID>(),
                "GetExtendedTcpTable",
            )?;
            let table_ptr = buffer.as_ptr().cast::<MIB_TCP6TABLE_OWNER_PID>();
            let rows = unsafe {
                std::slice::from_raw_parts(
                    std::ptr::addr_of!((*table_ptr).table).cast::<MIB_TCP6ROW_OWNER_PID>(),
                    num_entries,
                )
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

fn collect_udp(family: u32, class: UDP_TABLE_CLASS) -> Result<Vec<RawListener>, PlatformError> {
    let mut size: u32 = 0;
    let mut buffer: Vec<u32> = Vec::new();
    let mut retries = 0;

    loop {
        let status = unsafe {
            GetExtendedUdpTable(
                if buffer.is_empty() {
                    None
                } else {
                    Some(buffer.as_mut_ptr().cast())
                },
                &mut size,
                false,
                family,
                class,
                0,
            )
        };

        if status == 0 {
            break;
        } else if status == ERROR_INSUFFICIENT_BUFFER.0 {
            if retries >= 5 {
                return Err(PlatformError::CallFailed {
                    api: "GetExtendedUdpTable",
                    code: status,
                });
            }
            retries += 1;
            if size == 0 {
                return Err(PlatformError::CallFailed {
                    api: "GetExtendedUdpTable",
                    code: status,
                });
            }
            buffer = vec![0u32; (size as usize).div_ceil(size_of::<u32>())];
        } else {
            return Err(PlatformError::CallFailed {
                api: "GetExtendedUdpTable",
                code: status,
            });
        }
    }

    if buffer.is_empty() || buffer[0] == 0 {
        return Ok(Vec::new());
    }

    let mut listeners = Vec::new();
    match family {
        _ if family == AF_INET.0 as u32 => {
            let num_entries = checked_row_count(
                &buffer,
                offset_of!(MIB_UDPTABLE_OWNER_PID, table),
                size_of::<MIB_UDPROW_OWNER_PID>(),
                "GetExtendedUdpTable",
            )?;
            let table_ptr = buffer.as_ptr().cast::<MIB_UDPTABLE_OWNER_PID>();
            let rows = unsafe {
                std::slice::from_raw_parts(
                    std::ptr::addr_of!((*table_ptr).table).cast::<MIB_UDPROW_OWNER_PID>(),
                    num_entries,
                )
            };
            listeners.extend(rows.iter().filter_map(udp_row_to_listener));
        }
        _ if family == AF_INET6.0 as u32 => {
            let num_entries = checked_row_count(
                &buffer,
                offset_of!(MIB_UDP6TABLE_OWNER_PID, table),
                size_of::<MIB_UDP6ROW_OWNER_PID>(),
                "GetExtendedUdpTable",
            )?;
            let table_ptr = buffer.as_ptr().cast::<MIB_UDP6TABLE_OWNER_PID>();
            let rows = unsafe {
                std::slice::from_raw_parts(
                    std::ptr::addr_of!((*table_ptr).table).cast::<MIB_UDP6ROW_OWNER_PID>(),
                    num_entries,
                )
            };
            listeners.extend(rows.iter().filter_map(udp_row6_to_listener));
        }
        _ => {
            return Err(PlatformError::CallFailed {
                api: "GetExtendedUdpTable",
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
        // Win32 stores the address bytes in network order inside a DWORD.
        address: std::net::Ipv4Addr::from(u32::from_be(row.dwLocalAddr)).into(),
        port: u16::from_be(row.dwLocalPort as u16),
        pid: ProcessId(pid),
        socket_attribution: None,
    })
}

/// Converts one IPv6 TCP table row. Returns `None` for rows without an owner.
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
        socket_attribution: None,
    })
}

/// Converts one IPv4 UDP table row. Returns `None` for rows without an owner.
fn udp_row_to_listener(row: &MIB_UDPROW_OWNER_PID) -> Option<RawListener> {
    let pid = row.dwOwningPid;
    if pid == 0 {
        return None;
    }
    Some(RawListener {
        protocol: Protocol::Udp,
        // Win32 stores the address bytes in network order inside a DWORD.
        address: std::net::Ipv4Addr::from(u32::from_be(row.dwLocalAddr)).into(),
        port: u16::from_be(row.dwLocalPort as u16),
        pid: ProcessId(pid),
        socket_attribution: None,
    })
}

/// Converts one IPv6 UDP table row. Returns `None` for rows without an owner.
fn udp_row6_to_listener(row: &MIB_UDP6ROW_OWNER_PID) -> Option<RawListener> {
    let pid = row.dwOwningPid;
    if pid == 0 {
        return None;
    }
    Some(RawListener {
        protocol: Protocol::Udp,
        // ucLocalAddr is plain network-order bytes.
        address: std::net::Ipv6Addr::from(row.ucLocalAddr).into(),
        port: u16::from_be(row.dwLocalPort as u16),
        pid: ProcessId(pid),
        socket_attribution: None,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::{
        checked_row_count, collect_families, row_to_listener, row6_to_listener,
        udp_row_to_listener, udp_row6_to_listener,
    };
    use portspy_core::domain::process::ProcessId;
    use portspy_core::domain::protocol::Protocol;
    use portspy_core::platform::PlatformError;
    use std::mem::{offset_of, size_of};
    use std::net::IpAddr;
    use windows::Win32::Foundation::ERROR_INVALID_DATA;
    use windows::Win32::NetworkManagement::IpHelper::{
        MIB_TCP_STATE_LISTEN, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID, MIB_TCPROW_OWNER_PID,
        MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID,
    };
    use windows::Win32::Networking::WinSock::AF_INET6;

    #[test]
    fn failed_table_query_is_distinct_from_empty_tables() {
        let failure = PlatformError::CallFailed {
            api: "GetExtendedTcpTable",
            code: ERROR_INVALID_DATA.0,
        };
        let result = collect_families(|family| {
            if family == AF_INET6.0 as u32 {
                Err(failure.clone())
            } else {
                Ok(Vec::new())
            }
        });

        assert_eq!(result, Err(failure));
        assert!(
            collect_families(|_| Ok(Vec::new()))
                .expect("both tables succeeded")
                .is_empty()
        );
    }

    #[test]
    fn ipv6_row_count_rejects_buffer_that_only_fits_ipv4_row() {
        const ROWS_OFFSET: usize = offset_of!(MIB_TCP6TABLE_OWNER_PID, table);
        const ROW_SIZE: usize = size_of::<MIB_TCP6ROW_OWNER_PID>();
        const API: &str = "GetExtendedTcpTable";

        let mut buffer =
            vec![0; (ROWS_OFFSET + size_of::<MIB_TCPROW_OWNER_PID>()) / size_of::<u32>()];
        buffer[0] = 1;
        assert_eq!(
            checked_row_count(&buffer, ROWS_OFFSET, ROW_SIZE, API),
            Err(PlatformError::CallFailed {
                api: API,
                code: ERROR_INVALID_DATA.0,
            })
        );

        buffer.resize((ROWS_OFFSET + ROW_SIZE) / size_of::<u32>(), 0);
        assert_eq!(
            checked_row_count(&buffer, ROWS_OFFSET, ROW_SIZE, API),
            Ok(1)
        );
    }

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
        assert_eq!(
            listener.address,
            IpAddr::V4("127.0.0.1".parse().expect("valid"))
        );
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

    #[test]
    fn udp_ipv4_row_converts_with_protocol() {
        // 0.0.0.0 is the typical bound address of a UDP socket.
        let row = MIB_UDPROW_OWNER_PID {
            dwLocalAddr: 0,
            dwLocalPort: u32::from(u16::to_be(5353)),
            dwOwningPid: 7,
        };
        let listener = udp_row_to_listener(&row).expect("row should convert");
        assert_eq!(listener.protocol, Protocol::Udp);
        assert_eq!(
            listener.address,
            IpAddr::V4("0.0.0.0".parse().expect("valid"))
        );
        assert_eq!(listener.port, 5353);
        assert_eq!(listener.pid, ProcessId(7));
    }

    #[test]
    fn udp_ipv4_row_without_owner_is_skipped() {
        let row = MIB_UDPROW_OWNER_PID {
            dwOwningPid: 0,
            ..Default::default()
        };
        assert!(udp_row_to_listener(&row).is_none());
    }

    #[test]
    fn udp_ipv6_row_converts_bytes() {
        // ::1
        let mut row = MIB_UDP6ROW_OWNER_PID {
            dwLocalPort: u32::from(u16::to_be(1900)),
            dwOwningPid: 42,
            ..Default::default()
        };
        row.ucLocalAddr[15] = 1;
        let listener = udp_row6_to_listener(&row).expect("row should convert");
        assert_eq!(listener.protocol, Protocol::Udp);
        assert_eq!(listener.address, IpAddr::V6("::1".parse().expect("valid")));
        assert_eq!(listener.port, 1900);
        assert_eq!(listener.pid, ProcessId(42));
    }

    #[test]
    fn udp_port_conversion_handles_high_ports() {
        let row = MIB_UDPROW_OWNER_PID {
            dwLocalPort: u32::from(u16::to_be(65535)),
            dwOwningPid: 1,
            ..Default::default()
        };
        let listener = udp_row_to_listener(&row).expect("row should convert");
        assert_eq!(listener.port, 65535);
    }
}
