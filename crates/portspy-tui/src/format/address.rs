//! Socket address formatting.

use std::net::{IpAddr, SocketAddr};

use portspy_core::ports::Exposure;

/// Short display of a local address: `:3000` for loopback, the full address
/// otherwise.
pub fn short(address: IpAddr, port: u16) -> String {
    match Exposure::of(address) {
        Exposure::Loopback => format!(":{port}"),
        _ => SocketAddr::new(address, port).to_string(),
    }
}

/// Scope label for the listener list and inspector.
pub fn scope(address: IpAddr) -> &'static str {
    match Exposure::of(address) {
        Exposure::Loopback => "Loopback",
        Exposure::Interface => "Interface",
        Exposure::AllInterfaces => "All interfaces",
    }
}

/// Full address for the details view.
pub fn full(address: IpAddr, port: u16) -> String {
    SocketAddr::new(address, port).to_string()
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
        assert_eq!(
            short(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5)), 3000),
            "192.168.1.5:3000"
        );
    }

    #[test]
    fn scopes_are_classified() {
        assert_eq!(scope(IpAddr::V4(Ipv4Addr::LOCALHOST)), "Loopback");
        assert_eq!(scope(IpAddr::V4(Ipv4Addr::UNSPECIFIED)), "All interfaces");
        assert_eq!(scope(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))), "Interface");
    }

    #[test]
    fn full_address_includes_ipv6_brackets() {
        assert_eq!(full(IpAddr::V6(Ipv6Addr::LOCALHOST), 5173), "[::1]:5173");
    }

    #[test]
    fn unspecified_ipv6_is_unambiguous() {
        assert_eq!(short(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 135), "[::]:135");
    }
}
