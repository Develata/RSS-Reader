mod config;
mod login_flow;
mod login_page;
mod persisted_state;
mod rate_limit;
mod session;

use std::{collections::HashMap, sync::Arc};

use axum::{
    Form,
    extract::{Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use time::OffsetDateTime;
use tokio::sync::Mutex;

use self::config::verify_credentials;
pub(crate) use self::config::{AuthConfig, generate_password_hash, load_config};
use self::login_flow::{login_redirect, successful_login_response};
use self::login_page::{render_login_failure, render_login_page};
use self::rate_limit::{
    LoginThrottleState, clear_login_failures, login_attempt_is_blocked, rate_limit_key,
    record_login_failure,
};
use self::session::{
    SESSION_COOKIE, build_session_token, extract_cookie, gate_cookie_header, logout_cookie_header,
    logout_gate_cookie_header, session_cookie_header, session_is_valid,
};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<AuthConfig>,
    pub(crate) login_throttle: Arc<Mutex<HashMap<String, LoginThrottleState>>>,
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
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let next = sanitize_next(form.next.as_deref());
    let rate_limit_key = rate_limit_key(&state.config, &headers, &form.username);

    if login_attempt_is_blocked(&state, &rate_limit_key).await {
        return login_redirect(next, Some("rate_limited"));
    }

    if !verify_credentials(&state.config, &form.username, &form.password) {
        let blocked = record_login_failure(&state, &rate_limit_key).await;
        return login_redirect(
            next,
            Some(if blocked { "rate_limited" } else { "invalid_credentials" }),
        );
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

    successful_login_response(&state.config, next, &token)
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
        Some(path) if path.starts_with('/') && !path.starts_with("//") => path,
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
    fn generated_argon2_hash_verifies() {
        let hash = generate_password_hash("adminadmin").expect("hash");
        assert!(verify_password_hash(&hash, "adminadmin"));
        assert!(!verify_password_hash(&hash, "wrong-password"));
    }

    #[test]
    fn plaintext_password_bootstraps_persisted_hash_file() {
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

    #[test]
    fn rate_limit_key_ignores_proxy_headers_by_default() {
        let config = sample_config();
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.10"));
        headers.insert("x-real-ip", HeaderValue::from_static("198.51.100.2"));

        let key = rate_limit_key(&config, &headers, "admin");

        assert_eq!(key, "direct|admin");
    }

    #[test]
    fn rate_limit_key_uses_forwarded_headers_when_trusted() {
        let mut config = sample_config();
        config.trust_proxy_headers = true;

        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.10, 198.51.100.2"));

        let key = rate_limit_key(&config, &headers, "admin");

        assert_eq!(key, "203.0.113.10|admin");
    }

    #[cfg(unix)]
    #[test]
    fn persisted_auth_state_file_uses_owner_only_permissions() {
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
