//! Linux socket diagnostics. The wire layout follows linux/{inet_diag,netlink}.h.

use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use portspy_core::domain::protocol::Protocol;
use rustix::net::netlink::{SOCK_DIAG, SocketAddrNetlink};
use rustix::net::sockopt::{Timeout, set_socket_timeout};
use rustix::net::{
    AddressFamily, RecvFlags, SendFlags, SocketFlags, SocketType, connect, recvfrom, send,
    socket_with,
};

const SOCK_DIAG_BY_FAMILY: u16 = 20;
const INET_DIAG_CGROUP_ID: u16 = 21;
const NLMSG_DONE: u16 = 3;
const NLM_F_DUMP_REQUEST: u16 = 0x301;
const NLM_F_DUMP_INTR: u16 = 0x10;
const AF_INET: u8 = 2;
const AF_INET6: u8 = 10;
const TCP_LISTEN: u32 = 1 << 10;
const NETLINK_HEADER: usize = 16;
const DIAG_REQUEST: usize = 56;
const DIAG_MESSAGE: usize = 72;

pub(super) type SocketKey = (u64, Protocol, SocketAddr);

/// Get the kernel cgroup ID for unattributed sockets, keyed by inode and endpoint.
pub(super) fn socket_cgroups(wanted: &HashSet<SocketKey>) -> HashMap<SocketKey, u64> {
    let mut groups = HashMap::new();
    for (family, ipv6) in [(AF_INET, false), (AF_INET6, true)] {
        for protocol in [Protocol::Tcp, Protocol::Udp] {
            if wanted
                .iter()
                .any(|(_, p, addr)| *p == protocol && addr.is_ipv6() == ipv6)
                && let Ok(found) = dump_cgroups(family, protocol, wanted)
            {
                groups.extend(found);
            }
        }
    }
    groups
}

fn dump_cgroups(
    family: u8,
    protocol: Protocol,
    wanted: &HashSet<SocketKey>,
) -> io::Result<HashMap<SocketKey, u64>> {
    let socket = socket_with(
        AddressFamily::NETLINK,
        SocketType::RAW,
        SocketFlags::CLOEXEC,
        Some(SOCK_DIAG),
    )?;
    set_socket_timeout(&socket, Timeout::Recv, Some(Duration::from_millis(150)))?;
    connect(&socket, &SocketAddrNetlink::new(0, 0))?;

    // nlmsghdr + inet_diag_req_v2. TCP queries only listeners. UDP may expose
    // bound sockets in several states, so request all and match procfs rows.
    let mut request = [0u8; NETLINK_HEADER + DIAG_REQUEST];
    request[..4].copy_from_slice(&((NETLINK_HEADER + DIAG_REQUEST) as u32).to_ne_bytes());
    request[4..6].copy_from_slice(&SOCK_DIAG_BY_FAMILY.to_ne_bytes());
    request[6..8].copy_from_slice(&NLM_F_DUMP_REQUEST.to_ne_bytes());
    request[16] = family;
    let (number, states) = match protocol {
        Protocol::Tcp => (6, TCP_LISTEN),
        Protocol::Udp => (17, u32::MAX),
    };
    request[17] = number;
    request[20..24].copy_from_slice(&states.to_ne_bytes());
    send(&socket, &request, SendFlags::empty())?;

    let mut buffer = vec![0; 64 * 1024];
    let mut groups = HashMap::new();
    loop {
        let (received, size, sender) = recvfrom(&socket, &mut buffer[..], RecvFlags::TRUNC)?;
        if received == 0
            || received != size
            || !sender
                .and_then(|addr| SocketAddrNetlink::try_from(addr).ok())
                .is_some_and(|addr| addr.pid() == 0)
        {
            return Err(invalid_message());
        }
        if collect_datagram(&buffer[..received], protocol, wanted, &mut groups)
            .ok_or_else(invalid_message)?
        {
            return Ok(groups);
        }
    }
}

fn invalid_message() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid socket diagnostic response",
    )
}

fn collect_datagram(
    mut data: &[u8],
    protocol: Protocol,
    wanted: &HashSet<SocketKey>,
    groups: &mut HashMap<SocketKey, u64>,
) -> Option<bool> {
    while !data.is_empty() {
        let length = u32::from_ne_bytes(at(data, 0)?) as usize;
        let kind = u16::from_ne_bytes(at(data, 4)?);
        let flags = u16::from_ne_bytes(at(data, 6)?);
        if flags & NLM_F_DUMP_INTR != 0 {
            return None;
        }
        let payload = data.get(NETLINK_HEADER..length)?;
        match kind {
            NLMSG_DONE if payload.is_empty() || i32::from_ne_bytes(at(payload, 0)?) == 0 => {
                return Some(true);
            }
            SOCK_DIAG_BY_FAMILY => {
                if let Some((key, group)) = socket_cgroup(payload, protocol)
                    && wanted.contains(&key)
                {
                    groups.insert(key, group);
                }
            }
            _ => return None,
        }
        data = data.get(length.next_multiple_of(4)..)?;
    }
    Some(false)
}

