use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, anyhow, bail, ensure};
use time::Duration;
use tracing::warn;
use url::Url;

const DEFAULT_LOGIN_RATE_LIMIT_WINDOW_MINUTES: i64 = 15;
const DEFAULT_LOGIN_RATE_LIMIT_BLOCK_MINUTES: i64 = 15;
const DEFAULT_LOGIN_RATE_LIMIT_MAX_FAILURES: u32 = 5;

/// 已不再使用的旧配置：设置了只记一条警告，旧部署升级时不用改。
const OBSOLETE_VARIABLES: [(&str, &str); 4] = [
    ("RSS_READER_WEB_SESSION_SECRET", "会话改为服务器保存的不透明令牌"),
    ("RSS_READER_WEB_SESSION_TTL_HOURS", "会话固定为 30 天未使用过期、180 天绝对上限"),
    (
        "RSS_READER_WEB_SECURE_COOKIE",
        "cookie 是否带 Secure 由 RSS_READER_WEB_PUBLIC_ORIGIN 的协议决定",
    ),
    ("RSS_READER_WEB_ENABLE_SMOKE_HELPERS", "冒烟辅助路由已删除"),
];

#[derive(Clone)]
pub(crate) struct AuthConfig {
    pub(crate) bind_addr: SocketAddr,
    pub(crate) static_dir: PathBuf,
    pub(crate) username: String,
    /// 用户访问本服务的来源（`RSS_READER_WEB_PUBLIC_ORIGIN`）；开发环境可以不设，
    /// 见 [`super::origin::TrustedOrigins`]。
    pub(crate) public_origin: Option<Url>,
    pub(crate) trust_proxy_headers: bool,
    pub(crate) login_rate_limit: LoginRateLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebEnvironment {
    Development,
    Production,
}

#[derive(Clone)]
pub(crate) struct LoginRateLimit {
    pub(crate) max_failures: u32,
    pub(crate) window: Duration,
    pub(crate) block_for: Duration,
}

pub(crate) fn load_config() -> Result<AuthConfig> {
    let bind_addr = env::var("RSS_READER_WEB_BIND")
        .unwrap_or_else(|_| "0.0.0.0:80".to_string())
        .parse()
        .context("解析 RSS_READER_WEB_BIND 失败")?;
    let static_dir = PathBuf::from(
        env::var("RSS_READER_WEB_STATIC_DIR").unwrap_or_else(|_| "/app/public".to_string()),
    );
    let username = required_env("RSS_READER_WEB_USERNAME")?;
    let environment = env::var("RSS_READER_WEB_ENV")
        .ok()
        .map(parse_environment)
        .transpose()?
        .unwrap_or(WebEnvironment::Development);
    let public_origin = optional_env("RSS_READER_WEB_PUBLIC_ORIGIN")
        .map(|raw| parse_public_origin(&raw))
        .transpose()?;
    let trust_proxy_headers = env::var("RSS_READER_WEB_TRUST_PROXY_HEADERS")
        .ok()
        .map(|raw| matches!(raw.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    let login_rate_limit = load_login_rate_limit()?;

    if environment == WebEnvironment::Production {
        let Some(origin) = &public_origin else {
            bail!("生产环境必须设置 RSS_READER_WEB_PUBLIC_ORIGIN（用户访问本服务的 https 地址）");
        };
        ensure!(
            origin.scheme() == "https",
            "生产环境的 RSS_READER_WEB_PUBLIC_ORIGIN 必须是 https 地址：会话 cookie 需要 Secure"
        );
    }
    for (name, reason) in OBSOLETE_VARIABLES {
        if env::var_os(name).is_some() {
            warn!("{name} 已不再使用：{reason}");
        }
    }

    Ok(AuthConfig {
        bind_addr,
        static_dir,
        username,
        public_origin,
        trust_proxy_headers,
        login_rate_limit,
    })
}

/// 公开来源只能是 `协议://主机[:端口]`（http 或 https），不带用户信息、路径、查询或片段：
/// 它与浏览器的 `Origin` 请求头逐字比较。
pub(crate) fn parse_public_origin(raw: &str) -> Result<Url> {
    let url = Url::parse(raw)
        .with_context(|| format!("RSS_READER_WEB_PUBLIC_ORIGIN 不是合法的地址：{raw}"))?;
    let origin_only = matches!(url.scheme(), "http" | "https")
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none();
    ensure!(
        origin_only,
        "RSS_READER_WEB_PUBLIC_ORIGIN 只能是 协议://主机[:端口]（http 或 https），例如 https://reader.example.com；当前为 {raw}"
    );
    Ok(url)
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).map_err(|_| anyhow!("缺少环境变量：{name}"))
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn parse_environment(raw: String) -> Result<WebEnvironment> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "dev" | "development" | "local" => Ok(WebEnvironment::Development),
        "prod" | "production" => Ok(WebEnvironment::Production),
        _ => Err(anyhow!("RSS_READER_WEB_ENV 只能是 development 或 production")),
    }
}

fn load_login_rate_limit() -> Result<LoginRateLimit> {
    let max_failures = env::var("RSS_READER_WEB_LOGIN_MAX_FAILURES")
        .ok()
        .map(|raw| raw.parse::<u32>())
        .transpose()
        .context("解析 RSS_READER_WEB_LOGIN_MAX_FAILURES 失败")?
        .unwrap_or(DEFAULT_LOGIN_RATE_LIMIT_MAX_FAILURES);
    ensure!(max_failures > 0, "RSS_READER_WEB_LOGIN_MAX_FAILURES 必须大于 0");

    let window_minutes = env::var("RSS_READER_WEB_LOGIN_WINDOW_MINUTES")
        .ok()
        .map(|raw| raw.parse::<i64>())
        .transpose()
        .context("解析 RSS_READER_WEB_LOGIN_WINDOW_MINUTES 失败")?
        .unwrap_or(DEFAULT_LOGIN_RATE_LIMIT_WINDOW_MINUTES);
    ensure!(window_minutes > 0, "RSS_READER_WEB_LOGIN_WINDOW_MINUTES 必须大于 0");

    let block_minutes = env::var("RSS_READER_WEB_LOGIN_BLOCK_MINUTES")
        .ok()
        .map(|raw| raw.parse::<i64>())
        .transpose()
        .context("解析 RSS_READER_WEB_LOGIN_BLOCK_MINUTES 失败")?
        .unwrap_or(DEFAULT_LOGIN_RATE_LIMIT_BLOCK_MINUTES);
    ensure!(block_minutes > 0, "RSS_READER_WEB_LOGIN_BLOCK_MINUTES 必须大于 0");

    Ok(LoginRateLimit {
        max_failures,
        window: Duration::minutes(window_minutes),
        block_for: Duration::minutes(block_minutes),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_public_origin;

    #[test]
    fn a_public_origin_is_scheme_host_and_port_only() {
        for (raw, origin) in [
            ("https://reader.example.com", "https://reader.example.com"),
            ("https://reader.example.com/", "https://reader.example.com"),
            ("https://reader.example.com:443", "https://reader.example.com"),
            ("http://localhost:8039", "http://localhost:8039"),
            ("https://阅读.example", "https://xn--hf3ar1r.example"),
        ] {
            let url = parse_public_origin(raw).unwrap();
            assert_eq!(url.origin().ascii_serialization(), origin, "{raw}");
        }
        for raw in [
            "reader.example.com",
            "ftp://reader.example.com",
            "https://user:pass@reader.example.com",
            "https://reader.example.com/app",
            "https://reader.example.com/?x=1",
            "https://reader.example.com/#top",
            "null",
        ] {
            assert!(parse_public_origin(raw).is_err(), "{raw}");
        }
    }
}
