//! Socket discovery through the current Linux network namespace.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use portspy_core::domain::process::ProcessId;
use portspy_core::domain::protocol::Protocol;
use portspy_core::platform::{PlatformError, RawListener};
use procfs::net::{self, TcpState};
use procfs::process::{FDTarget, all_processes};
use procfs::{ProcError, ProcResult};

pub(super) fn listeners() -> Result<Vec<RawListener>, PlatformError> {
    let tcp = net::tcp().map_err(discovery_error)?;
    let tcp6 = optional_table(net::tcp6())?;
    let udp = net::udp().map_err(discovery_error)?;
    let udp6 = optional_table(net::udp6())?;
    let sockets: Vec<_> = tcp
        .into_iter()
        .chain(tcp6)
        .filter(|row| row.state == TcpState::Listen)
        .map(|row| (row.inode, Protocol::Tcp, row.local_address))
        .chain(
            udp.into_iter()
                .chain(udp6)
                .map(|row| (row.inode, Protocol::Udp, row.local_address)),
        )
        .filter(|(_, _, address)| address.port() != 0)
        .collect();
    let mut owners: HashMap<_, HashSet<_>> = sockets
        .iter()
        .map(|(inode, _, _)| (*inode, HashSet::new()))
        .collect();

    for process in all_processes().map_err(discovery_error)?.flatten() {
        let Ok(pid) = u32::try_from(process.pid()) else {
            continue;
        };
        let Ok(descriptors) = process.fd() else {
            continue;
        };
        for descriptor in descriptors.flatten() {
            if let FDTarget::Socket(inode) = descriptor.target
                && let Some(pids) = owners.get_mut(&inode)
            {
                pids.insert(ProcessId(pid));
            }
        }
    }

    let mut seen = HashSet::new();
    let mut listeners = Vec::new();
    for (inode, protocol, address) in sockets {
        let pids = owners.get(&inode).into_iter().flatten();
        if pids.clone().next().is_none() {
            push_listener(&mut seen, &mut listeners, protocol, address, ProcessId(0));
        }
        for &pid in pids {
            push_listener(&mut seen, &mut listeners, protocol, address, pid);
        }
    }
    Ok(listeners)
}

fn push_listener(
    seen: &mut HashSet<(Protocol, SocketAddr, ProcessId)>,
    listeners: &mut Vec<RawListener>,
    protocol: Protocol,
    address: SocketAddr,
    pid: ProcessId,
) {
    if seen.insert((protocol, address, pid)) {
        listeners.push(RawListener {
            protocol,
            address: address.ip(),
            port: address.port(),
            pid,
        });
    }
}

fn optional_table<T>(result: ProcResult<Vec<T>>) -> Result<Vec<T>, PlatformError> {
    match result {
        Ok(rows) => Ok(rows),
        Err(ProcError::NotFound(_)) => Ok(Vec::new()),
        Err(error) => Err(discovery_error(error)),
    }
}

fn discovery_error(error: ProcError) -> PlatformError {
    PlatformError::OperationFailed {
        operation: "socket discovery",
        message: error.to_string(),
    }
}
