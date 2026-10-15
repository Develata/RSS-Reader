//! Outbound access policy (v0.2 phase C, C1): which destinations a server may reach.
//!
//! Desktop, Android and an ordinary CLI reach any address ([`OutboundPolicy::Unrestricted`]). A
//! server reaches public addresses only ([`OutboundPolicy::PublicOnly`]), with the exceptions its
//! policy file names. A public-only request is checked at every hop, in three places, none of which
//! covers the others:
//!
//! 1. before it is sent, [`OutboundPolicy::check_url`] checks the scheme and a literal address
//!    (a literal IP never reaches a resolver);
//! 2. a host name resolves through [`OutboundPolicy::client_builder`]'s resolver, which fails the
//!    request when any answer is refused (no mixed answers, no rebinding to a refused one);
//! 3. every redirect target goes through [`OutboundPolicy::redirect`], which checks it like (1).
//!
//! Public-only clients never use a proxy: a proxy would resolve and connect in our place.
//!
//! What counts as public is [`is_public_address`]. A network-specific NAT64 prefix inside ordinary
//! global space cannot be recognised from the address: a deployment that translates such a prefix
//! to private IPv4 must block it in its egress rules, or list it under `deny`.

mod address;
mod policy_file;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::redirect::{Action, Attempt, Policy};
use url::{Host, Url};

pub use address::is_public_address;
pub use policy_file::{POLICY_FILE, ensure_server_policy, load_policy};

/// Where outbound requests may go.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OutboundPolicy {
    #[default]
    Unrestricted,
    PublicOnly(PublicOnly),
}

/// Public addresses only, with explicit exceptions; `deny` wins over everything.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublicOnly {
    /// Host names whose resolved addresses are allowed (an internal WebDAV server, say),
    /// lower case and without a trailing dot.
    allow_hosts: Vec<String>,
    allow_networks: Vec<Network>,
    deny_networks: Vec<Network>,
}

impl PublicOnly {
    pub fn new(allow_hosts: Vec<String>, allow_networks: Vec<Network>, deny: Vec<Network>) -> Self {
        let allow_hosts = allow_hosts.iter().map(|host| normalized_host(host)).collect();
        Self { allow_hosts, allow_networks, deny_networks: deny }
    }

    /// Whether `ip`, reached as `host` (`None` for a literal address), may be connected to.
    pub fn allows(&self, host: Option<&str>, ip: IpAddr) -> bool {
        if self.deny_networks.iter().any(|network| network.contains(ip)) {
            return false;
        }
        let allowed_host = host.is_some_and(|host| {
            let host = normalized_host(host);
            self.allow_hosts.contains(&host)
        });
        allowed_host
            || self.allow_networks.iter().any(|network| network.contains(ip))
            || is_public_address(ip)
    }

    /// The addresses `host` resolved to, all of them allowed; one refused answer refuses the
    /// whole request, so the connection cannot fall back to it.
    fn checked(&self, host: &str, addresses: Vec<SocketAddr>) -> Result<Vec<SocketAddr>, Refused> {
        if addresses.is_empty() {
            return Err(Refused(format!("{host}（没有解析结果）")));
        }
        match addresses.iter().find(|address| !self.allows(Some(host), address.ip())) {
            Some(refused) => Err(Refused(format!("{host}（{}）", refused.ip()))),
            None => Ok(addresses),
        }
    }
}

/// A network in CIDR notation, or a single address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Network {
    address: IpAddr,
    prefix: u8,
}

impl Network {
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.address, ip) {
            (IpAddr::V4(network), IpAddr::V4(ip)) => {
                self.prefix == 0
                    || (u32::from(network) ^ u32::from(ip)) >> (32 - u32::from(self.prefix)) == 0
            }
            (IpAddr::V6(network), IpAddr::V6(ip)) => {
                self.prefix == 0
                    || (u128::from(network) ^ u128::from(ip)) >> (128 - u32::from(self.prefix)) == 0
            }
            _ => false,
        }
    }
}

