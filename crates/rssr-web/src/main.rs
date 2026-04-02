use std::{
    collections::HashMap,
    env,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context, Result, anyhow, ensure};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::{
    Form, Router,
    extract::{Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::Parser;
use hmac::{Hmac, Mac};
use rand_core::OsRng;
use serde::Deserialize;
use sha2::Sha256;
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

const DEFAULT_LOGIN_RATE_LIMIT_WINDOW_MINUTES: i64 = 15;
const DEFAULT_LOGIN_RATE_LIMIT_BLOCK_MINUTES: i64 = 15;
const DEFAULT_LOGIN_RATE_LIMIT_MAX_FAILURES: u32 = 5;

type HmacSha256 = Hmac<Sha256>;

const SESSION_COOKIE: &str = "rssr_web_session";
const GATE_COOKIE: &str = "rssr_web_gate";
const APP_NAME: &str = "RSS-Reader";
const WEB_LOGIN_MARKUP: &str = include_str!("../../../assets/branding/rssr-mark.svg");

#[derive(Parser, Debug)]
struct Cli {
    #[arg(long, value_name = "PASSWORD")]
    print_password_hash: Option<String>,
}

#[derive(Clone)]
struct AppState {
    config: Arc<AuthConfig>,
    login_throttle: Arc<Mutex<HashMap<String, LoginThrottleState>>>,
}

#[derive(Clone)]
struct AuthConfig {
    bind_addr: SocketAddr,
    static_dir: PathBuf,
    username: String,
    password_policy: PasswordPolicy,
    session_secret: Vec<u8>,
    secure_cookie: bool,
    session_ttl: Duration,
    login_rate_limit: LoginRateLimit,
}

#[derive(Clone)]
enum PasswordPolicy {
    Argon2Hash(String),
    PlaintextDev(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebEnvironment {
    Development,
    Production,
}

#[derive(Clone)]
struct LoginRateLimit {
    max_failures: u32,
    window: Duration,
    block_for: Duration,
}

#[derive(Clone, Debug)]
struct LoginThrottleState {
    failures: u32,
    window_started_at: OffsetDateTime,
    blocked_until: Option<OffsetDateTime>,
}

#[derive(Deserialize, Default)]
struct LoginQuery {
    next: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct FeedProxyQuery {
    url: String,
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
    next: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    if let Some(password) = cli.print_password_hash {
        println!("{}", generate_password_hash(&password)?);
        return Ok(());
    }

    let config = Arc::new(load_config()?);
    let app_state =
        AppState { config: config.clone(), login_throttle: Arc::new(Mutex::new(HashMap::new())) };
    ensure!(
        config.static_dir.join("index.html").exists(),
        "静态资源目录缺少 index.html：{}",
        config.static_dir.display()
    );

    let protected = Router::new()
        .route("/session-probe", get(session_probe))
        .route("/feed-proxy", get(feed_proxy))
        .fallback_service(
            ServeDir::new(config.static_dir.clone())
                .not_found_service(ServeFile::new(config.static_dir.join("index.html"))),
        )
        .layer(middleware::from_fn_with_state(app_state.clone(), require_auth));

    let app = Router::new()
        .route("/login", get(show_login).post(handle_login))
        .route("/logout", get(handle_logout))
        .route("/healthz", get(healthz))
        .merge(protected)
        .with_state(app_state);

    info!("starting rssr-web on {}", config.bind_addr);
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("绑定监听地址失败：{}", config.bind_addr))?;

    axum::serve(listener, app).await.context("启动 Web 服务失败")?;

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}

fn load_config() -> Result<AuthConfig> {
    let bind_addr = env::var("RSS_READER_WEB_BIND")
        .unwrap_or_else(|_| "0.0.0.0:80".to_string())
        .parse()
        .context("解析 RSS_READER_WEB_BIND 失败")?;
    let static_dir = PathBuf::from(
        env::var("RSS_READER_WEB_STATIC_DIR").unwrap_or_else(|_| "/app/public".to_string()),
    );
    let username = required_env("RSS_READER_WEB_USERNAME")?;
    let password_policy = load_password_policy()?;
    let session_secret = required_env("RSS_READER_WEB_SESSION_SECRET")?;
    ensure!(session_secret.len() >= 32, "RSS_READER_WEB_SESSION_SECRET 至少需要 32 个字符");
    let environment = env::var("RSS_READER_WEB_ENV")
        .ok()
        .map(parse_environment)
        .transpose()?
        .unwrap_or(WebEnvironment::Development);
    let secure_cookie = env::var("RSS_READER_WEB_SECURE_COOKIE")
        .ok()
        .map(|raw| matches!(raw.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    let session_ttl_hours = env::var("RSS_READER_WEB_SESSION_TTL_HOURS")
        .ok()
        .map(|raw| raw.parse::<i64>())
        .transpose()
        .context("解析 RSS_READER_WEB_SESSION_TTL_HOURS 失败")?
        .unwrap_or(12);
    ensure!(session_ttl_hours > 0, "会话时长必须大于 0");
    let login_rate_limit = load_login_rate_limit()?;

    if environment == WebEnvironment::Production {
        ensure!(secure_cookie, "生产环境必须开启 RSS_READER_WEB_SECURE_COOKIE=true。");
        ensure!(
            matches!(password_policy, PasswordPolicy::Argon2Hash(_)),
            "生产环境必须使用 RSS_READER_WEB_PASSWORD_HASH，禁止继续使用明文 RSS_READER_WEB_PASSWORD。"
        );
    } else if matches!(password_policy, PasswordPolicy::PlaintextDev(_)) {
        warn!("rssr-web 正在使用明文 RSS_READER_WEB_PASSWORD，仅建议用于本地开发。");
    }

    Ok(AuthConfig {
        bind_addr,
        static_dir,
        username,
        password_policy,
        session_secret: session_secret.into_bytes(),
        secure_cookie,
        session_ttl: Duration::hours(session_ttl_hours),
        login_rate_limit,
    })
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).map_err(|_| anyhow!("缺少环境变量：{name}"))
}

fn parse_proxy_feed_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "feed URL 不合法。".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("只允许代理 http/https feed URL。".to_string());
    }
    Ok(url)
}

fn parse_environment(raw: String) -> Result<WebEnvironment> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "dev" | "development" | "local" => Ok(WebEnvironment::Development),
        "prod" | "production" => Ok(WebEnvironment::Production),
        _ => Err(anyhow!("RSS_READER_WEB_ENV 只能是 development 或 production")),
    }
}

fn load_password_policy() -> Result<PasswordPolicy> {
    let password_hash = env::var("RSS_READER_WEB_PASSWORD_HASH").ok();
    let password = env::var("RSS_READER_WEB_PASSWORD").ok();

    if let Some(hash) = password_hash {
        validate_password_hash(&hash)?;
        return Ok(PasswordPolicy::Argon2Hash(hash));
    }

    if let Some(password) = password {
        ensure!(password.len() >= 8, "RSS_READER_WEB_PASSWORD 至少需要 8 个字符");
        return Ok(PasswordPolicy::PlaintextDev(password));
    }

    Err(anyhow!(
        "缺少环境变量：RSS_READER_WEB_PASSWORD_HASH（推荐）或 RSS_READER_WEB_PASSWORD（仅开发环境）"
    ))
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

fn validate_password_hash(raw: &str) -> Result<()> {
    PasswordHash::new(raw)
        .map_err(|_| anyhow!("RSS_READER_WEB_PASSWORD_HASH 不是合法的 Argon2 哈希"))?;
    Ok(())
}

fn generate_password_hash(password: &str) -> Result<String> {
    ensure!(password.len() >= 8, "密码至少需要 8 个字符");
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| anyhow!("生成密码哈希失败"))?;
    Ok(hash.to_string())
}

fn validate_proxy_host(url: &reqwest::Url) -> Result<(String, u16), String> {
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

async fn validate_proxy_target(raw: &str) -> Result<reqwest::Url, String> {
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

async fn show_login(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
) -> impl IntoResponse {
    let next = sanitize_next(query.next.as_deref());
    let error_message = match query.error.as_deref() {
        Some("invalid_credentials") => "用户名或密码错误。",
        Some("session_expired") => "登录已过期，请重新登录。",
        Some("rate_limited") => "登录尝试过于频繁，请稍后再试。",
        _ => "",
    };

    let secure_note = if state.config.secure_cookie {
        "当前会话 cookie 已启用 Secure。"
    } else {
        "当前会话 cookie 未启用 Secure；生产环境请通过 HTTPS 反向代理并开启 RSS_READER_WEB_SECURE_COOKIE=true。"
    };

    Html(format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{} 登录</title>
  <style>
    :root {{
      color-scheme: light;
      --bg: #f5f1ea;
      --panel: rgba(255,255,255,0.92);
      --line: rgba(77, 55, 35, 0.12);
      --ink: #231b14;
      --muted: #6b6258;
      --accent: #6d4c35;
      --accent-strong: #533827;
      --danger: #9b3d2e;
      --shadow: 0 20px 48px rgba(32, 24, 18, 0.08);
      font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      background:
        radial-gradient(circle at top, rgba(255,255,255,0.7), transparent 42%),
        linear-gradient(180deg, #efe4d5, var(--bg));
      color: var(--ink);
    }}
    .login-shell {{
      width: min(420px, calc(100vw - 32px));
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 20px;
      box-shadow: var(--shadow);
      padding: 28px;
    }}
    .brand {{
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 14px;
    }}
    .brand-mark {{
      width: 56px;
      height: 56px;
      flex: 0 0 auto;
    }}
    .brand-mark svg {{
      display: block;
      width: 100%;
      height: 100%;
    }}
    .brand-name {{
      margin: 0;
      font-size: 1.15rem;
      font-weight: 800;
      letter-spacing: 0.02em;
    }}
    h1 {{ margin: 0 0 8px; font-size: 1.8rem; }}
    p {{ margin: 0 0 16px; color: var(--muted); line-height: 1.6; }}
    form {{ display: grid; gap: 14px; margin-top: 18px; }}
    label {{ display: grid; gap: 6px; font-weight: 600; }}
    input {{
      width: 100%;
      border: 1px solid var(--line);
      border-radius: 12px;
      padding: 12px 14px;
      font: inherit;
      background: rgba(255,255,255,0.95);
    }}
    input:focus {{
      outline: none;
      border-color: rgba(109, 76, 53, 0.45);
      box-shadow: 0 0 0 3px rgba(109, 76, 53, 0.12);
    }}
    button {{
      margin-top: 6px;
      border: none;
      border-radius: 12px;
      padding: 12px 16px;
      font: inherit;
      font-weight: 700;
      background: var(--accent);
      color: white;
      cursor: pointer;
    }}
    button:hover {{ background: var(--accent-strong); }}
    .error {{ min-height: 22px; color: var(--danger); font-weight: 600; }}
    .note {{ margin-top: 14px; font-size: 0.92rem; }}
  </style>
</head>
<body>
  <main class="login-shell">
    <div class="brand">
      <div class="brand-mark">{}</div>
      <p class="brand-name">{}</p>
    </div>
    <h1>登录 {}</h1>
    <p>这个 Web 部署启用了登录保护。输入部署者提供的用户名和密码后，才能进入阅读器。</p>
    <div class="error">{}</div>
    <form method="post" action="/login" autocomplete="on">
      <input type="hidden" name="next" value="{}">
      <label for="login-username">用户名
        <input id="login-username" name="username" type="text" autocomplete="username" required>
      </label>
      <label for="login-password">密码
        <input id="login-password" name="password" type="password" autocomplete="current-password" required>
      </label>
      <button type="submit">登录</button>
    </form>
    <p class="note">{}</p>
  </main>
</body>
</html>"#,
        APP_NAME,
        WEB_LOGIN_MARKUP,
        APP_NAME,
        APP_NAME,
        error_message,
        html_escape(next),
        secure_note
    ))
}

async fn handle_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let next = sanitize_next(form.next.as_deref());
    let rate_limit_key = rate_limit_key(&headers, &form.username);

    if login_attempt_is_blocked(&state, &rate_limit_key).await {
        return Redirect::to(&format!(
            "/login?error=rate_limited&next={}",
            urlencoding::encode(next)
        ))
        .into_response();
    }

    if !credentials_match(&state.config, &form.username, &form.password) {
        let blocked = record_login_failure(&state, &rate_limit_key).await;
        return Redirect::to(&format!(
            "/login?error={}&next={}",
            if blocked { "rate_limited" } else { "invalid_credentials" },
            urlencoding::encode(next)
        ))
        .into_response();
    }

    clear_login_failures(&state, &rate_limit_key).await;

    let expires_at = OffsetDateTime::now_utc() + state.config.session_ttl;
    let token = match build_session_token(&state.config, expires_at.unix_timestamp()) {
        Ok(token) => token,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("登录失败：{}", html_escape(err.to_string()))),
            )
                .into_response();
        }
    };

    let mut response = Redirect::to(next).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie_header(&token, &state.config))
            .expect("valid session cookie"),
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&gate_cookie_header(&state.config)).expect("valid gate cookie"),
    );
    response
}

