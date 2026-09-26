use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use axum::{
    body::Body,
    extract::Query,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use reqwest::Url;
use serde::Deserialize;
use url::Host;

/// 代理是登录后才可达的，但仍然不能无上限地把上游响应整个读进内存：
/// 一个 feed URL 指向超大文件就足以把服务打爆。
const MAX_PROXIED_FEED_BYTES: usize = 8 * 1024 * 1024;
const PROXY_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const PROXY_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
pub(crate) struct FeedProxyQuery {
    pub(crate) url: String,
}

pub(crate) async fn feed_proxy(Query(query): Query<FeedProxyQuery>) -> impl IntoResponse {
    let (upstream_url, upstream_addr) = match resolve_validated_target(&query.url).await {
        Ok(target) => target,
        Err(err) => return (StatusCode::BAD_REQUEST, err).into_response(),
    };

    let response = match fetch_proxied_feed(upstream_url, upstream_addr).await {
        Ok(response) => response,
        Err(err) => return (StatusCode::BAD_GATEWAY, err).into_response(),
    };

    let final_url = response.url().to_string();
    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let etag = response.headers().get(header::ETAG).cloned();
    let last_modified = response.headers().get(header::LAST_MODIFIED).cloned();
    let body = match read_body_with_limit(response, MAX_PROXIED_FEED_BYTES).await {
        Ok(body) => body,
        Err(err) => return (StatusCode::BAD_GATEWAY, err).into_response(),
    };

    let mut proxied = Response::builder().status(status).header("x-rssr-final-url", final_url);
    if let Some(value) = content_type {
        proxied = proxied.header(header::CONTENT_TYPE, value);
    }
    if let Some(value) = etag {
        proxied = proxied.header(header::ETAG, value);
    }
    if let Some(value) = last_modified {
        proxied = proxied.header(header::LAST_MODIFIED, value);
    }

    proxied.body(Body::from(body)).expect("valid proxied feed response")
}

fn parse_proxy_feed_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|_| "feed URL 不合法。".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("只允许代理 http/https feed URL。".to_string());
    }
    Ok(url)
}

enum ValidatedHost {
    Domain { name: String, port: u16 },
    Address(SocketAddr),
}

fn validate_proxy_host(url: &Url) -> Result<ValidatedHost, String> {
    let port =
        url.port_or_known_default().ok_or_else(|| "无法确定 feed URL 的端口。".to_string())?;
    match url.host().ok_or_else(|| "feed URL 缺少主机名。".to_string())? {
        Host::Ipv4(ip) => validate_proxy_ip(IpAddr::V4(ip), port),
        Host::Ipv6(ip) => validate_proxy_ip(IpAddr::V6(ip), port),
        Host::Domain(name) => {
            if name.eq_ignore_ascii_case("localhost") || name.ends_with(".localhost") {
                return Err("出于安全原因，禁止代理 localhost 地址。".to_string());
            }
            if name.ends_with(".local") {
                return Err("出于安全原因，禁止代理 .local 内网域名。".to_string());
            }
            Ok(ValidatedHost::Domain { name: name.to_string(), port })
        }
    }
}

fn validate_proxy_ip(ip: IpAddr, port: u16) -> Result<ValidatedHost, String> {
    if is_disallowed_proxy_ip(ip) {
        Err("出于安全原因，禁止代理内网或本地地址。".to_string())
    } else {
        Ok(ValidatedHost::Address(SocketAddr::new(ip, port)))
    }
}

/// 解析主机名并校验解析结果，返回校验通过的地址本身。
///
/// 必须把地址一起返回：如果只校验、之后再让 HTTP 客户端按域名重新解析一次，攻击者控制的
/// DNS 可以在两次解析之间换成 127.0.0.1 之类的内网地址（DNS rebinding）。调用方要把这个
/// 地址钉死给客户端用，保证「校验的」和「实际连接的」是同一个 IP。
async fn resolve_validated_target(raw: &str) -> Result<(Url, SocketAddr), String> {
    let url = parse_proxy_feed_url(raw)?;
    let (host, port) = match validate_proxy_host(&url)? {
        ValidatedHost::Address(addr) => return Ok((url, addr)),
        ValidatedHost::Domain { name, port } => (name, port),
    };
    let resolved =
        tokio::time::timeout(PROXY_CONNECT_TIMEOUT, tokio::net::lookup_host((host.as_str(), port)))
            .await
            .map_err(|_| "解析 feed 主机名超时。".to_string())?
            .map_err(|_| "无法解析 feed 主机名。".to_string())?
            .collect::<Vec<_>>();

    let Some(first) = resolved.first().copied() else {
        return Err("无法解析 feed 主机名。".to_string());
    };
    // 只要有任意一条解析结果落在内网就整体拒绝，不去挑一个“看起来安全”的。
    if resolved.iter().any(|addr| is_disallowed_proxy_ip(addr.ip())) {
        return Err("出于安全原因，禁止代理内网或本地地址。".to_string());
    }

    Ok((url, first))
}

