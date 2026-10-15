mod config;
mod credentials;
mod legacy_state;
mod login_flow;
mod login_page;
mod origin;
mod rate_limit;
mod session;
mod store;

use std::{collections::HashMap, net::SocketAddr, path::Path, sync::Arc};

use anyhow::{Context, Result};
use axum::{
    Extension, Form, Json, Router,
    body::Bytes,
    extract::{ConnectInfo, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{any, get},
};
use serde::Deserialize;
use subtle::ConstantTimeEq;
use time::OffsetDateTime;
use tokio::sync::{Mutex, Semaphore};
use tracing::{error, info, warn};

#[cfg(test)]
pub(crate) use self::config::parse_public_origin;
pub(crate) use self::config::{AuthConfig, load_config};
pub(crate) use self::credentials::generate_password_hash;
use self::credentials::{SEED_VARIABLES, credentials_match, initial_credential};
use self::login_flow::{logged_out_response, login_redirect, successful_login_response};
use self::login_page::{render_login_page, render_logout_page};
use self::origin::{TrustedOrigins, is_safe};
use self::rate_limit::{
    LoginThrottleState, clear_login_failures, is_device_key, login_attempt_is_blocked,
    rate_limit_key, record_login_failure,
};
use self::session::{SESSION_COOKIE, build_device_token, device_cookie_header, extract_cookie};
pub(crate) use self::store::AuthStore;
use self::store::{Credential, SessionToken};

/// 同时进行的密码校验上限。Argon2 每次都要占用一个阻塞线程和可观的内存，登录请求再多
/// 也不会把服务拖垮（审计 F2）。
const MAX_CONCURRENT_CREDENTIAL_CHECKS: usize = 2;
/// 带有效已知设备标记的请求另用的校验名额：其他来源占满上面的名额时，所有者登录过的浏览器
/// 仍能校验密码。设备标记由服务端签发，攻击者无法批量制造（审计 F1）。
const MAX_CONCURRENT_DEVICE_CREDENTIAL_CHECKS: usize = 1;
/// 同时校验的会话上限。每个带会话 cookie 的请求（含静态资源）都要查一次认证库；名额满时
/// 直接返回 503，不排队：随意伪造的 cookie 再多，阻塞线程池与等待队列也有界（评审 V1）。
/// 正常的页面加载并发远低于这个数。
const MAX_CONCURRENT_SESSION_CHECKS: usize = 64;
/// 登录后的写请求携带 CSRF 令牌的请求头。
pub(crate) const CSRF_HEADER: &str = "x-rssr-csrf";

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<AuthConfig>,
    store: Arc<AuthStore>,
    origins: Arc<TrustedOrigins>,
    device_key: [u8; 32],
    pub(crate) login_throttle: Arc<Mutex<HashMap<String, LoginThrottleState>>>,
    credential_checks: Arc<Semaphore>,
    device_credential_checks: Arc<Semaphore>,
    session_checks: Arc<Semaphore>,
}

impl AppState {
    /// 为监听在 `local`（实际绑定的地址）上的服务建立状态。
    pub(crate) fn new(
        config: Arc<AuthConfig>,
        store: Arc<AuthStore>,
        local: SocketAddr,
    ) -> Result<Self> {
        let origins = TrustedOrigins::new(config.public_origin.as_ref(), local)?;
        let device_key = store.device_key()?;
        Ok(Self {
            config,
            store,
            origins: Arc::new(origins),
            device_key,
            login_throttle: Arc::default(),
            credential_checks: Arc::new(Semaphore::new(MAX_CONCURRENT_CREDENTIAL_CHECKS)),
            device_credential_checks: Arc::new(Semaphore::new(
                MAX_CONCURRENT_DEVICE_CREDENTIAL_CHECKS,
            )),
            session_checks: Arc::new(Semaphore::new(MAX_CONCURRENT_SESSION_CHECKS)),
        })
    }

    /// 测试用：占满会话校验名额。
    #[cfg(test)]
    pub(crate) fn exhaust_session_checks(&self) -> tokio::sync::OwnedSemaphorePermit {
        Arc::clone(&self.session_checks)
            .try_acquire_many_owned(MAX_CONCURRENT_SESSION_CHECKS as u32)
            .expect("all session check slots")
    }