async fn handle_logout(State(state): State<AppState>) -> impl IntoResponse {
    let mut response = Redirect::to("/login").into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&logout_cookie_header(&state.config)).expect("valid logout cookie"),
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&logout_gate_cookie_header(&state.config))
            .expect("valid logout gate cookie"),
    );
    response
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn session_probe() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

async fn feed_proxy(Query(query): Query<FeedProxyQuery>) -> impl IntoResponse {
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

    proxied.body(axum::body::Body::from(body)).expect("valid proxied feed response")
}

async fn fetch_proxied_feed(initial_url: reqwest::Url) -> Result<reqwest::Response, String> {
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

async fn require_auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if let Some(cookie_value) = extract_cookie(request.headers(), SESSION_COOKIE) {
        if session_is_valid(&state.config, cookie_value) {
            return next.run(request).await;
        }

        let target = sanitize_next(Some(
            request.uri().path_and_query().map(|value| value.as_str()).unwrap_or("/"),
        ));
        return Redirect::to(&format!(
            "/login?error=session_expired&next={}",
            urlencoding::encode(target)
        ))
        .into_response();
    }

    let target = sanitize_next(Some(
        request.uri().path_and_query().map(|value| value.as_str()).unwrap_or("/"),
    ));
    Redirect::to(&format!("/login?next={}", urlencoding::encode(target))).into_response()
}

