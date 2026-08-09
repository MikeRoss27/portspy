//! Socket address formatting.

use std::net::IpAddr;

use portspy_core::ports::Exposure;

/// Short display of a local address: `:3000` for loopback, the full address
/// otherwise.
pub fn short(address: IpAddr, port: u16) -> String {
    match Exposure::of(address) {
        Exposure::Loopback => format!(":{port}"),
        _ => format!("{address}:{port}"),
    }
}

/// Scope label for the details view.
pub fn scope(address: IpAddr) -> &'static str {
    match Exposure::of(address) {
        Exposure::Loopback => "localhost",
        Exposure::Interface => "interface",
        Exposure::AllInterfaces => "all interfaces",
    }
}

/// Full address for the details view.
pub fn full(address: IpAddr, port: u16) -> String {
    format!("{address}:{port}")
}

#[cfg(test)]
mod tests {
    use super::{full, scope, short};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn loopback_v4_is_short() {
        assert_eq!(short(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000), ":3000");
    }

    #[test]
    fn loopback_v6_is_short() {
        assert_eq!(short(IpAddr::V6(Ipv6Addr::LOCALHOST), 3000), ":3000");
    }

    #[test]
    fn lan_address_is_full() {
        assert_eq!(short(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5)), 3000), "192.168.1.5:3000");
    }

    #[test]
    fn scopes_are_classified() {
        assert_eq!(scope(IpAddr::V4(Ipv4Addr::LOCALHOST)), "localhost");
        assert_eq!(scope(IpAddr::V4(Ipv4Addr::UNSPECIFIED)), "all interfaces");
        assert_eq!(scope(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))), "interface");
    }

    #[test]
    fn full_address_includes_ipv6_brackets() {
        assert_eq!(full(IpAddr::V6(Ipv6Addr::LOCALHOST), 5173), "::1:5173");
    }
}