    fn secure(&self) -> bool {
        self.origins.secure()
    }

    /// 读出凭据并校验密码，失败时记到 `key` 上。哈希与凭据代数来自同一次读取，服务器不缓存
    /// 哈希；校验通过时返回这份凭据，建立会话时再核对代数未变。
    async fn check_and_record(
        &self,
        key: String,
        username: String,
        password: String,
    ) -> CredentialCheck<Credential> {
        let store = Arc::clone(&self.store);
        let config = Arc::clone(&self.config);
        self.check_and_record_with(key, move || {
            let Some(credential) = store.credential()? else {
                return Ok(None);
            };
            let matches = credentials_match(
                &credential.password_hash,
                &config.username,
                &username,
                &password,
            );
            Ok(matches.then_some(credential))
        })
        .await
    }

    /// 在阻塞线程池里运行 `verify`（Argon2 不占用异步 worker），失败时记到 `key` 上。
    ///
    /// 名额在派生任务之前以不等待的方式取得，没有空闲名额就直接返回 `Busy`，不排队，也不派生
    /// 任务；取得的名额由任务持有到记账结束，所以同时存在的校验任务不超过名额数（审计 F2）。
    /// 校验与记账在这个独立任务里完成：客户端断开导致请求被丢弃时，任务照常跑完并记下失败，
    /// 已经开始的 Argon2 也照常占着名额，断开再重试既不能绕过限速，也不能突破并发上限（独立
    /// 评审）。读不出凭据（认证库出错）时没有校验任何密码，不记失败。
    async fn check_and_record_with<T: Send + 'static>(
        &self,
        key: String,
        verify: impl FnOnce() -> Result<Option<T>> + Send + 'static,
    ) -> CredentialCheck<T> {
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
            match tokio::task::spawn_blocking(verify).await {
                Ok(Ok(Some(value))) => CredentialCheck::Valid(value),
                Ok(Err(error)) => {
                    error!(%error, "读取凭据失败");
                    CredentialCheck::Unavailable
                }
                Ok(Ok(None)) | Err(_) => {
                    CredentialCheck::Invalid { blocked: record_login_failure(&state, &key).await }
                }
            }
        })
        .await
        .unwrap_or(CredentialCheck::Busy)
    }
}

/// 一次密码校验的结果。
#[derive(Debug)]
enum CredentialCheck<T> {
    /// 校验名额已满，没有校验。
    Busy,
    /// 认证库出错，没有校验。
    Unavailable,
    Valid(T),
    /// 密码不对；`blocked` 表示这次失败让来源进入了限速。
    Invalid {
        blocked: bool,
    },
}

/// 服务器打开数据目录里的认证库；还没有凭据时按决定 Q1 的顺序建立它。已有凭据时，凭据
/// 环境变量不再读取（改密码用 `rssr-web passwd`）。然后开始本次运行：上次没有正常结束时吊销
/// 全部会话（[`AuthStore::begin_run`]）；最后清掉已经失效的会话。认证库不可写时启动失败。
/// 停止时调用 [`AuthStore::end_run`]。
pub(crate) fn open_store(data_dir: &Path) -> Result<AuthStore> {
    let store = AuthStore::open(data_dir)?;
    let mut source = None;
    let seeded = store.initialize(|| {
        let (hash, from) = initial_credential()?;
        source = Some(from);
        Ok(hash)
    })?;
    if seeded {
        info!(source = source.as_deref().unwrap_or_default(), "已建立认证库的所有者凭据");
    } else if SEED_VARIABLES.iter().any(|name| std::env::var_os(name).is_some()) {
        warn!("认证库已有凭据，忽略 {}；改密码请用 rssr-web passwd", SEED_VARIABLES.join("、"));
    }
    if store.begin_run()? {
        warn!("上次运行没有正常结束，或停止时仍有删不掉的失效会话：已吊销全部会话，需要重新登录");
    }
    let purged = store.purge_expired(now()).context("清理失效的会话失败")?;
    if purged > 0 {
        info!(purged, "启动时清理了失效的会话");
    }
    Ok(store)
}

