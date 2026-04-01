use std::{env, net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{Context, Result, anyhow, ensure};
use axum::{
    Form, Router,
    extract::{Query, Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use time::{Duration, OffsetDateTime};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;
use tracing_subscriber::EnvFilter;

type HmacSha256 = Hmac<Sha256>;

const SESSION_COOKIE: &str = "rssr_web_session";
const APP_NAME: &str = "RSS-Reader";
const WEB_LOGIN_MARKUP: &str = include_str!("../../../assets/branding/rssr-mark.svg");

#[derive(Clone)]
struct AppState {
    config: Arc<AuthConfig>,
}

#[derive(Clone)]
struct AuthConfig {
    bind_addr: SocketAddr,
    static_dir: PathBuf,
    username: String,
    password: String,
    session_secret: Vec<u8>,
    secure_cookie: bool,
    session_ttl: Duration,
}

#[derive(Deserialize, Default)]
struct LoginQuery {
    next: Option<String>,
    error: Option<String>,
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

    let config = Arc::new(load_config()?);
    ensure!(
        config.static_dir.join("index.html").exists(),
        "静态资源目录缺少 index.html：{}",
        config.static_dir.display()
    );

    let protected = Router::new()
        .fallback_service(
            ServeDir::new(config.static_dir.clone())
                .not_found_service(ServeFile::new(config.static_dir.join("index.html"))),
        )
        .layer(middleware::from_fn_with_state(AppState { config: config.clone() }, require_auth));

    let app = Router::new()
        .route("/login", get(show_login).post(handle_login))
        .route("/logout", get(handle_logout))
        .route("/healthz", get(healthz))
        .merge(protected)
        .with_state(AppState { config: config.clone() });

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
    let password = required_env("RSS_READER_WEB_PASSWORD")?;
    let session_secret = required_env("RSS_READER_WEB_SESSION_SECRET")?;
    ensure!(session_secret.len() >= 32, "RSS_READER_WEB_SESSION_SECRET 至少需要 32 个字符");
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

    Ok(AuthConfig {
        bind_addr,
        static_dir,
        username,
        password,
        session_secret: session_secret.into_bytes(),
        secure_cookie,
        session_ttl: Duration::hours(session_ttl_hours),
    })
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).map_err(|_| anyhow!("缺少环境变量：{name}"))
}

async fn show_login(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
) -> impl IntoResponse {
    let next = sanitize_next(query.next.as_deref());
    let error_message = match query.error.as_deref() {
        Some("invalid_credentials") => "用户名或密码错误。",
        Some("session_expired") => "登录已过期，请重新登录。",
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
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let next = sanitize_next(form.next.as_deref());
    if !credentials_match(&state.config, &form.username, &form.password) {
        return Redirect::to(&format!(
            "/login?error=invalid_credentials&next={}",
            urlencoding::encode(next)
        ))
        .into_response();
    }

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
    response
}

async fn handle_logout(State(state): State<AppState>) -> impl IntoResponse {
    let mut response = Redirect::to("/login").into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&logout_cookie_header(&state.config)).expect("valid logout cookie"),
    );
    response
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
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
    username == config.username && password == config.password
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
            password: "secret".to_string(),
            session_secret: b"01234567890123456789012345678901".to_vec(),
            secure_cookie: false,
            session_ttl: Duration::hours(12),
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
}