fn credentials_match(config: &AuthConfig, username: &str, password: &str) -> bool {
    if username.trim() != config.username {
        return false;
    }

    match &config.password_policy {
        PasswordPolicy::Argon2Hash(hash) => verify_password_hash(hash, password),
        PasswordPolicy::PlaintextDev(expected) => expected == password,
    }
}

fn verify_password_hash(hash: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

fn build_session_token(config: &AuthConfig, expires_at: i64) -> Result<String> {
    let payload = format!("{}:{expires_at}", config.username);
    let mut mac =
        HmacSha256::new_from_slice(&config.session_secret).context("初始化会话签名器失败")?;
    mac.update(payload.as_bytes());
    let signature = mac.finalize().into_bytes();
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload.as_bytes()),
        URL_SAFE_NO_PAD.encode(signature)
    ))
}

fn session_is_valid(config: &AuthConfig, cookie_value: &str) -> bool {
    let Some((payload_part, signature_part)) = cookie_value.split_once('.') else {
        return false;
    };
    let Ok(payload_bytes) = URL_SAFE_NO_PAD.decode(payload_part) else {
        return false;
    };
    let Ok(signature_bytes) = URL_SAFE_NO_PAD.decode(signature_part) else {
        return false;
    };
    let Ok(payload) = String::from_utf8(payload_bytes) else {
        return false;
    };
    let Some((username, expires_raw)) = payload.rsplit_once(':') else {
        return false;
    };
    if username != config.username {
        return false;
    }
    let Ok(expires_at) = expires_raw.parse::<i64>() else {
        return false;
    };
    if OffsetDateTime::now_utc().unix_timestamp() > expires_at {
        return false;
    }

    let Ok(mut mac) = HmacSha256::new_from_slice(&config.session_secret) else {
        return false;
    };
    mac.update(payload.as_bytes());
    mac.verify_slice(&signature_bytes).is_ok()
}