/// 给应用加上认证（v0.2 阶段 C，C2）：
/// - `protected` 的路由要求有效会话，未登录时跳转登录页；
/// - `/api` 下的一切要求有效会话，未登录时返回 JSON 401，不存在的接口返回 JSON 404，从不落入
///   页面回退；写请求还要在 [`CSRF_HEADER`] 里带上本会话的 CSRF 令牌（`GET /api/v1/session`）；
/// - 登录与退出登录（`GET /logout` 是确认页，`POST /logout` 才退出）；
/// - 最外层：每个写请求的 `Origin` 必须是信任的来源（[`TrustedOrigins`]）。
pub(crate) fn with_auth(protected: Router<AppState>, state: AppState) -> Router {
    let protected = protected.layer(middleware::from_fn_with_state(state.clone(), require_auth));
    let api = Router::new()
        .route("/api/v1/session", get(api_session))
        .route("/api", any(api_not_found))
        .route("/api/", any(api_not_found))
        .route("/api/{*rest}", any(api_not_found))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_api_session));
    Router::new()
        .route("/login", get(show_login).post(handle_login))
        .route("/logout", get(confirm_logout).post(handle_logout))
        .merge(api)
        .merge(protected)
        .layer(middleware::from_fn_with_state(state.clone(), require_trusted_origin))
        .with_state(state)
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

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use time::Duration;

    use super::config::LoginRateLimit;
    use super::{AppState, AuthConfig, AuthStore, generate_password_hash};

    /// 测试用的临时目录，结束时删除。
    pub(crate) struct ScratchDir(PathBuf);

    impl ScratchDir {
        pub(crate) fn new(label: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("rssr-web-{label}-{}-{nonce}-{sequence}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        pub(crate) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub(crate) fn sample_config() -> AuthConfig {
        AuthConfig {
            bind_addr: "127.0.0.1:8039".parse().unwrap(),
            static_dir: PathBuf::from("/tmp"),
            username: "demo".to_string(),
            public_origin: None,
            trust_proxy_headers: false,
            login_rate_limit: LoginRateLimit {
                max_failures: 5,
                window: Duration::minutes(15),
                block_for: Duration::minutes(15),
            },
        }
    }

    /// 凭据为 demo / secret123 的认证库，在 `dir` 下的 `data`。
    pub(crate) fn sample_store(dir: &ScratchDir) -> Arc<AuthStore> {
        let store = AuthStore::open(&dir.path().join("data")).unwrap();
        store.change_password(&generate_password_hash("secret123").unwrap()).unwrap();
        Arc::new(store)
    }

    /// 凭据为 demo / secret123、监听 127.0.0.1:8039 的认证状态。
    pub(crate) fn state_with(config: AuthConfig) -> (AppState, ScratchDir) {
        let dir = ScratchDir::new("state");
        let store = sample_store(&dir);
        let state =
            AppState::new(Arc::new(config), store, "127.0.0.1:8039".parse().unwrap()).unwrap();
        (state, dir)
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

fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

pub(crate) async fn show_login(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
) -> impl IntoResponse {
    let next = sanitize_next(query.next.as_deref());
    Html(render_login_page(state.secure(), next, query.error.as_deref()))
}

/// 登录表单（`Origin` 已在外层核对）。限速与校验名额见 [`AppState::check_and_record_with`]。
pub(crate) async fn handle_login(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    let next = sanitize_next(form.next.as_deref());
    let peer = peer.map(|Extension(ConnectInfo(address))| address.ip());
    let rate_limit_key = rate_limit_key(&state.config, &state.device_key, &headers, peer);

    if login_attempt_is_blocked(&state, &rate_limit_key).await {
        return login_redirect(next, Some("rate_limited"));
    }

    let known_device = is_device_key(&rate_limit_key);
    let credential =
        match state.check_and_record(rate_limit_key.clone(), form.username, form.password).await {
            CredentialCheck::Busy => return login_redirect(next, Some("rate_limited")),
            CredentialCheck::Unavailable => return login_redirect(next, Some("unavailable")),
            CredentialCheck::Invalid { blocked } => {
                return login_redirect(
                    next,
                    Some(if blocked { "rate_limited" } else { "invalid_credentials" }),
                );
            }
            CredentialCheck::Valid(credential) => credential,
        };

    clear_login_failures(&state, &rate_limit_key).await;

    let store = Arc::clone(&state.store);
    let token =
        match tokio::task::spawn_blocking(move || store.create_session(&credential, now())).await {
            Ok(Ok(Some(token))) => token,
            // 核对之后改了密码：核对的是旧密码。
            Ok(Ok(None)) => return login_redirect(next, Some("invalid_credentials")),
            Ok(Err(error)) => {
                error!(%error, "建立会话失败");
                return login_redirect(next, Some("unavailable"));
            }
            Err(error) => {
                error!(%error, "建立会话的任务失败");
                return login_redirect(next, Some("unavailable"));
            }
        };
    // 已经是已知设备就沿用原标记，失败计数仍落在同一个键上；否则签发一个。
    let device_cookie = (!known_device).then(|| {
        device_cookie_header(
            &build_device_token(&state.device_key, &state.config.username),
            state.secure(),
        )
    });

    successful_login_response(state.secure(), next, token.as_str(), device_cookie)
}

/// `GET /logout`：确认页，表单带上本会话的 CSRF 令牌。没有有效会话时回到登录页。
pub(crate) async fn confirm_logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match session_status(&state, &headers).await {
        Ok(SessionStatus::Valid(token)) => {
            no_store(Html(render_logout_page(&token.csrf())).into_response())
        }
        Ok(SessionStatus::Busy) => unavailable(),
        Ok(SessionStatus::Missing | SessionStatus::Invalid) => {
            Redirect::to("/login").into_response()
        }
        Err(error) => {
            error!(%error, "校验会话失败");
            unavailable()
        }
    }
}

/// `POST /logout`：吊销本会话（`Origin` 已在外层核对）。CSRF 令牌来自确认页的表单字段
/// `csrf`，或请求头 [`CSRF_HEADER`]。没有会话 cookie 时只清掉 cookie。
pub(crate) async fn handle_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(token) = extract_cookie(&headers, SESSION_COOKIE).and_then(SessionToken::parse) {
        let provided =
            headers.get(CSRF_HEADER).map(|value| value.as_bytes().to_vec()).or_else(|| {
                url::form_urlencoded::parse(&body)
                    .find(|(name, _)| name == "csrf")
                    .map(|(_, value)| value.into_owned().into_bytes())
            });
        if !csrf_matches(provided.as_deref(), &token.csrf()) {
            return (StatusCode::FORBIDDEN, "缺少或错误的 CSRF 令牌").into_response();
        }
        // 与会话校验共用名额：伪造的令牌再多，退出请求也排不出无界的阻塞任务。
        let Ok(slot) = Arc::clone(&state.session_checks).try_acquire_owned() else {
            return unavailable();
        };
        let store = Arc::clone(&state.store);
        match tokio::task::spawn_blocking(move || {
            let _slot = slot;
            store.revoke(&token)
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                error!(%error, "吊销会话失败");
                return unavailable();
            }
            Err(error) => {
                error!(%error, "吊销会话的任务失败");
                return unavailable();
            }
        }
    }
    logged_out_response(state.secure())
}

pub(crate) async fn session_probe() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

/// 本会话的 CSRF 令牌，由 [`require_api_session`] 放进请求扩展。
#[derive(Clone)]
struct SessionCsrf(String);

/// `GET /api/v1/session`：页面由此取得本会话的 CSRF 令牌。不缓存，不带任何跨源头。
async fn api_session(Extension(SessionCsrf(csrf)): Extension<SessionCsrf>) -> Response {
    no_store(Json(serde_json::json!({ "csrf_token": csrf })).into_response())
}

async fn api_not_found() -> Response {
    api_error(StatusCode::NOT_FOUND, "not_found", "没有这个接口")
}

fn api_error(status: StatusCode, code: &str, message: &str) -> Response {
    no_store(
        (status, Json(serde_json::json!({ "code": code, "message": message }))).into_response(),
    )
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn unavailable() -> Response {
    (StatusCode::SERVICE_UNAVAILABLE, "认证暂时不可用，请稍后再试").into_response()
}

fn is_api(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

/// 请求所带会话的状态。
enum SessionStatus {
    Missing,
    /// 带了会话 cookie，但它无效（过期、被吊销，或者是旧版本的 cookie）。
    Invalid,
    /// 会话校验名额已满，没有校验。
    Busy,
    Valid(SessionToken),
}

/// 校验请求所带的会话，有效时按需续期。认证库出错（含该续期却写不进）时返回错误，调用方
/// 拒绝请求（偏于关闭）。名额在派生阻塞任务之前取得，由任务持有到校验结束。
async fn session_status(state: &AppState, headers: &HeaderMap) -> Result<SessionStatus> {
    let Some(value) = extract_cookie(headers, SESSION_COOKIE) else {
        return Ok(SessionStatus::Missing);
    };
    let Some(token) = SessionToken::parse(value) else {
        return Ok(SessionStatus::Invalid);
    };
    let Ok(permit) = Arc::clone(&state.session_checks).try_acquire_owned() else {
        return Ok(SessionStatus::Busy);
    };
    let store = Arc::clone(&state.store);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        Ok(if store.validate(&token, now())? {
            SessionStatus::Valid(token)
        } else {
            SessionStatus::Invalid
        })
    })
    .await
    .context("校验会话的任务失败")?
}

fn csrf_matches(provided: Option<&[u8]>, expected: &str) -> bool {
    provided.is_some_and(|provided| bool::from(provided.ct_eq(expected.as_bytes())))
}

/// 每个写请求的 `Origin` 必须是信任的来源；缺少、为 `null` 或不一致时拒绝。
async fn require_trusted_origin(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let trusted = is_safe(request.method())
        || request
            .headers()
            .get(header::ORIGIN)
            .is_some_and(|origin| state.origins.trusts(origin.as_bytes()));
    if trusted {
        return next.run(request).await;
    }
    if is_api(request.uri().path()) {
        api_error(StatusCode::FORBIDDEN, "untrusted_origin", "请求来源不受信任")
    } else {
        (StatusCode::FORBIDDEN, "请求来源不受信任").into_response()
    }
}

/// 页面路由的会话检查：没有有效会话时跳转登录页。
pub(crate) async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let error = match session_status(&state, request.headers()).await {
        Ok(SessionStatus::Valid(_)) => return next.run(request).await,
        Ok(SessionStatus::Invalid) => Some("session_expired"),
        Ok(SessionStatus::Missing) => None,
        Ok(SessionStatus::Busy) => return unavailable(),
        Err(error) => {
            error!(%error, "校验会话失败");
            return unavailable();
        }
    };
    let target = sanitize_next(Some(
        request.uri().path_and_query().map(|value| value.as_str()).unwrap_or("/"),
    ));
    login_redirect(target, error)
}

