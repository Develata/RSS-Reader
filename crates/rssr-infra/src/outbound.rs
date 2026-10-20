//! 原生出站请求的目标地址策略。
//!
//! 桌面端替用户访问用户自己给出的地址，局域网 feed 是正常用法，默认不限制。服务端替已登录用户
//! 访问用户给出的地址时，同样的请求就成了从服务器发往内网的请求，所以服务端默认只放行公网地址。
//!
//! [`OutboundPolicy::PublicOnly`] 分三处生效：
//!
//! 1. 域名：自定义 DNS 解析器只要有一条解析结果落在内网就整体拒绝。HTTP 客户端直接连接解析器
//!    返回的地址，所以"校验的地址"和"实际连接的地址"是同一个，DNS rebinding 无从插手。
//! 2. IP 字面量：连接器遇到 IP 字面量不会调用解析器，因此首跳由调用方用
//!    [`OutboundPolicy::check_url`] 检查，每次重定向由客户端的重定向策略检查。
//! 3. 环境代理：经代理时目标地址由代理解析，上面两条都无法生效，所以不使用环境代理。
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
};

use anyhow::bail;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use url::{Host, Url};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutboundPolicy {
    /// 不限制目标地址，并沿用环境代理。桌面、Android 与 CLI 的行为。
    #[default]
    Unrestricted,
    /// 只访问公网地址，不使用环境代理。服务端默认值。
    PublicOnly,
}

impl OutboundPolicy {
    /// 检查一次请求的目标 URL。只看 IP 字面量；域名交给解析器检查。
    pub fn check_url(self, url: &Url) -> anyhow::Result<()> {
        if self == Self::Unrestricted {
            return Ok(());
        }
        let ip = match url.host() {
            Some(Host::Ipv4(ip)) => IpAddr::V4(ip),
            Some(Host::Ipv6(ip)) => IpAddr::V6(ip),
            Some(Host::Domain(_)) | None => return Ok(()),
        };
        if is_disallowed_ip(ip) {
            bail!("出于安全原因，服务端不访问内网或本地地址（{ip}）。");
        }
        Ok(())
    }

    pub(crate) fn redirect_allowed(self, url: &Url) -> bool {
        self.check_url(url).is_ok()
    }

    /// 给客户端装上本策略的解析器与代理设置；重定向检查由各客户端自己的重定向策略调用
    /// [`Self::redirect_allowed`]。
    pub(crate) fn configure(self, builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        match self {
            Self::Unrestricted => builder,
            Self::PublicOnly => builder
                .no_proxy()
                .dns_resolver(Arc::new(PublicOnlyResolver { disallowed: is_disallowed_ip })),
        }
    }
}

struct PublicOnlyResolver {
    /// 生产中恒为 [`is_disallowed_ip`]；测试换成放行回环地址，以便走通成功路径。
    disallowed: fn(IpAddr) -> bool,
}

impl Resolve for PublicOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        let disallowed = self.disallowed;
        Box::pin(async move {
            // 端口占位为 0，客户端会换成请求的实际端口。
            let addrs: Vec<SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            if let Some(addr) = addrs.iter().find(|addr| disallowed(addr.ip())) {
                return Err(std::io::Error::other(format!(
                    "出于安全原因，服务端不访问内网或本地地址（{host} 解析为 {}）。",
                    addr.ip()
                ))
                .into());
            }
            let addrs: Addrs = Box::new(addrs.into_iter());
            Ok(addrs)
        })
    }
}

/// 不允许服务端访问的地址：内网、本机、链路本地、组播、文档与基准测试保留段、
/// 运营商共享地址段，以及它们的 IPv4 映射 IPv6 形式。
pub fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.octets()[0] == 0
                || ip.octets()[0] >= 240
                || ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_unspecified()
                || ip.is_multicast()
                || is_documentation_ipv4(ip)
                || is_shared_address_space_ipv4(ip)
                || is_benchmark_ipv4(ip)
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || is_documentation_ipv6(ip)
                || ip.to_ipv4().is_some_and(|v4| is_disallowed_ip(IpAddr::V4(v4)))
        }
    }
}

fn is_documentation_ipv4(ip: Ipv4Addr) -> bool {
    matches!(
        (ip.octets()[0], ip.octets()[1], ip.octets()[2]),
        (192, 0, 2) | (198, 51, 100) | (203, 0, 113)
    )
}

fn is_shared_address_space_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

fn is_benchmark_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 198 && (octets[1] == 18 || octets[1] == 19)
}

fn is_documentation_ipv6(ip: Ipv6Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 0x20 && octets[1] == 0x01 && octets[2] == 0x0d && octets[3] == 0xb8
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use url::Url;

    use super::{OutboundPolicy, PublicOnlyResolver, is_disallowed_ip};

    #[test]
    fn disallowed_ip_covers_private_loopback_and_mapped_forms() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:192.168.0.1",
        ] {
            assert!(is_disallowed_ip(ip.parse::<IpAddr>().unwrap()), "{ip} must be disallowed");
        }
        for ip in ["1.1.1.1", "93.184.216.34", "2606:4700:4700::1111"] {
            assert!(!is_disallowed_ip(ip.parse::<IpAddr>().unwrap()), "{ip} must be allowed");
        }
    }

    #[test]
    fn public_only_rejects_private_ip_literals_but_leaves_domains_to_the_resolver() {
        let policy = OutboundPolicy::PublicOnly;
        for url in ["http://127.0.0.1:8080/feed.xml", "https://[::1]/", "http://192.168.0.2/"] {
            assert!(policy.check_url(&Url::parse(url).unwrap()).is_err(), "{url}");
        }
        for url in ["https://1.1.1.1/feed", "https://example.com/feed.xml"] {
            assert!(policy.check_url(&Url::parse(url).unwrap()).is_ok(), "{url}");
        }
        let local = Url::parse("http://127.0.0.1/").unwrap();
        assert!(OutboundPolicy::Unrestricted.check_url(&local).is_ok());
    }

    /// 走通解析器的成功路径：解析结果的端口占位 0 必须被换成请求的端口。
    #[tokio::test]
    async fn resolver_success_path_connects_to_the_requested_port() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(std::sync::Arc::new(PublicOnlyResolver { disallowed: |_| false }))
            .build()
            .unwrap();

        let body = client
            .get(format!("http://localhost:{port}/"))
            .send()
            .await
            .expect("resolved request must reach the listener")
            .text()
            .await
            .unwrap();
        assert_eq!(body, "ok");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn public_only_resolver_rejects_names_that_resolve_to_loopback() {
        let client =
            OutboundPolicy::PublicOnly.configure(reqwest::Client::builder()).build().unwrap();
        let error = client
            .get("http://localhost:9/")
            .send()
            .await
            .expect_err("localhost must not be reachable under PublicOnly");
        let chain = format!("{:?}", anyhow::Error::from(error));
        assert!(chain.contains("内网或本地地址"), "unexpected error: {chain}");
    }
}
