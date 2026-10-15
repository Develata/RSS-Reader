//! Outbound access policy (v0.2 phase C, C1): which destinations a server may reach.
//!
//! [`is_public_address`] is an allowlist of globally routable unicast addresses, after the IANA
//! special-purpose address registries. Anything else is refused, including every IPv6 form that
//! embeds an IPv4 address (mapped, compatible, NAT64, 6to4, Teredo): such forms reach public hosts
//! only through translation, and a translator can just as well reach private ones.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Whether `ip` is a globally routable unicast address a server may connect to.
///
/// Refused IPv4 ranges: `0/8`, `10/8`, `100.64/10`, `127/8`, `169.254/16`, `172.16/12`,
/// `192.0.0/24`, `192.0.2/24`, `192.88.99/24`, `192.168/16`, `198.18/15`, `198.51.100/24`,
/// `203.0.113/24`, `224/4` and `240/4` (which holds `255.255.255.255`).
///
/// IPv6: only `2000::/3`, without `2001::/23` (IETF protocol assignments, Teredo among them),
/// `2001:db8::/32` (documentation) and `2002::/16` (6to4). This also refuses `::/128`, `::1`,
/// `::ffff:0:0/96`, `::/96`, `64:ff9b::/96`, `64:ff9b:1::/48`, `100::/64`, `fc00::/7`,
/// `fe80::/10`, `fec0::/10` and `ff00::/8`, all outside `2000::/3`. A few anycast services inside
/// `192.0.0/24` and `2001::/23` are refused with their ranges; a feed reader does not need them.
pub fn is_public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

const REFUSED_IPV4: [(Ipv4Addr, u8); 15] = [
    (Ipv4Addr::new(0, 0, 0, 0), 8),
    (Ipv4Addr::new(10, 0, 0, 0), 8),
    (Ipv4Addr::new(100, 64, 0, 0), 10),
    (Ipv4Addr::new(127, 0, 0, 0), 8),
    (Ipv4Addr::new(169, 254, 0, 0), 16),
    (Ipv4Addr::new(172, 16, 0, 0), 12),
    (Ipv4Addr::new(192, 0, 0, 0), 24),
    (Ipv4Addr::new(192, 0, 2, 0), 24),
    (Ipv4Addr::new(192, 88, 99, 0), 24),
    (Ipv4Addr::new(192, 168, 0, 0), 16),
    (Ipv4Addr::new(198, 18, 0, 0), 15),
    (Ipv4Addr::new(198, 51, 100, 0), 24),
    (Ipv4Addr::new(203, 0, 113, 0), 24),
    (Ipv4Addr::new(224, 0, 0, 0), 4),
    (Ipv4Addr::new(240, 0, 0, 0), 4),
];

const REFUSED_IN_GLOBAL_IPV6: [(Ipv6Addr, u8); 3] = [
    (Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 23),
    (Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 0), 16),
];

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    !REFUSED_IPV4.iter().any(|&(network, prefix)| in_ipv4_network(ip, network, prefix))
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    in_ipv6_network(ip, Ipv6Addr::new(0x2000, 0, 0, 0, 0, 0, 0, 0), 3)
        && !REFUSED_IN_GLOBAL_IPV6
            .iter()
            .any(|&(network, prefix)| in_ipv6_network(ip, network, prefix))
}

/// Whether `ip` lies in `network/prefix`; the prefixes above are between 3 and 24 bits.
fn in_ipv4_network(ip: Ipv4Addr, network: Ipv4Addr, prefix: u8) -> bool {
    (u32::from(ip) ^ u32::from(network)) >> (32 - u32::from(prefix)) == 0
}

fn in_ipv6_network(ip: Ipv6Addr, network: Ipv6Addr, prefix: u8) -> bool {
    (u128::from(ip) ^ u128::from(network)) >> (128 - u32::from(prefix)) == 0
}

#[cfg(test)]
mod tests {
    use super::is_public_address;

    /// Every special-purpose form is refused; ordinary public hosts are accepted. The NAT64,
    /// 6to4, Teredo and IPv4-mapped forms embed `10.0.0.1` or another address, public or not:
    /// they are refused either way.
    #[test]
    fn only_globally_routable_unicast_addresses_are_public() {
        let public = [
            "1.1.1.1",
            "8.8.8.8",
            "93.184.215.14",
            "100.63.255.255",
            "100.128.0.0",
            "172.15.255.255",
            "172.32.0.0",
            "192.0.1.1",
            "192.88.98.1",
            "198.17.255.255",
            "198.20.0.0",
            "223.255.255.255",
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
            "2a00:1450:4001:80b::200e",
            "2001:200::1",
            "2003::1",
        ];
        let refused = [
            "0.0.0.0",
            "0.1.2.3",
            "10.0.0.1",
            "100.64.0.1",
            "100.127.255.255",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.255",
            "192.0.0.1",
            "192.0.0.9",
            "192.0.2.1",
            "192.88.99.1",
            "192.168.1.1",
            "198.18.0.1",
            "198.19.255.255",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "239.255.255.250",
            "240.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "::ffff:192.0.0.1",
            "::ffff:1.2.3.4",
            "::10.0.0.1",
            "64:ff9b::a00:1",
            "64:ff9b::808:808",
            "64:ff9b:1::a00:1",
            "100::1",
            "2001::1",
            "2001:0:4136:e378:8000:63bf:3fff:fdd2",
            "2001:1ff::1",
            "2001:db8::1",
            "2002:a00:1::1",
            "2002:808:808::1",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "fec0::1",
            "ff02::1",
            "1fff:ffff::1",
            "4000::1",
        ];
        for raw in public {
            assert!(is_public_address(raw.parse().unwrap()), "{raw} should be public");
        }
        for raw in refused {
            assert!(!is_public_address(raw.parse().unwrap()), "{raw} should be refused");
        }
    }
}
