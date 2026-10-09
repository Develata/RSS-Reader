mod config;
mod login_flow;
mod login_page;
mod persisted_state;
mod rate_limit;
mod session;

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::{
    Extension, Form,
    extract::{ConnectInfo, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use time::OffsetDateTime;
use tokio::sync::{Mutex, Semaphore};

use self::config::verify_credentials;
pub(crate) use self::config::{AuthConfig, generate_password_hash, load_config};
use self::login_flow::{login_redirect, successful_login_response};
use self::login_page::{render_login_failure, render_login_page};
use self::rate_limit::{
    LoginThrottleState, clear_login_failures, login_attempt_is_blocked, rate_limit_key,
    record_login_failure,
};
use self::session::{
    DEVICE_COOKIE, SESSION_COOKIE, build_device_token, build_session_token, device_cookie_header,
    device_nonce, extract_cookie, gate_cookie_header, logout_cookie_header,
    logout_gate_cookie_header, session_cookie_header, session_is_valid,
};

/// 同时进行的密码校验上限。Argon2 每次都要占用一个阻塞线程和可观的内存，登录请求再多
/// 也只排队，不会把服务拖垮（审计 F2）。
const MAX_CONCURRENT_CREDENTIAL_CHECKS: usize = 2;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<AuthConfig>,
    pub(crate) login_throttle: Arc<Mutex<HashMap<String, LoginThrottleState>>>,
    credential_checks: Arc<Semaphore>,
}

impl AppState {
    pub(crate) fn new(config: Arc<AuthConfig>) -> Self {
        Self {
            config,
            login_throttle: Arc::default(),
            credential_checks: Arc::new(Semaphore::new(MAX_CONCURRENT_CREDENTIAL_CHECKS)),
        }
    }

    /// 在阻塞线程池里做 Argon2 校验，不占用异步 worker。
    async fn check_credentials(&self, username: String, password: String) -> bool {
        let Ok(_permit) = self.credential_checks.acquire().await else {
            return false;
        };
        let config = Arc::clone(&self.config);
        tokio::task::spawn_blocking(move || verify_credentials(&config, &username, &password))
            .await
            .unwrap_or(false)
    }
}

/// 环境变量是进程级的，而 `cargo test` 默认多线程并行跑同一个测试二进制。
/// 认证配置相关的测试会 set/remove `HOME`、`RSS_READER_WEB_AUTH_STATE_FILE` 等变量，
/// 并行执行时会互相踩掉对方刚设置的值（表现为偶发失败、重跑又通过）。
/// 所有改动进程环境变量的测试都必须先取这把锁。
#[cfg(test)]
pub(crate) mod test_env {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    /// 忽略 poisoning：某个测试 panic 不应该把其余测试全部连带失败。
    pub(crate) fn lock() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Deserialize, Default)]
pub(crate) struct LoginQuery {
    next: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct LoginForm {
    username: String,
    password: String,
    next: Option<String>,
}

pub(crate) async fn show_login(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
) -> impl IntoResponse {
    let next = sanitize_next(query.next.as_deref());
    Html(render_login_page(&state.config, next, query.error.as_deref()))
}

pub(crate) async fn handle_login(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let next = sanitize_next(form.next.as_deref());
    let peer = peer.map(|Extension(ConnectInfo(address))| address.ip());
    let rate_limit_key = rate_limit_key(&state.config, &headers, peer, &form.username);

    if let Some(key) = &rate_limit_key
        && login_attempt_is_blocked(&state, key).await
    {
        return login_redirect(next, Some("rate_limited"));
    }

    if !state.check_credentials(form.username, form.password).await {
        let blocked = match &rate_limit_key {
            Some(key) => record_login_failure(&state, key).await,
            None => false,
        };
        return login_redirect(
            next,
            Some(if blocked { "rate_limited" } else { "invalid_credentials" }),
        );
    }

    if let Some(key) = &rate_limit_key {
        clear_login_failures(&state, key).await;
    }

    let expires_at = OffsetDateTime::now_utc() + state.config.session_ttl;
    let token = match build_session_token(&state.config, expires_at.unix_timestamp()) {
        Ok(token) => token,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(render_login_failure(err.to_string())),
            )
                .into_response();
        }
    };
    // 已经是已知设备就沿用原标记，失败计数仍落在同一个键上；否则签发一个。
    let known_device = extract_cookie(&headers, DEVICE_COOKIE)
        .and_then(|value| device_nonce(&state.config, value))
        .is_some();
    let device_cookie = if known_device {
        None
    } else {
        build_device_token(&state.config)
            .map(|device| device_cookie_header(&device, &state.config))
            .ok()
    };

    successful_login_response(&state.config, next, &token, device_cookie)
}

