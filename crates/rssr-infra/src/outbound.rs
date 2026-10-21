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

/// 不允许服务端访问的地址：内网、本机、链路本地、组播、文档与基准测试保留段、运营商共享地址段，
/// 以及把这些 IPv4 地址嵌进 IPv6 的各种转换形式。
///
/// 转换前缀的处理：能解出嵌入 IPv4 的（IPv4 映射/兼容、NAT64 知名前缀 `64:ff9b::/96`、
/// 6to4 `2002::/16`）按嵌入地址判断；无法安全判断或只供本地使用的（NAT64 本地前缀
/// `64:ff9b:1::/48`、Teredo `2001::/32`）一律拒绝。
pub fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_disallowed_ipv4(ip),
        IpAddr::V6(ip) => is_disallowed_ipv6(ip),
    }
}

fn is_disallowed_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    a == 0
        || a >= 240
        || ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_multicast()
        // 192.0.0.0/24 IETF 协议分配；192.0.2/24、198.51.100/24、203.0.113/24 文档。
        || (a, b, c) == (192, 0, 0)
        || matches!((a, b, c), (192, 0, 2) | (198, 51, 100) | (203, 0, 113))
        // 100.64.0.0/10 运营商共享地址；198.18.0.0/15 基准测试。
        || (a == 100 && (64..=127).contains(&b))
        || (a == 198 && (b == 18 || b == 19))
}

fn is_disallowed_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    if ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
    {
        return true;
    }
    // IPv4 映射（::ffff:a.b.c.d）与兼容（::a.b.c.d）形式。
    if let Some(v4) = ip.to_ipv4() {
        return is_disallowed_ipv4(v4);
    }
    let embedded_low = || Ipv4Addr::from(((segments[6] as u32) << 16) | segments[7] as u32);
    match segments {
        // NAT64 知名前缀 64:ff9b::/96：按嵌入的 IPv4 判断。
        [0x64, 0xff9b, 0, 0, 0, 0, _, _] => is_disallowed_ipv4(embedded_low()),
        // NAT64 本地前缀 64:ff9b:1::/48：映射规则由本地网络决定，无法判断。
        [0x64, 0xff9b, 1, ..] => true,
        // 丢弃前缀 100::/64。
        [0x100, 0, 0, 0, ..] => true,
        // Teredo 2001::/32：嵌入地址经过混淆，不尝试解码。
        [0x2001, 0, ..] => true,
        // 基准测试 2001:2::/48。
        [0x2001, 2, 0, ..] => true,
        // 文档 2001:db8::/32 与 3fff::/20。
        [0x2001, 0xdb8, ..] => true,
        [0x3fff, second, ..] if second & 0xf000 == 0 => true,
        // 6to4 2002::/16：第 2、3 段是嵌入的 IPv4。
        [0x2002, high, low, ..] => {
            is_disallowed_ipv4(Ipv4Addr::from(((high as u32) << 16) | low as u32))
        }
        // 已废弃的站点本地地址 fec0::/10。
        [first, ..] if first & 0xffc0 == 0xfec0 => true,
        _ => false,
    }
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
        for ip in [
            // NAT64 local-use prefix, NAT64 well-known prefix embedding 10.0.0.1, 6to4 of 192.168.0.1,
            // Teredo, benchmark, documentation, discard-only, site-local, IETF protocol block.
            "64:ff9b:1::a00:1",
            "64:ff9b::a00:1",
            "2002:c0a8:1::1",
            "2001::1",
            "2001:2::1",
            "3fff::1",
            "100::1",
            "fec0::1",
            "192.0.0.8",
        ] {
            assert!(is_disallowed_ip(ip.parse::<IpAddr>().unwrap()), "{ip} must be disallowed");
        }
        for ip in [
            "1.1.1.1",
            "93.184.216.34",
            "2606:4700:4700::1111",
            // NAT64 of a public IPv4 and 6to4 of a public IPv4 stay reachable.
            "64:ff9b::101:101",
            "2002:101:101::1",
            // Just outside the 3fff::/20 documentation block.
            "3fff:1000::1",
        ] {
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
