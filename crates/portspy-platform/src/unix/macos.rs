//! macOS socket and process discovery.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use libproc::bsd_info::BSDInfo;
use libproc::file_info::{ListFDs, ProcFDType, pidfdinfo};
use libproc::net_info::{SocketFDInfo, SocketInfo, SocketInfoKind, TcpSIState};
use libproc::proc_pid::{listpidinfo, pidinfo};
use libproc::processes::{ProcFilter, pids_by_type};
use portspy_core::domain::process::{ProcessId, ProcessIdentity};
use portspy_core::domain::protocol::Protocol;
use portspy_core::platform::{KillOutcome, PlatformError, RawListener, RawProcess};
use rustix::process::Pid;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

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
        socket_attribution: None,
    })
}

pub(super) fn processes() -> Result<Vec<RawProcess>, PlatformError> {
    let discovery_error = |message| PlatformError::OperationFailed {
        operation: "process discovery",
        message,
    };
    let before: HashMap<_, _> = pids_by_type(ProcFilter::All)
        .map_err(|error| discovery_error(error.to_string()))?
        .into_iter()
        .filter_map(|pid| Some((pid, process_start_time(pid)?)))
        .collect();
    let refresh = ProcessRefreshKind::nothing()
        .with_memory()
        .with_exe(UpdateKind::Always)
        .with_cwd(UpdateKind::Always)
        .with_cmd(UpdateKind::Always);
    let mut system = System::new().map_err(|error| discovery_error(error.to_string()))?;
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh);

    Ok(system
        .processes()
        .values()
        .filter_map(|process| {
            let pid = process.pid().as_u32();
            // Both high-resolution observations must name the same instance.
            // A start time retrieved only after the metadata may belong to a
            // replacement process that inherited the PID during collection.
            let started_at = *before.get(&pid)?;
            if process_start_time(pid) != Some(started_at) {
                return None;
            }
            let command_line: Vec<_> = process
                .cmd()
                .iter()
                .map(|arg| arg.to_string_lossy())
                .collect();
            Some(RawProcess {
                pid: ProcessId(pid),
                parent_pid: process.parent().map(|parent| ProcessId(parent.as_u32())),
                name: process.name().to_string_lossy().into_owned(),
                executable: process.exe().map(ToOwned::to_owned),
                command_line: (!command_line.is_empty()).then(|| command_line.join(" ")),
                working_directory: process.cwd().map(ToOwned::to_owned),
                started_at: Some(started_at),
                memory_bytes: Some(process.memory()),
                cpu_percent: None,
            })
        })
        .collect())
}

fn process_start_time(pid: u32) -> Option<SystemTime> {
    let info = pidinfo::<BSDInfo>(i32::try_from(pid).ok()?, 0).ok()?;
    if info.pbi_start_tvsec == 0 {
        return None;
    }
    UNIX_EPOCH
        .checked_add(Duration::from_secs(info.pbi_start_tvsec))?
        .checked_add(Duration::from_micros(info.pbi_start_tvusec))
}

pub(super) fn kill_checked(_: Pid, _: ProcessIdentity) -> Result<KillOutcome, PlatformError> {
    // Sending a signal by PID can target a replacement even when a separate
    // start-time check immediately preceded it.
    Err(PlatformError::Unsupported(
        "identity-checked termination on macOS",
    ))
}