pub(crate) async fn handle_logout(State(state): State<AppState>) -> impl IntoResponse {
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

pub(crate) async fn session_probe() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

pub(crate) fn issue_smoke_auth_cookie_headers(
    config: &AuthConfig,
) -> anyhow::Result<(String, String)> {
    let expires_at = OffsetDateTime::now_utc() + config.session_ttl;
    let token = build_session_token(config, expires_at.unix_timestamp())?;
    Ok((session_cookie_header(&token, config), gate_cookie_header(config)))
}

pub(crate) async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(cookie_value) = extract_cookie(request.headers(), SESSION_COOKIE) {
        if session_is_valid(&state.config, cookie_value) {
            return next.run(request).await;
        }

        let target = sanitize_next(Some(
            request.uri().path_and_query().map(|value| value.as_str()).unwrap_or("/"),
        ));
        return login_redirect(target, Some("session_expired"));
    }

    let target = sanitize_next(Some(
        request.uri().path_and_query().map(|value| value.as_str()).unwrap_or("/"),
    ));
    login_redirect(target, None)
}

fn sanitize_next(next: Option<&str>) -> &str {
    match next {
        Some(path)
            if path.starts_with('/')
                && !path.starts_with("//")
                // Browsers normalize backslashes and strip some controls before resolving URLs.
                && !path.bytes().any(|byte| byte == b'\\' || byte.is_ascii_control()) =>
        {
            path
        }
        _ => "/",
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::auth::config::{LoginRateLimit, PasswordPolicy, verify_password_hash};
    use crate::auth::rate_limit::cleanup_rate_limit_entries;
    use time::Duration;

    unsafe fn set_test_var(name: &str, value: impl AsRef<std::ffi::OsStr>) {
        unsafe {
            std::env::set_var(name, value);
        }
    }

    unsafe fn remove_test_var(name: &str) {
        unsafe {
            std::env::remove_var(name);
        }
    }

    fn sample_config() -> AuthConfig {
        AuthConfig {
            bind_addr: "127.0.0.1:8039".parse().unwrap(),
            static_dir: PathBuf::from("/tmp"),
            username: "demo".to_string(),
            password_policy: PasswordPolicy::Argon2Hash(
                generate_password_hash("secret123").expect("sample hash"),
            ),
            session_secret: b"01234567890123456789012345678901".to_vec(),
            secure_cookie: false,
            trust_proxy_headers: false,
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
    fn sanitized_login_redirect_stays_same_origin_and_is_a_valid_header() {
        let origin = url::Url::parse("https://reader.example/").unwrap();
        for input in
            ["/\\evil.example", "/\t/evil.example", "/\n/evil.example", "/entries\r\nX-Test: bad"]
        {
            let target = sanitize_next(Some(input));
            assert_eq!(origin.join(target).unwrap().origin(), origin.origin(), "{input:?}");
            assert!(axum::http::HeaderValue::from_str(target).is_ok(), "{input:?}");
        }
        assert_eq!(
            sanitize_next(Some("/entries?q=%E4%B8%AD%E6%96%87")),
            "/entries?q=%E4%B8%AD%E6%96%87"
        );
    }

    #[test]
    fn generated_argon2_hash_verifies() {
        let hash = generate_password_hash("adminadmin").expect("hash");
        assert!(verify_password_hash(&hash, "adminadmin"));
        assert!(!verify_password_hash(&hash, "wrong-password"));
    }

    #[test]
    fn plaintext_password_bootstraps_persisted_hash_file() {
        let _env_guard = test_env::lock();
        let auth_file = std::env::temp_dir().join(format!(
            "rssr-web-auth-{}-{}.json",
            std::process::id(),
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));

        let config =
            load_config_from_test_file(&auth_file, Some("adminadmin"), None).expect("config");
        match config.password_policy {
            PasswordPolicy::Argon2Hash(hash) => {
                assert!(verify_password_hash(&hash, "adminadmin"));
            }
        }

        let persisted = fs::read_to_string(&auth_file).expect("persisted auth state should exist");
        assert!(persisted.contains("password_hash"));

        let _ = fs::remove_file(auth_file);
    }

    #[test]
    fn persisted_hash_is_used_without_password_env() {
        let _env_guard = test_env::lock();
        let auth_file = std::env::temp_dir().join(format!(
            "rssr-web-auth-{}-{}.json",
            std::process::id(),
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        let hash = generate_password_hash("adminadmin").expect("hash");
        let session_secret = "01234567890123456789012345678901";
        fs::write(
            &auth_file,
            format!("{{\"password_hash\":\"{}\",\"session_secret\":\"{}\"}}", hash, session_secret),
        )
        .expect("write auth state");

        let config = load_config_from_test_file(&auth_file, None, None).expect("config");
        match config.password_policy {
            PasswordPolicy::Argon2Hash(persisted) => {
                assert!(verify_password_hash(&persisted, "adminadmin"));
            }
        }

        let _ = fs::remove_file(auth_file);
    }

    #[test]
    fn missing_session_secret_is_generated_and_persisted() {
        let _env_guard = test_env::lock();
        let auth_file = std::env::temp_dir().join(format!(
            "rssr-web-auth-{}-{}.json",
            std::process::id(),
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        let hash = generate_password_hash("adminadmin").expect("hash");
        fs::write(&auth_file, format!("{{\"password_hash\":\"{}\"}}", hash))
            .expect("write auth state");

        let config = load_config_from_test_file(&auth_file, None, None).expect("config");
        assert!(config.session_secret.len() >= 32);

        let persisted = fs::read_to_string(&auth_file).expect("read persisted state");
        assert!(persisted.contains("session_secret"));

        let _ = fs::remove_file(auth_file);
    }

    #[test]
    fn persisted_session_secret_is_reused() {
        let _env_guard = test_env::lock();
        let auth_file = std::env::temp_dir().join(format!(
            "rssr-web-auth-{}-{}.json",
            std::process::id(),
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        let hash = generate_password_hash("adminadmin").expect("hash");
        fs::write(
            &auth_file,
            format!(
                "{{\"password_hash\":\"{}\",\"session_secret\":\"01234567890123456789012345678901\"}}",
                hash
            ),
        )
        .expect("write auth state");

        let config = load_config_from_test_file(&auth_file, None, None).expect("config");
        assert_eq!(
            String::from_utf8(config.session_secret).expect("utf8"),
            "01234567890123456789012345678901"
        );

        let _ = fs::remove_file(auth_file);
    }

    const PEER: Option<std::net::IpAddr> =
        Some(std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, 7)));

    #[test]
    fn rate_limit_key_uses_the_peer_address_unless_proxies_are_trusted() {
        let config = sample_config();
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.10"));
        headers.insert("x-real-ip", HeaderValue::from_static("198.51.100.2"));

        let key = rate_limit_key(&config, &headers, PEER, "demo");

        assert_eq!(key.as_deref(), Some("client|192.0.2.7"));
        assert_eq!(
            rate_limit_key(&config, &headers, None, "demo").as_deref(),
            Some("client|unknown")
        );
    }

    /// 可信代理把对端地址追加在最右边；左边的项由客户端填写，可以随意伪造（审计 F1）。
    #[test]
    fn trusted_forwarded_for_uses_the_address_the_proxy_appended() {
        let mut config = sample_config();
        config.trust_proxy_headers = true;
        let mut headers = HeaderMap::new();
        headers.append("x-forwarded-for", HeaderValue::from_static("1.1.1.1, 203.0.113.10"));
        headers.append("x-forwarded-for", HeaderValue::from_static("6.6.6.6, 198.51.100.2"));

        let key = rate_limit_key(&config, &headers, PEER, "demo");

        assert_eq!(key.as_deref(), Some("client|198.51.100.2"));
    }

    #[test]
    fn unknown_usernames_never_allocate_rate_limit_keys() {
        let config = sample_config();
        assert_eq!(rate_limit_key(&config, &HeaderMap::new(), PEER, "admin"), None);
    }

    #[test]
    fn device_tokens_verify_only_with_the_issuing_secret_and_username() {
        let config = sample_config();
        let token = build_device_token(&config).expect("device token");
        let nonce = device_nonce(&config, &token).expect("valid device token");
        assert!(!nonce.is_empty() && !nonce.contains('.'));

        let mut other_secret = sample_config();
        other_secret.session_secret = b"abcdefghijabcdefghijabcdefghijab".to_vec();
        let mut other_user = sample_config();
        other_user.username = "someone".into();
        assert_eq!(device_nonce(&other_secret, &token), None);
        assert_eq!(device_nonce(&other_user, &token), None);
        assert_eq!(device_nonce(&config, &format!("{nonce}.AAAA")), None);
        // A session token must not pass as a device token.
        let session = build_session_token(&config, i64::MAX).expect("session token");
        assert_eq!(device_nonce(&config, &session), None);
    }

    fn login_form(password: &str) -> Form<LoginForm> {
        Form(LoginForm {
            username: "demo".into(),
            password: password.into(),
            next: Some("/entries".into()),
        })
    }

    async fn attempt(
        state: &AppState,
        cookie: Option<&str>,
        password: &str,
    ) -> (Option<String>, Vec<String>) {
        let mut headers = HeaderMap::new();
        if let Some(cookie) = cookie {
            headers.insert(header::COOKIE, HeaderValue::from_str(cookie).unwrap());
        }
        let peer = PEER.map(|ip| Extension(ConnectInfo(SocketAddr::new(ip, 40_000))));
        let response = handle_login(State(state.clone()), peer, headers, login_form(password))
            .await
            .into_response();
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let cookies = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok().map(String::from))
            .collect();
        (location, cookies)
    }

    /// 攻击者与所有者共用同一个客户端地址（例如都在同一个反向代理后面）时，把地址锁住也
    /// 锁不住所有者已登录过的浏览器；没有设备标记的新客户端仍受限速（审计 F1）。
    #[tokio::test]
    async fn exhausting_the_client_bucket_does_not_lock_out_a_known_device() {
        let mut config = sample_config();
        config.login_rate_limit.max_failures = 2;
        let state = AppState::new(Arc::new(config));

        let (location, cookies) = attempt(&state, None, "secret123").await;
        assert_eq!(location.as_deref(), Some("/entries"));
        let device = cookies
            .iter()
            .find_map(|cookie| cookie.strip_prefix(&format!("{DEVICE_COOKIE}=")))
            .and_then(|rest| rest.split(';').next())
            .expect("a first login issues a device cookie");
        let device_cookie = format!("{DEVICE_COOKIE}={device}");
        assert!(cookies.iter().any(|cookie| cookie.contains("Path=/login")));

        for _ in 0..2 {
            attempt(&state, None, "wrong-password").await;
        }
        let (location, _) = attempt(&state, None, "secret123").await;
        assert!(location.unwrap().contains("error=rate_limited"), "new clients stay throttled");

        let (location, cookies) = attempt(&state, Some(&device_cookie), "secret123").await;
        assert_eq!(location.as_deref(), Some("/entries"), "the known device still logs in");
        assert!(
            !cookies.iter().any(|cookie| cookie.starts_with(DEVICE_COOKIE)),
            "a known device keeps its marker"
        );
    }

    #[cfg(unix)]
    #[test]
    fn persisted_auth_state_file_uses_owner_only_permissions() {
        let _env_guard = test_env::lock();
        use std::os::unix::fs::PermissionsExt;

        let auth_file = std::env::temp_dir().join(format!(
            "rssr-web-auth-{}-{}.json",
            std::process::id(),
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        let hash = generate_password_hash("adminadmin").expect("hash");
        fs::write(
            &auth_file,
            format!(
                "{{\"password_hash\":\"{}\",\"session_secret\":\"01234567890123456789012345678901\"}}",
                hash
            ),
        )
        .expect("write auth state");

        let _ = load_config_from_test_file(&auth_file, None, None).expect("config");

        let mode = fs::metadata(&auth_file).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        let _ = fs::remove_file(auth_file);
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

    fn load_config_from_test_file(
        auth_state_file: &std::path::Path,
        plaintext_password: Option<&str>,
        password_hash: Option<&str>,
    ) -> anyhow::Result<AuthConfig> {
        unsafe {
            set_test_var("RSS_READER_WEB_BIND", "127.0.0.1:8039");
            set_test_var("RSS_READER_WEB_STATIC_DIR", "/tmp");
            set_test_var("RSS_READER_WEB_USERNAME", "admin");
            set_test_var("RSS_READER_WEB_AUTH_STATE_FILE", auth_state_file);
            remove_test_var("RSS_READER_WEB_SESSION_SECRET");
            remove_test_var("RSS_READER_WEB_PASSWORD");
            remove_test_var("RSS_READER_WEB_PASSWORD_HASH");
        }

        if let Some(password) = plaintext_password {
            unsafe {
                set_test_var("RSS_READER_WEB_PASSWORD", password);
            }
        }
        if let Some(hash) = password_hash {
            unsafe {
                set_test_var("RSS_READER_WEB_PASSWORD_HASH", hash);
            }
        }

        let result = load_config();

        unsafe {
            remove_test_var("RSS_READER_WEB_BIND");
            remove_test_var("RSS_READER_WEB_STATIC_DIR");
            remove_test_var("RSS_READER_WEB_USERNAME");
            remove_test_var("RSS_READER_WEB_AUTH_STATE_FILE");
            remove_test_var("RSS_READER_WEB_PASSWORD");
            remove_test_var("RSS_READER_WEB_PASSWORD_HASH");
        }

        result
    }
}
