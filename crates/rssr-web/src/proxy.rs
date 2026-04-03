use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use axum::{
    body::Body,
    extract::Query,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use reqwest::Url;
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct FeedProxyQuery {
    pub(crate) url: String,
}

pub(crate) async fn feed_proxy(Query(query): Query<FeedProxyQuery>) -> impl IntoResponse {
    let upstream_url = match validate_proxy_target(&query.url).await {
        Ok(url) => url,
        Err(err) => return (StatusCode::BAD_REQUEST, err).into_response(),
    };

    let response = match fetch_proxied_feed(upstream_url).await {
        Ok(response) => response,
        Err(err) => return (StatusCode::BAD_GATEWAY, err).into_response(),
    };

    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let etag = response.headers().get(header::ETAG).cloned();
    let last_modified = response.headers().get(header::LAST_MODIFIED).cloned();
    let body = match response.bytes().await {
        Ok(body) => body,
        Err(err) => {
            return (StatusCode::BAD_GATEWAY, format!("读取 feed 代理响应失败：{err}"))
                .into_response();
        }
    };

    let mut proxied = Response::builder().status(status);
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

fn validate_proxy_host(url: &Url) -> Result<(String, u16), String> {
    let host = url.host_str().ok_or_else(|| "feed URL 缺少主机名。".to_string())?;

    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return Err("出于安全原因，禁止代理 localhost 地址。".to_string());
    }

    let port =
        url.port_or_known_default().ok_or_else(|| "无法确定 feed URL 的端口。".to_string())?;

    if let Ok(ip) = host.parse::<IpAddr>() {
        return if is_disallowed_proxy_ip(ip) {
            Err("出于安全原因，禁止代理内网或本地地址。".to_string())
        } else {
            Ok((host.to_string(), port))
        };
    }

    if host.ends_with(".local") {
        return Err("出于安全原因，禁止代理 .local 内网域名。".to_string());
    }

    Ok((host.to_string(), port))
}

async fn validate_proxy_target(raw: &str) -> Result<Url, String> {
    let url = parse_proxy_feed_url(raw)?;
    let (host, port) = validate_proxy_host(&url)?;
    let resolved = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|_| "无法解析 feed 主机名。".to_string())?;

    if resolved.map(|addr| addr.ip()).any(is_disallowed_proxy_ip) {
        return Err("出于安全原因，禁止代理内网或本地地址。".to_string());
    }

    Ok(url)
}

fn is_disallowed_proxy_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
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
                || ip == Ipv6Addr::LOCALHOST
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

async fn fetch_proxied_feed(initial_url: Url) -> Result<reqwest::Response, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| format!("初始化 feed 代理客户端失败：{err}"))?;
    let mut current_url = initial_url;

    for _ in 0..5 {
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
        current_url = validate_proxy_target(redirected.as_str()).await?;
    }

    Err("feed 代理重定向次数过多。".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