impl std::str::FromStr for Network {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || format!("不是地址或 CIDR 网段：{text:?}");
        let (address, prefix) = match text.split_once('/') {
            Some((address, prefix)) => (address, Some(prefix)),
            None => (text, None),
        };
        let address: IpAddr = address.parse().map_err(|_| invalid())?;
        let width = if address.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            Some(prefix) => prefix.parse::<u8>().map_err(|_| invalid())?,
            None => width,
        };
        if prefix > width {
            return Err(invalid());
        }
        Ok(Self { address, prefix })
    }
}

/// A destination the policy does not allow.
#[derive(Debug, thiserror::Error)]
#[error("出站访问策略不允许访问 {0}")]
pub struct Refused(String);

impl OutboundPolicy {
    /// Checks a URL before it is requested, and every redirect target: only http(s), and a literal
    /// address must be allowed. A host name is checked when it resolves.
    pub fn check_url(&self, url: &Url) -> Result<(), Refused> {
        let Self::PublicOnly(policy) = self else { return Ok(()) };
        let refused = || Refused(url.as_str().to_owned());
        if !matches!(url.scheme(), "http" | "https") {
            return Err(refused());
        }
        match url.host() {
            Some(Host::Domain(_)) => Ok(()),
            Some(Host::Ipv4(ip)) if policy.allows(None, IpAddr::V4(ip)) => Ok(()),
            Some(Host::Ipv6(ip)) if policy.allows(None, IpAddr::V6(ip)) => Ok(()),
            _ => Err(refused()),
        }
    }

    /// A client builder for this policy: a public-only one resolves through the policy and uses
    /// no proxy. Building a client from it must not fall back to a default client on failure.
    pub fn client_builder(&self) -> reqwest::ClientBuilder {
        let builder = reqwest::Client::builder();
        match self {
            Self::Unrestricted => builder,
            Self::PublicOnly(policy) => {
                builder.no_proxy().dns_resolver(Arc::new(PolicyResolver(policy.clone())))
            }
        }
    }

    /// The redirect policy of an operation (`next`), with every target checked first.
    pub fn redirect(&self, next: impl Fn(Attempt) -> Action + Send + Sync + 'static) -> Policy {
        let policy = self.clone();
        Policy::custom(move |attempt| match policy.check_url(attempt.url()) {
            Ok(()) => next(attempt),
            Err(refused) => attempt.error(refused),
        })
    }
}

struct PolicyResolver(PublicOnly);

impl Resolve for PolicyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let policy = self.0.clone();
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let resolved: std::io::Result<Vec<SocketAddr>> =
                tokio::net::lookup_host((host.as_str(), 0)).await.map(Iterator::collect);
            let addresses = policy.checked(&host, resolved?)?;
            Ok::<Addrs, Box<dyn std::error::Error + Send + Sync>>(Box::new(addresses.into_iter()))
        })
    }
}