fn session_cookie_header(token: &str, config: &AuthConfig) -> String {
    let max_age = config.session_ttl.whole_seconds();
    let secure = if config.secure_cookie { "; Secure" } else { "" };
    format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}")
}

fn logout_cookie_header(config: &AuthConfig) -> String {
    let secure = if config.secure_cookie { "; Secure" } else { "" };
    format!("{SESSION_COOKIE}=deleted; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}")
}

fn gate_cookie_header(config: &AuthConfig) -> String {
    let max_age = config.session_ttl.whole_seconds();
    let secure = if config.secure_cookie { "; Secure" } else { "" };
    format!("{GATE_COOKIE}=1; Path=/; SameSite=Lax; Max-Age={max_age}{secure}")
}

fn rate_limit_key(headers: &HeaderMap, username: &str) -> String {
    let client = forwarded_ip(headers).unwrap_or("unknown");
    format!("{}|{}", client, username.trim().to_ascii_lowercase())
}

fn forwarded_ip(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
}

async fn login_attempt_is_blocked(state: &AppState, key: &str) -> bool {
    let now = OffsetDateTime::now_utc();
    let mut throttle = state.login_throttle.lock().await;
    cleanup_rate_limit_entries(&mut throttle, now, &state.config.login_rate_limit);
    throttle
        .get(key)
        .and_then(|entry| entry.blocked_until)
        .is_some_and(|blocked_until| blocked_until > now)
}

