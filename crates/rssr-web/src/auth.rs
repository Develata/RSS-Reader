mod config;
mod login_flow;
mod login_page;
mod persisted_state;
mod rate_limit;
mod session;
// Wired into the routes by the next commit (C2).
#[allow(dead_code)]
mod store;

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
    LoginThrottleState, clear_login_failures, is_device_key, login_attempt_is_blocked,
    rate_limit_key, record_login_failure,
};
use self::session::{
    SESSION_COOKIE, build_device_token, build_session_token, device_cookie_header, extract_cookie,
    gate_cookie_header, logout_cookie_header, logout_gate_cookie_header, session_cookie_header,
    session_is_valid,
};

/// 同时进行的密码校验上限。Argon2 每次都要占用一个阻塞线程和可观的内存，登录请求再多
/// 也不会把服务拖垮（审计 F2）。
const MAX_CONCURRENT_CREDENTIAL_CHECKS: usize = 2;
/// 带有效已知设备标记的请求另用的校验名额：其他来源占满上面的名额时，所有者登录过的浏览器
/// 仍能校验密码。设备标记由服务端签发，攻击者无法批量制造（审计 F1）。
const MAX_CONCURRENT_DEVICE_CREDENTIAL_CHECKS: usize = 1;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<AuthConfig>,
    pub(crate) login_throttle: Arc<Mutex<HashMap<String, LoginThrottleState>>>,
    credential_checks: Arc<Semaphore>,
    device_credential_checks: Arc<Semaphore>,
}

impl AppState {
    pub(crate) fn new(config: Arc<AuthConfig>) -> Self {
        Self {
            config,
            login_throttle: Arc::default(),
            credential_checks: Arc::new(Semaphore::new(MAX_CONCURRENT_CREDENTIAL_CHECKS)),
            device_credential_checks: Arc::new(Semaphore::new(
                MAX_CONCURRENT_DEVICE_CREDENTIAL_CHECKS,
            )),
        }
    }

    /// 校验密码，失败时记到 `key` 上。
    async fn check_and_record(
        &self,
        key: String,
        username: String,
        password: String,
    ) -> CredentialCheck {
        let config = Arc::clone(&self.config);
        self.check_and_record_with(key, move || verify_credentials(&config, &username, &password))
            .await
    }

    /// 在阻塞线程池里运行 `verify`（Argon2 不占用异步 worker），失败时记到 `key` 上。
    ///
    /// 名额在派生任务之前以不等待的方式取得，没有空闲名额就直接返回 `Busy`，不排队，也不派生
    /// 任务；取得的名额由任务持有到记账结束，所以同时存在的校验任务不超过名额数（审计 F2）。
    /// 校验与记账在这个独立任务里完成：客户端断开导致请求被丢弃时，任务照常跑完并记下失败，
    /// 已经开始的 Argon2 也照常占着名额，断开再重试既不能绕过限速，也不能突破并发上限（独立
    /// 评审）。
    async fn check_and_record_with(
        &self,
        key: String,
        verify: impl FnOnce() -> bool + Send + 'static,
    ) -> CredentialCheck {
        let slots = if is_device_key(&key) {
            &self.device_credential_checks
        } else {
            &self.credential_checks
        };
        let Ok(slot) = Arc::clone(slots).try_acquire_owned() else {
            return CredentialCheck::Busy;
        };
        let state = self.clone();
        tokio::spawn(async move {
            let _slot = slot;
            if tokio::task::spawn_blocking(verify).await.unwrap_or(false) {
                CredentialCheck::Valid
            } else {
                CredentialCheck::Invalid { blocked: record_login_failure(&state, &key).await }
            }
        })
        .await
        .unwrap_or(CredentialCheck::Busy)
    }
}

/// 一次密码校验的结果。
#[derive(Debug)]
enum CredentialCheck {
    /// 校验名额已满，没有校验。
    Busy,
    Valid,
    /// 密码不对；`blocked` 表示这次失败让来源进入了限速。
    Invalid {
        blocked: bool,
    },
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
    let rate_limit_key = rate_limit_key(&state.config, &headers, peer);

    if login_attempt_is_blocked(&state, &rate_limit_key).await {
        return login_redirect(next, Some("rate_limited"));
    }

    let known_device = is_device_key(&rate_limit_key);
    match state.check_and_record(rate_limit_key.clone(), form.username, form.password).await {
        CredentialCheck::Busy => return login_redirect(next, Some("rate_limited")),
        CredentialCheck::Invalid { blocked } => {
            return login_redirect(
                next,
                Some(if blocked { "rate_limited" } else { "invalid_credentials" }),
            );
        }
        CredentialCheck::Valid => {}
    }

    clear_login_failures(&state, &rate_limit_key).await;

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
    use crate::auth::rate_limit::{MAX_TRACKED_LOGIN_KEYS, cleanup_rate_limit_entries};
    use crate::auth::session::{DEVICE_COOKIE, device_nonce};
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
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