/// `/api` 的会话检查：没有有效会话时返回 JSON 401；写请求还要带上本会话的 CSRF 令牌。
async fn require_api_session(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = match session_status(&state, request.headers()).await {
        Ok(SessionStatus::Valid(token)) => token,
        Ok(SessionStatus::Busy) => {
            return api_error(StatusCode::SERVICE_UNAVAILABLE, "busy", "服务器繁忙，请稍后再试");
        }
        Ok(SessionStatus::Missing | SessionStatus::Invalid) => {
            return api_error(StatusCode::UNAUTHORIZED, "unauthenticated", "未登录或登录已过期");
        }
        Err(error) => {
            error!(%error, "校验会话失败");
            return api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "auth_unavailable",
                "认证暂时不可用",
            );
        }
    };
    let csrf = token.csrf();
    if !is_safe(request.method())
        && !csrf_matches(request.headers().get(CSRF_HEADER).map(HeaderValue::as_bytes), &csrf)
    {
        return api_error(StatusCode::FORBIDDEN, "csrf_mismatch", "缺少或错误的 CSRF 令牌");
    }
    request.extensions_mut().insert(SessionCsrf(csrf));
    next.run(request).await
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
    use std::net::SocketAddr;

    use super::test_support::{sample_config, state_with};
    use super::*;
    use crate::auth::config::LoginRateLimit;
    use crate::auth::rate_limit::{MAX_TRACKED_LOGIN_KEYS, cleanup_rate_limit_entries};
    use crate::auth::session::{DEVICE_COOKIE, device_nonce};
    use time::Duration;

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

    const PEER: Option<std::net::IpAddr> =
        Some(std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, 7)));
    const KEY: [u8; 32] = [7; 32];

    #[test]
    fn rate_limit_key_uses_the_peer_address_unless_proxies_are_trusted() {
        let config = sample_config();
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.10"));
        headers.insert("x-real-ip", HeaderValue::from_static("198.51.100.2"));

        let key = rate_limit_key(&config, &KEY, &headers, PEER);

        assert_eq!(key, "client|192.0.2.7");
        assert_eq!(rate_limit_key(&config, &KEY, &headers, None), "client|unknown");
    }

    /// 可信代理把对端地址追加在最右边；左边的项由客户端填写，可以随意伪造（审计 F1）。
    #[test]
    fn trusted_forwarded_for_uses_the_address_the_proxy_appended() {
        let mut config = sample_config();
        config.trust_proxy_headers = true;
        let mut headers = HeaderMap::new();
        headers.append("x-forwarded-for", HeaderValue::from_static("1.1.1.1, 203.0.113.10"));
        headers.append("x-forwarded-for", HeaderValue::from_static("6.6.6.6, 198.51.100.2"));

        let key = rate_limit_key(&config, &KEY, &headers, PEER);

        assert_eq!(key, "client|198.51.100.2");
    }

    #[test]
    fn device_tokens_verify_only_with_the_issuing_key_and_username() {
        let token = build_device_token(&KEY, "demo");
        let nonce = device_nonce(&KEY, "demo", &token).expect("valid device token");
        assert!(!nonce.is_empty() && !nonce.contains('.'));

        assert_eq!(device_nonce(&[8; 32], "demo", &token), None);
        assert_eq!(device_nonce(&KEY, "someone", &token), None);
        assert_eq!(device_nonce(&KEY, "demo", &format!("{nonce}.AAAA")), None);
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
        let response = handle_login(State(state.clone()), peer, headers, form).await;
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
        let (state, _dir) = state_with(config);

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
        let (real, _real_dir) = state_with(config.clone());
        let (unknown, _unknown_dir) = state_with(config);
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
        let (state, _dir) = state_with(config);
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
        let (state, _dir) = state_with(sample_config());
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
        let (state, _dir) = state_with(sample_config());
        let (started, started_rx) = tokio::sync::oneshot::channel();
        let (release, release_rx) = std::sync::mpsc::channel::<()>();
        let request = tokio::spawn({
            let state = state.clone();
            async move {
                state
                    .check_and_record_with("client|192.0.2.7".into(), move || {
                        started.send(()).expect("the test waits for the check");
                        release_rx.recv().expect("the test releases the check");
                        Ok(None::<()>)
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
        let (state, _dir) = state_with(sample_config());
        let _busy = Arc::clone(&state.credential_checks)
            .try_acquire_many_owned(MAX_CONCURRENT_CREDENTIAL_CHECKS as u32)
            .expect("all general slots");
        let ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut check = std::pin::pin!(state.check_and_record_with("client|192.0.2.7".into(), {
            let ran = Arc::clone(&ran);
            move || {
                ran.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(None::<()>)
            }
        }));

        let first =
            std::future::poll_fn(|cx| std::task::Poll::Ready(check.as_mut().poll(cx))).await;
        assert!(matches!(first, std::task::Poll::Ready(CredentialCheck::Busy)));
        assert!(!ran.load(std::sync::atomic::Ordering::SeqCst));
        assert!(state.login_throttle.lock().await.is_empty(), "nothing is recorded");
    }

    /// 认证库出错时没有校验任何密码：不记失败，提示稍后再试。
    #[tokio::test]
    async fn an_unreadable_credential_is_not_a_failed_attempt() {
        let (state, _dir) = state_with(sample_config());
        let check = state
            .check_and_record_with("client|192.0.2.7".into(), || {
                Err::<Option<()>, _>(anyhow::anyhow!("database is locked"))
            })
            .await;
        assert!(matches!(check, CredentialCheck::Unavailable));
        assert!(state.login_throttle.lock().await.is_empty());
    }

    const WATCHDOG: std::time::Duration = std::time::Duration::from_secs(10);

    async fn within<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::time::timeout(WATCHDOG, future).await.expect("watchdog")
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