/// Lower case, without the trailing dot of a fully qualified name.
fn normalized_host(host: &str) -> String {
    host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use url::Url;

    use super::{Network, OutboundPolicy, PublicOnly};

    fn public_only(allow_hosts: &[&str], allow: &[&str], deny: &[&str]) -> OutboundPolicy {
        let networks = |list: &[&str]| list.iter().map(|text| text.parse().unwrap()).collect();
        OutboundPolicy::PublicOnly(PublicOnly::new(
            allow_hosts.iter().map(|host| (*host).to_owned()).collect(),
            networks(allow),
            networks(deny),
        ))
    }

    /// Literal addresses are checked before a request is sent, in every form the URL parser
    /// accepts for them (decimal, octal, hexadecimal and shortened IPv4, IPv6 with an embedded
    /// IPv4); host names wait for their resolution; only http(s) is allowed.
    #[test]
    fn urls_are_checked_by_scheme_and_literal_address() {
        let policy = public_only(&[], &[], &[]);
        let check = |raw: &str| policy.check_url(&Url::parse(raw).unwrap()).is_ok();
        for allowed in [
            "https://example.com/feed.xml",
            "http://localhost./feed.xml",
            "https://93.184.215.14/feed.xml",
            "https://[2606:4700:4700::1111]/",
        ] {
            assert!(check(allowed), "{allowed}");
        }
        for refused in [
            "http://127.0.0.1:8080/",
            "http://2130706433/",
            "http://0x7f.1/",
            "http://0177.0.0.1/",
            "http://127.1/",
            "http://[::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:93.184.215.14]/",
            "http://169.254.169.254/latest/meta-data/",
            "http://[fe80::1]/",
            "file:///etc/passwd",
            "ftp://example.com/",
            "data:text/plain,hello",
        ] {
            assert!(!check(refused), "{refused}");
        }
        assert!(
            OutboundPolicy::Unrestricted
                .check_url(&Url::parse("http://127.0.0.1/").unwrap())
                .is_ok()
        );
    }

    /// Exceptions: an allowed network or host name lets a private address through; a denied
    /// network refuses even a public one and wins over the exceptions.
    #[test]
    fn allow_and_deny_lists() {
        let OutboundPolicy::PublicOnly(policy) =
            public_only(&["DAV.Internal."], &["10.1.0.0/16"], &["93.184.0.0/16", "10.1.2.0/24"])
        else {
            unreachable!()
        };
        let ip = |raw: &str| raw.parse().unwrap();
        assert!(policy.allows(None, ip("10.1.9.9")));
        assert!(!policy.allows(None, ip("10.2.0.1")));
        assert!(!policy.allows(None, ip("10.1.2.3")), "deny wins over an allowed network");
        assert!(policy.allows(Some("dav.internal"), ip("192.168.1.5")));
        assert!(policy.allows(Some("dav.internal."), ip("192.168.1.5")));
        assert!(!policy.allows(Some("other.internal"), ip("192.168.1.5")));
        assert!(!policy.allows(Some("dav.internal"), ip("10.1.2.3")), "deny wins over a host");
        assert!(!policy.allows(None, ip("93.184.215.14")), "a denied public network");
        assert!(policy.allows(None, ip("1.1.1.1")));
    }

    /// One refused answer refuses the whole resolution: the connection must not be able to fall
    /// back to it (a rebinding answer next to a public one).
    #[test]
    fn a_resolution_with_any_refused_answer_is_refused() {
        let OutboundPolicy::PublicOnly(policy) = public_only(&[], &[], &[]) else { unreachable!() };
        let address = |raw: &str| SocketAddr::new(raw.parse().unwrap(), 0);
        let public = vec![address("1.1.1.1"), address("2606:4700:4700::1111")];
        assert_eq!(policy.checked("example.com", public.clone()).unwrap(), public);
        assert!(
            policy.checked("example.com", vec![address("1.1.1.1"), address("10.0.0.1")]).is_err()
        );
        assert!(policy.checked("example.com", Vec::new()).is_err());
    }

    /// A real resolution through the policy's resolver: `localhost` is refused unless allowed by
    /// name.
    #[tokio::test]
    async fn the_resolver_refuses_names_that_resolve_to_refused_addresses() {
        use reqwest::dns::Resolve;
        let resolve = |policy: OutboundPolicy| async move {
            let OutboundPolicy::PublicOnly(policy) = policy else { unreachable!() };
            super::PolicyResolver(policy).resolve("localhost".parse().unwrap()).await
        };
        assert!(resolve(public_only(&[], &[], &[])).await.is_err());
        let allowed = resolve(public_only(&["localhost"], &[], &[])).await.unwrap();
        assert!(allowed.into_iter().all(|address| address.ip().is_loopback()));
    }

    #[test]
    fn networks_parse_as_cidr_or_single_addresses() {
        let network: Network = "10.0.0.0/8".parse().unwrap();
        assert!(network.contains("10.255.0.1".parse().unwrap()));
        assert!(!network.contains("11.0.0.1".parse().unwrap()));
        let single: Network = "192.168.1.5".parse().unwrap();
        assert!(single.contains("192.168.1.5".parse().unwrap()));
        assert!(!single.contains("192.168.1.6".parse().unwrap()));
        let v6: Network = "fd00::/8".parse().unwrap();
        assert!(v6.contains("fd12::1".parse().unwrap()));
        assert!(!v6.contains("10.0.0.1".parse().unwrap()), "families never mix");
        for invalid in ["10.0.0.0/33", "fd00::/129", "example.com", "10.0.0.0/x", ""] {
            assert!(invalid.parse::<Network>().is_err(), "{invalid}");
        }
    }
}
