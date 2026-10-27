//! 请求来源：写请求的 `Origin` 必须与信任的来源逐字一致（v0.2 阶段 C，C2）。
//!
//! - 设置了 `RSS_READER_WEB_PUBLIC_ORIGIN` 时只信任它；
//! - 开发环境可以不设，此时服务器只能监听回环地址，只信任与实际监听地址完全一致的来源
//!   （例如 `http://127.0.0.1:<端口>` 与 `http://localhost:<端口>`）。同一主机的其他端口或页面
//!   不因“是本机”而被信任。监听地址取实际绑定的套接字，端口为 0 时也是真实端口。
//!
//! 缺少 `Origin`、为 `null` 或不一致的写请求一律拒绝。读请求（GET、HEAD、OPTIONS）不检查：
//! 它们没有副作用，响应也不带任何跨源头，别的来源读不到。

use std::net::{IpAddr, SocketAddr};

use anyhow::{Result, ensure};
use axum::http::Method;
use url::Url;

pub(crate) struct TrustedOrigins {
    origins: Vec<String>,
    secure: bool,
}

impl TrustedOrigins {
    /// `public` 是配置的公开来源，`local` 是实际监听的地址。
    pub(crate) fn new(public: Option<&Url>, local: SocketAddr) -> Result<Self> {
        if let Some(public) = public {
            return Ok(Self {
                origins: vec![public.origin().ascii_serialization()],
                secure: public.scheme() == "https",
            });
        }
        ensure!(
            local.ip().is_loopback(),
            "没有设置 RSS_READER_WEB_PUBLIC_ORIGIN 时只能监听回环地址（当前为 {local}）：请设置用户访问本服务的地址"
        );
        let host = match local.ip() {
            IpAddr::V4(ip) => ip.to_string(),
            IpAddr::V6(ip) => format!("[{ip}]"),
        };
        let port = local.port();
        let origins = [format!("http://{host}:{port}"), format!("http://localhost:{port}")]
            .iter()
            .map(|raw| Url::parse(raw).map(|url| url.origin().ascii_serialization()))
            .collect::<Result<_, _>>()?;
        Ok(Self { origins, secure: false })
    }

    pub(crate) fn trusts(&self, origin: &[u8]) -> bool {
        self.origins.iter().any(|trusted| trusted.as_bytes() == origin)
    }

    /// 会话 cookie 是否带 `Secure`：信任的来源是 https 时带。
    pub(crate) fn secure(&self) -> bool {
        self.secure
    }
}

/// 没有副作用、不需要核对来源的方法。
pub(crate) fn is_safe(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trusted(public: Option<&str>, local: &str) -> Result<TrustedOrigins> {
        let public = public.map(|raw| Url::parse(raw).unwrap());
        TrustedOrigins::new(public.as_ref(), local.parse().unwrap())
    }

    #[test]
    fn a_configured_origin_is_the_only_one_trusted() {
        let origins = trusted(Some("https://reader.example.com"), "0.0.0.0:8080").unwrap();
        assert!(origins.trusts(b"https://reader.example.com"));
        assert!(origins.secure());
        for other in [
            &b"http://reader.example.com"[..],
            b"https://reader.example.com:8443",
            b"https://evil.example",
            b"http://127.0.0.1:8080",
            b"null",
            b"",
        ] {
            assert!(!origins.trusts(other), "{}", String::from_utf8_lossy(other));
        }
    }

    #[test]
    fn without_a_configured_origin_only_the_bound_loopback_address_is_trusted() {
        let origins = trusted(None, "127.0.0.1:18080").unwrap();
        assert!(origins.trusts(b"http://127.0.0.1:18080"));
        assert!(origins.trusts(b"http://localhost:18080"));
        assert!(!origins.secure());
        for other in [
            &b"http://127.0.0.1:18081"[..],
            b"http://localhost:3000",
            b"https://127.0.0.1:18080",
            b"http://[::1]:18080",
            b"null",
        ] {
            assert!(!origins.trusts(other), "{}", String::from_utf8_lossy(other));
        }
        let v6 = trusted(None, "[::1]:8039").unwrap();
        assert!(v6.trusts(b"http://[::1]:8039") && v6.trusts(b"http://localhost:8039"));
        let default_port = trusted(None, "127.0.0.1:80").unwrap();
        assert!(default_port.trusts(b"http://127.0.0.1"), "browsers omit the default port");

        assert!(trusted(None, "0.0.0.0:8080").is_err());
        assert!(trusted(None, "192.0.2.10:8080").is_err());
    }
}
