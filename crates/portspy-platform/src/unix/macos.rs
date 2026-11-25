//! Socket discovery through the macOS process file-descriptor API.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use libproc::bsd_info::BSDInfo;
use libproc::file_info::{ListFDs, ProcFDType, pidfdinfo};
use libproc::net_info::{SocketFDInfo, SocketInfo, SocketInfoKind, TcpSIState};
use libproc::proc_pid::{listpidinfo, pidinfo};
use libproc::processes::{ProcFilter, pids_by_type};
use portspy_core::domain::process::ProcessId;
use portspy_core::domain::protocol::Protocol;
use portspy_core::platform::{PlatformError, RawListener};

pub(super) fn listeners() -> Result<Vec<RawListener>, PlatformError> {
    let pids = pids_by_type(ProcFilter::All).map_err(|error| PlatformError::OperationFailed {
        operation: "socket discovery",
        message: error.to_string(),
    })?;
    let mut seen = HashSet::new();
    let mut listeners = Vec::new();
    for pid in pids {
        let Ok(native_pid) = i32::try_from(pid) else {
            continue;
        };
        let Ok(info) = pidinfo::<BSDInfo>(native_pid, 0) else {
            continue;
        };
        let Ok(descriptors) = listpidinfo::<ListFDs>(native_pid, info.pbi_nfiles as usize) else {
            continue;
        };
        for fd in descriptors {
            if !matches!(ProcFDType::from(fd.proc_fdtype), ProcFDType::Socket) {
                continue;
            }
            let Ok(socket) = pidfdinfo::<SocketFDInfo>(native_pid, fd.proc_fd) else {
                continue;
            };
            if let Some(listener) = socket_to_listener(socket.psi, ProcessId(pid))
                && seen.insert((listener.protocol, listener.address, listener.port, pid))
            {
                listeners.push(listener);
            }
        }
    }
    Ok(listeners)
}

fn socket_to_listener(socket: SocketInfo, pid: ProcessId) -> Option<RawListener> {
    if !matches!(socket.soi_family, libc::AF_INET | libc::AF_INET6) {
        return None;
    }

    let (protocol, address_info) = match SocketInfoKind::from(socket.soi_kind) {
        SocketInfoKind::Tcp if socket.soi_protocol == libc::IPPROTO_TCP => {
            // Safety: the socket kind identifies the active TCP union field.
            let tcp = unsafe { socket.soi_proto.pri_tcp };
            if !matches!(TcpSIState::from(tcp.tcpsi_state), TcpSIState::Listen) {
                return None;
            }
            (Protocol::Tcp, tcp.tcpsi_ini)
        }
        SocketInfoKind::In if socket.soi_protocol == libc::IPPROTO_UDP => {
            // Safety: the socket kind identifies the active IP union field.
            (Protocol::Udp, unsafe { socket.soi_proto.pri_in })
        }
        _ => return None,
    };
    let port = u16::from_be(address_info.insi_lport as u16);
    if port == 0 {
        return None;
    }
    let address = if socket.soi_family == libc::AF_INET {
        // Safety: the IPv4 socket family selects the IPv4 address field.
        let address = unsafe { address_info.insi_laddr.ina_46.i46a_addr4.s_addr };
        IpAddr::V4(Ipv4Addr::from(address.to_ne_bytes()))
    } else {
        // Safety: the IPv6 socket family selects the IPv6 address field.
        let address = unsafe { address_info.insi_laddr.ina_6.s6_addr };
        IpAddr::V6(Ipv6Addr::from(address))
    };

    Some(RawListener {
        protocol,
        address,
        port,
        pid,
    })
}