        let key = rate_limit_key(&config, &headers, PEER);

        assert_eq!(key, "client|192.0.2.7");
        assert_eq!(rate_limit_key(&config, &headers, None), "client|unknown");
    }

    /// 可信代理把对端地址追加在最右边；左边的项由客户端填写，可以随意伪造（审计 F1）。
    #[test]
    fn trusted_forwarded_for_uses_the_address_the_proxy_appended() {
        let mut config = sample_config();
        config.trust_proxy_headers = true;
        let mut headers = HeaderMap::new();
        headers.append("x-forwarded-for", HeaderValue::from_static("1.1.1.1, 203.0.113.10"));
        headers.append("x-forwarded-for", HeaderValue::from_static("6.6.6.6, 198.51.100.2"));

        let key = rate_limit_key(&config, &headers, PEER);

        assert_eq!(key, "client|198.51.100.2");
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

    async fn attempt(
        state: &AppState,
        cookie: Option<&str>,
        password: &str,
    ) -> (Option<String>, Vec<String>) {
        attempt_as(state, PEER.unwrap(), cookie, "demo", password).await
    }

    async fn attempt_as(
        state: &AppState,
        peer: std::net::IpAddr,
        cookie: Option<&str>,
        username: &str,
        password: &str,
    ) -> (Option<String>, Vec<String>) {
        let mut headers = HeaderMap::new();
        if let Some(cookie) = cookie {
            headers.insert(header::COOKIE, HeaderValue::from_str(cookie).unwrap());
        }
        let peer = Some(Extension(ConnectInfo(SocketAddr::new(peer, 40_000))));
        let form = Form(LoginForm {
            username: username.into(),
            password: password.into(),
            next: Some("/entries".into()),
        });
        let response =
            handle_login(State(state.clone()), peer, headers, form).await.into_response();
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

        let device_cookie = first_login_device_cookie(&state).await;

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

    /// 成功登录一次，返回签发的已知设备标记（作为请求 `Cookie` 头的值）。
    async fn first_login_device_cookie(state: &AppState) -> String {
        let (location, cookies) = attempt(state, None, "secret123").await;
        assert_eq!(location.as_deref(), Some("/entries"));
        assert!(cookies.iter().any(|cookie| cookie.contains("Path=/login")));
        let device = cookies
            .iter()
            .find_map(|cookie| cookie.strip_prefix(&format!("{DEVICE_COOKIE}=")))
            .and_then(|rest| rest.split(';').next())
            .expect("a first login issues a device cookie");
        format!("{DEVICE_COOKIE}={device}")
    }

    /// 用户名不存在时同样计数：否则只有真实用户名会被限速，攻击者据此就能判断用户名是否
    /// 存在，也能用错误用户名无限次占用密码校验（审计 F1）。
    #[tokio::test]
    async fn unknown_usernames_are_throttled_like_the_real_one() {
        let mut config = sample_config();
        config.login_rate_limit.max_failures = 2;
        let real = AppState::new(Arc::new(config.clone()));
        let unknown = AppState::new(Arc::new(config));
        let peer = PEER.unwrap();

        for _ in 0..2 {
            let (real_location, _) = attempt_as(&real, peer, None, "demo", "wrong").await;
            let (unknown_location, _) = attempt_as(&unknown, peer, None, "admin", "wrong").await;
            assert_eq!(real_location, unknown_location);
        }
        let (location, _) = attempt_as(&unknown, peer, None, "demo", "secret123").await;
        assert!(location.unwrap().contains("error=rate_limited"));
    }

    /// 计数表满后，新的客户端地址共用溢出计数，照样会被限速；已知设备不受影响（审计 F1）。
    #[tokio::test]
    async fn a_full_failure_table_still_throttles_new_clients() {
        let mut config = sample_config();
        config.login_rate_limit.max_failures = 2;
        let state = AppState::new(Arc::new(config));
        let device_cookie = first_login_device_cookie(&state).await;
        {
            let now = OffsetDateTime::now_utc();
            let mut throttle = state.login_throttle.lock().await;
            for index in 0..MAX_TRACKED_LOGIN_KEYS {
                throttle.insert(
                    format!("client|10.0.{}.{}", index / 256, index % 256),
                    LoginThrottleState { failures: 1, window_started_at: now, blocked_until: None },
                );
            }
        }
        let first: std::net::IpAddr = "198.51.100.1".parse().unwrap();
        let second: std::net::IpAddr = "198.51.100.2".parse().unwrap();

        let (location, _) = attempt_as(&state, first, None, "demo", "wrong").await;
        assert!(location.unwrap().contains("error=invalid_credentials"));
        let (location, _) = attempt_as(&state, second, None, "demo", "wrong").await;
        assert!(location.unwrap().contains("error=rate_limited"));
        let (location, _) = attempt_as(&state, first, None, "demo", "secret123").await;
        assert!(location.unwrap().contains("error=rate_limited"));

        let (location, _) = attempt(&state, Some(&device_cookie), "secret123").await;
        assert_eq!(location.as_deref(), Some("/entries"));
    }

    /// 其他来源占满密码校验名额时，已知设备仍有自己的名额（审计 F1）。
    #[tokio::test]
    async fn a_known_device_has_its_own_credential_check_slot() {
        let state = AppState::new(Arc::new(sample_config()));
        let device_cookie = first_login_device_cookie(&state).await;
        let _busy = Arc::clone(&state.credential_checks)
            .try_acquire_many_owned(MAX_CONCURRENT_CREDENTIAL_CHECKS as u32)
            .expect("all general slots");

        let (location, _) = attempt(&state, None, "secret123").await;
        assert!(location.unwrap().contains("error=rate_limited"));
        let (location, _) = attempt(&state, Some(&device_cookie), "secret123").await;
        assert_eq!(location.as_deref(), Some("/entries"));
    }

    /// 客户端在密码校验期间断开、请求被丢弃：已经开始的校验继续占着名额，结束后照样记下失败；
    /// 否则断开再重试既能突破并发上限，也能一直校验而不被限速（审计 F2、独立评审）。
    /// 校验函数停在闸门上，取消一定发生在校验期间。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_check_abandoned_by_its_request_keeps_its_slot_and_records_the_failure() {
        let state = AppState::new(Arc::new(sample_config()));
        let (started, started_rx) = tokio::sync::oneshot::channel();
        let (release, release_rx) = std::sync::mpsc::channel::<()>();
        let request = tokio::spawn({
            let state = state.clone();
            async move {
                state
                    .check_and_record_with("client|192.0.2.7".into(), move || {
                        started.send(()).expect("the test waits for the check");
                        release_rx.recv().expect("the test releases the check");
                        false
                    })
                    .await
            }
        });
        within(started_rx).await.expect("the check started");
        request.abort();
        assert!(within(request).await.expect_err("the request was dropped").is_cancelled());
        assert_eq!(
            state.credential_checks.available_permits(),
            MAX_CONCURRENT_CREDENTIAL_CHECKS - 1,
            "the running check keeps its slot"
        );

        release.send(()).expect("the check is waiting");
        within(async {
            while state.credential_checks.available_permits() < MAX_CONCURRENT_CREDENTIAL_CHECKS {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await;
        let failures =
            state.login_throttle.lock().await.get("client|192.0.2.7").map(|e| e.failures);
        assert_eq!(failures, Some(1), "the abandoned check recorded its failure");
    }

    /// 名额已满时第一次 poll 就返回 `Busy`：不运行校验，也不派生任务。`tokio::spawn` 不会同步
    /// poll 新任务，先派生、在任务里取名额的写法第一次 poll 只能得到 `Pending`（审计 F2、独立
    /// 评审）。
    #[tokio::test]
    async fn a_busy_check_returns_at_once_without_running_anything() {
        let state = AppState::new(Arc::new(sample_config()));
        let _busy = Arc::clone(&state.credential_checks)
            .try_acquire_many_owned(MAX_CONCURRENT_CREDENTIAL_CHECKS as u32)
            .expect("all general slots");
        let ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut check = std::pin::pin!(state.check_and_record_with("client|192.0.2.7".into(), {
            let ran = Arc::clone(&ran);
            move || {
                ran.store(true, std::sync::atomic::Ordering::SeqCst);
                false
            }
        }));

        let first =
            std::future::poll_fn(|cx| std::task::Poll::Ready(check.as_mut().poll(cx))).await;
        assert!(matches!(first, std::task::Poll::Ready(CredentialCheck::Busy)));
        assert!(!ran.load(std::sync::atomic::Ordering::SeqCst));
        assert!(state.login_throttle.lock().await.is_empty(), "nothing is recorded");
    }

    const WATCHDOG: std::time::Duration = std::time::Duration::from_secs(10);

    async fn within<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::time::timeout(WATCHDOG, future).await.expect("watchdog")
    }

    #[test]
    fn session_tokens_must_be_signed_current_and_for_this_user() {
        let config = sample_config();
        let future = OffsetDateTime::now_utc().unix_timestamp() + 60;
        assert!(session_is_valid(&config, &build_session_token(&config, future).unwrap()));

        let expired = build_session_token(&config, 1).unwrap();
        assert!(!session_is_valid(&config, &expired));
        let mut other_user = sample_config();
        other_user.username = "someone".into();
        assert!(!session_is_valid(&config, &build_session_token(&other_user, future).unwrap()));
        let payload = URL_SAFE_NO_PAD.encode(format!("demo:{future}"));
        assert!(!session_is_valid(&config, &format!("{payload}.AAAA")));
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
