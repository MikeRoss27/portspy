//! Exposure classification for local addresses.
//!
//! PortSpy distinguishes listeners that are only reachable on loopback from
//! listeners exposed on every interface, because the two are very different
//! security-wise and conceptually.

use std::net::IpAddr;

/// How exposed a listener is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Exposure {
    /// Reachable only from this machine (`127.0.0.1`, `::1`).
    Loopback,
    /// Reachable through a specific interface address.
    Interface,
    /// Bound to all interfaces (`0.0.0.0`, `::`).
    AllInterfaces,
}

impl Exposure {
    /// Classifies a local address.
    pub fn of(address: IpAddr) -> Self {
        match address {
            IpAddr::V4(addr) if addr.is_loopback() => Self::Loopback,
            IpAddr::V6(addr) if addr.is_loopback() => Self::Loopback,
            IpAddr::V4(addr) if addr.is_unspecified() => Self::AllInterfaces,
            IpAddr::V6(addr) if addr.is_unspecified() => Self::AllInterfaces,
            _ => Self::Interface,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Exposure;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    fn v4(octets: [u8; 4]) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(octets))
    }

    fn v6(octets: [u16; 8]) -> IpAddr {
        IpAddr::V6(Ipv6Addr::from(octets))
    }

    #[test]
    fn loopback_v4_is_loopback() {
        assert_eq!(Exposure::of(v4([127, 0, 0, 1])), Exposure::Loopback);
    }

    #[test]
    fn loopback_v6_is_loopback() {
        assert_eq!(Exposure::of(v6([0, 0, 0, 0, 0, 0, 0, 1])), Exposure::Loopback);
    }

    #[test]
    fn unspecified_v4_is_all_interfaces() {
        assert_eq!(Exposure::of(v4([0, 0, 0, 0])), Exposure::AllInterfaces);
    }

    #[test]
    fn unspecified_v6_is_all_interfaces() {
        assert_eq!(Exposure::of(v6([0; 8])), Exposure::AllInterfaces);
    }

    #[test]
    fn lan_address_is_interface() {
        assert_eq!(Exposure::of(v4([192, 168, 1, 42])), Exposure::Interface);
        assert_eq!(Exposure::of(v6([0xfe80, 0, 0, 0, 0, 0, 0, 1])), Exposure::Interface);
    }
}