async fn record_login_failure(state: &AppState, key: &str) -> bool {
    let now = OffsetDateTime::now_utc();
    let mut throttle = state.login_throttle.lock().await;
    cleanup_rate_limit_entries(&mut throttle, now, &state.config.login_rate_limit);
    let entry = throttle.entry(key.to_string()).or_insert(LoginThrottleState {
        failures: 0,
        window_started_at: now,
        blocked_until: None,
    });

    if now - entry.window_started_at > state.config.login_rate_limit.window {
        entry.failures = 0;
        entry.window_started_at = now;
        entry.blocked_until = None;
    }

    entry.failures += 1;
    if entry.failures >= state.config.login_rate_limit.max_failures {
        entry.blocked_until = Some(now + state.config.login_rate_limit.block_for);
        true
    } else {
        false
    }
}

async fn clear_login_failures(state: &AppState, key: &str) {
    let mut throttle = state.login_throttle.lock().await;
    throttle.remove(key);
}

fn cleanup_rate_limit_entries(
    throttle: &mut HashMap<String, LoginThrottleState>,
    now: OffsetDateTime,
    config: &LoginRateLimit,
) {
    throttle.retain(|_, entry| {
        if let Some(blocked_until) = entry.blocked_until
            && blocked_until > now
        {
            return true;
        }
        now - entry.window_started_at <= config.window
    });
}

fn logout_gate_cookie_header(config: &AuthConfig) -> String {
    let secure = if config.secure_cookie { "; Secure" } else { "" };
    format!("{GATE_COOKIE}=deleted; Path=/; SameSite=Lax; Max-Age=0{secure}")
}

fn extract_cookie<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|part| {
            let trimmed = part.trim();
            let (cookie_name, cookie_value) = trimmed.split_once('=')?;
            (cookie_name == name).then_some(cookie_value)
        })
        .next()
}

fn sanitize_next(next: Option<&str>) -> &str {
    match next {
        Some(path) if path.starts_with('/') && !path.starts_with("//") => path,
        _ => "/",
    }
}

fn html_escape(raw: impl AsRef<str>) -> String {
    raw.as_ref()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> AuthConfig {
        AuthConfig {
            bind_addr: "127.0.0.1:8039".parse().unwrap(),
            static_dir: PathBuf::from("/tmp"),
            username: "demo".to_string(),
            password_policy: PasswordPolicy::PlaintextDev("secret123".to_string()),
            session_secret: b"01234567890123456789012345678901".to_vec(),
            secure_cookie: false,
            session_ttl: Duration::hours(12),
            login_rate_limit: LoginRateLimit {
                max_failures: 5,
                window: Duration::minutes(15),
                block_for: Duration::minutes(15),
            },
        }
    }

    #[test]
    fn token_roundtrip_is_valid() {
        let config = sample_config();
        let token = build_session_token(&config, OffsetDateTime::now_utc().unix_timestamp() + 60)
            .expect("token");
        assert!(session_is_valid(&config, &token));
    }

    #[test]
    fn sanitize_next_rejects_external_urls() {
        assert_eq!(sanitize_next(Some("https://evil.example")), "/");
        assert_eq!(sanitize_next(Some("//evil.example")), "/");
        assert_eq!(sanitize_next(Some("/entries/1")), "/entries/1");
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
        assert!(
            validate_proxy_host(&reqwest::Url::parse("http://127.0.0.1/feed.xml").unwrap())
                .is_err()
        );
        assert!(
            validate_proxy_host(&reqwest::Url::parse("http://localhost/feed.xml").unwrap())
                .is_err()
        );
        assert!(
            validate_proxy_host(
                &reqwest::Url::parse("http://169.254.169.254/latest/meta-data").unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn generated_argon2_hash_verifies() {
        let hash = generate_password_hash("adminadmin").expect("hash");
        assert!(verify_password_hash(&hash, "adminadmin"));
        assert!(!verify_password_hash(&hash, "wrong-password"));
    }

    #[test]
    fn rate_limit_blocks_after_repeated_failures() {
        let mut entries = HashMap::new();
        let config = LoginRateLimit {
            max_failures: 2,
            window: Duration::minutes(15),
            block_for: Duration::minutes(15),
        };
        let now = OffsetDateTime::now_utc();
        entries.insert(
            "client".to_string(),
            LoginThrottleState {
                failures: 2,
                window_started_at: now,
                blocked_until: Some(now + config.block_for),
            },
        );
        cleanup_rate_limit_entries(&mut entries, now, &config);
        assert!(entries.contains_key("client"));
    }
}