fn is_disallowed_proxy_ip(ip: IpAddr) -> bool {
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
                || ip == Ipv4Addr::new(169, 254, 169, 254)
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || is_documentation_ipv6(ip)
                || ip.to_ipv4().is_some_and(|v4| is_disallowed_proxy_ip(IpAddr::V4(v4)))
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

/// 边下边累计字节数，一超过上限立刻放弃。先看 `Content-Length` 只能挡住诚实的上游，
/// 真正的保护来自流式累计。
async fn read_body_with_limit(
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    if let Some(content_length) = response.content_length()
        && content_length > max_bytes as u64
    {
        return Err(format!("feed 响应体积超过 {max_bytes} 字节上限。"));
    }

    let mut buffered = Vec::new();
    while let Some(chunk) =
        response.chunk().await.map_err(|err| format!("读取 feed 代理响应失败：{err}"))?
    {
        if buffered.len() + chunk.len() > max_bytes {
            return Err(format!("feed 响应体积超过 {max_bytes} 字节上限。"));
        }
        buffered.extend_from_slice(&chunk);
    }

    Ok(buffered)
}

/// 每一跳都用「校验时得到的地址」新建一个客户端并把域名钉到该地址上。
///
/// 仍然按域名发请求（而不是直接拿 IP 拼 URL），这样 Host 头与 TLS SNI 保持正确；
/// `resolve` 只是替换掉这一次的 DNS 解析结果。重定向同理：每跳都要重新校验并重新钉。
fn pinned_client(url: &Url, addr: SocketAddr) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        // An environment proxy can resolve the host elsewhere, bypassing our pinned IP.
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(PROXY_REQUEST_TIMEOUT)
        .connect_timeout(PROXY_CONNECT_TIMEOUT);
    if let Some(Host::Domain(host)) = url.host() {
        builder = builder.resolve(host, addr);
    }
    builder.build().map_err(|err| format!("初始化 feed 代理客户端失败：{err}"))
}

async fn fetch_proxied_feed(
    initial_url: Url,
    initial_addr: SocketAddr,
) -> Result<reqwest::Response, String> {
    let mut current_url = initial_url;
    let mut current_addr = initial_addr;

    for _ in 0..5 {
        let client = pinned_client(&current_url, current_addr)?;
        let response = client
            .get(current_url.clone())
            .header(
                header::ACCEPT,
                "application/atom+xml, application/rss+xml, application/xml, text/xml;q=0.9, */*;q=0.1",
            )
            .send()
            .await
            .map_err(|err| format!("feed 代理请求失败：{err}"))?;

        if !response.status().is_redirection() {
            return Ok(response);
        }

        let Some(location) = response.headers().get(header::LOCATION) else {
            return Err("feed 代理收到重定向，但响应缺少 Location 头。".to_string());
        };
        let location =
            location.to_str().map_err(|_| "feed 代理收到无法解析的重定向地址。".to_string())?;
        let redirected = current_url
            .join(location)
            .map_err(|_| "feed 代理收到非法的重定向地址。".to_string())?;
        let (next_url, next_addr) = resolve_validated_target(redirected.as_str()).await?;
        current_url = next_url;
        current_addr = next_addr;
    }

    Err("feed 代理重定向次数过多。".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pinned_client_ignores_environment_proxy() {
        const CHILD: &str = "RSSR_TEST_PINNED_PROXY_CHILD";
        if std::env::var_os(CHILD).is_none() {
            // Isolate environment changes from parallel tests and reqwest's proxy cache.
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child
                .args([
                    "--exact",
                    "proxy::tests::pinned_client_ignores_environment_proxy",
                    "--nocapture",
                ])
                .env(CHILD, "1");
            for key in
                ["HTTP_PROXY", "http_proxy", "HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"]
            {
                child.env(key, "http://127.0.0.1:0");
            }
            child.env("NO_PROXY", "").env("no_proxy", "");
            let output = child.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                axum::Router::new()
                    .route("/feed", axum::routing::get(|| async { "pinned fixture" })),
            )
            .await
            .unwrap();
        });
        let url = Url::parse(&format!("http://feed.example.invalid:{}/feed", addr.port())).unwrap();
        let result = pinned_client(&url, addr).unwrap().get(url).send().await;
        server.abort();
        assert_eq!(
            result.expect("must reach the pinned address directly").status(),
            StatusCode::OK
        );
    }

    #[test]
    fn parse_proxy_feed_url_only_allows_http_and_https() {
        assert!(parse_proxy_feed_url("https://example.com/feed.xml").is_ok());
        assert!(parse_proxy_feed_url("http://example.com/feed.xml").is_ok());
        assert!(parse_proxy_feed_url("file:///tmp/feed.xml").is_err());
        assert!(parse_proxy_feed_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn proxy_validation_rejects_local_targets() {
        assert!(validate_proxy_host(&Url::parse("http://127.0.0.1/feed.xml").unwrap()).is_err());
        assert!(validate_proxy_host(&Url::parse("http://localhost/feed.xml").unwrap()).is_err());
        assert!(
            validate_proxy_host(&Url::parse("http://169.254.169.254/latest/meta-data").unwrap())
                .is_err()
        );
        assert!(validate_proxy_host(&Url::parse("http://0.0.0.1/feed.xml").unwrap()).is_err());
        for target in [
            "http://[::1]/feed.xml",
            "http://[::ffff:0.0.0.1]/feed.xml",
            "http://[::ffff:127.0.0.1]/feed.xml",
            "http://[::ffff:169.254.169.254]/feed.xml",
            "http://[::ffff:192.168.1.2]/feed.xml",
            "http://[fc00::1]/feed.xml",
        ] {
            assert!(validate_proxy_host(&Url::parse(target).unwrap()).is_err(), "{target}");
        }
    }

    #[tokio::test]
    async fn public_literal_addresses_are_pinned_without_dns_lookup() {
        for (raw, expected) in [
            ("http://8.8.8.8/feed.xml", "8.8.8.8:80"),
            ("https://[2606:4700::1111]/feed.xml", "[2606:4700::1111]:443"),
        ] {
            let (_, addr) = resolve_validated_target(raw).await.expect("public literal");
            assert_eq!(addr, expected.parse::<SocketAddr>().unwrap());
        }
    }
}