fn socket_cgroup(payload: &[u8], protocol: Protocol) -> Option<(SocketKey, u64)> {
    let inode = u64::from(u32::from_ne_bytes(at(payload, 68)?));
    let port = u16::from_be_bytes(at(payload, 4)?);
    let ip = match *payload.first()? {
        AF_INET => IpAddr::V4(Ipv4Addr::from(at::<4>(payload, 8)?)),
        AF_INET6 => IpAddr::V6(Ipv6Addr::from(at::<16>(payload, 8)?)),
        _ => return None,
    };
    let mut attributes = payload.get(DIAG_MESSAGE..)?;
    while !attributes.is_empty() {
        let len = usize::from(u16::from_ne_bytes(at(attributes, 0)?));
        let kind = u16::from_ne_bytes(at(attributes, 2)?);
        let value = attributes.get(4..len)?;
        if kind == INET_DIAG_CGROUP_ID {
            let group = u64::from_ne_bytes(value.try_into().ok()?);
            return Some(((inode, protocol, SocketAddr::new(ip, port)), group));
        }
        attributes = attributes.get(len.next_multiple_of(4)..)?;
    }
    None
}

fn at<const N: usize>(data: &[u8], offset: usize) -> Option<[u8; N]> {
    data.get(offset..offset + N)?.try_into().ok()
}

/// Look up service unit names via cgroup v2 directory inodes. A scope may
/// contain multiple processes, so this never assigns a PID.
pub(super) fn services(mut wanted: HashSet<u64>) -> HashMap<u64, String> {
    let root = Path::new("/sys/fs/cgroup");
    if !root.join("cgroup.controllers").is_file() {
        return HashMap::new();
    }

    let mut names = HashMap::new();
    let mut pending = vec![(root.to_owned(), None)];
    while !wanted.is_empty()
        && let Some((path, inherited_unit)) = pending.pop()
    {
        let unit = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| name.ends_with(".service") && !name.starts_with("user@"))
            .map(str::to_owned)
            .or(inherited_unit);
        if let Ok(metadata) = path.metadata()
            && wanted.remove(&metadata.ino())
            && let Some(unit) = &unit
        {
            names.insert(metadata.ino(), unit.clone());
        }
        pending.extend(child_directories(&path).map(|child| (child, unit.clone())));
    }
    names
}

fn child_directories(path: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diag_payload(port: u16, inode: u32) -> [u8; DIAG_MESSAGE + 12] {
        let mut payload = [0; DIAG_MESSAGE + 12];
        payload[0] = AF_INET;
        payload[4..6].copy_from_slice(&port.to_be_bytes());
        payload[8..12].copy_from_slice(&Ipv4Addr::LOCALHOST.octets());
        payload[68..72].copy_from_slice(&inode.to_ne_bytes());
        payload[72..74].copy_from_slice(&12u16.to_ne_bytes());
        payload[74..76].copy_from_slice(&INET_DIAG_CGROUP_ID.to_ne_bytes());
        payload[76..84].copy_from_slice(&56u64.to_ne_bytes());
        payload
    }

    #[test]
    fn decodes_socket_cgroup_and_rejects_truncated_attributes() -> io::Result<()> {
        let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = socket.local_addr()?;
        let payload = diag_payload(addr.port(), 42);
        assert_eq!(
            socket_cgroup(&payload, Protocol::Tcp),
            Some(((42, Protocol::Tcp, addr), 56))
        );
        assert_eq!(socket_cgroup(&payload[..83], Protocol::Tcp), None);
        Ok(())
    }

    #[test]
    fn dump_matches_only_the_snapshot_inode_and_endpoint() -> io::Result<()> {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0")?;
        let address = socket.local_addr()?;
        let key = (52, Protocol::Udp, address);
        let mut datagram = [0; NETLINK_HEADER + DIAG_MESSAGE + 12];
        let length = datagram.len() as u32;
        datagram[..4].copy_from_slice(&length.to_ne_bytes());
        datagram[4..6].copy_from_slice(&SOCK_DIAG_BY_FAMILY.to_ne_bytes());
        datagram[NETLINK_HEADER..].copy_from_slice(&diag_payload(address.port(), 52));

        let mut groups = HashMap::new();
        assert_eq!(
            collect_datagram(
                &datagram,
                Protocol::Udp,
                &HashSet::from([(53, Protocol::Udp, address)]),
                &mut groups
            ),
            Some(false)
        );
        assert!(groups.is_empty());
        assert_eq!(
            collect_datagram(&datagram, Protocol::Udp, &HashSet::from([key]), &mut groups),
            Some(false)
        );
        assert_eq!(groups.get(&key), Some(&56));
        Ok(())
    }
}
