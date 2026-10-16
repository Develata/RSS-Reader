//! The reader's HTTP API (v0.2 phase C, C3): every [`Backend`] operation under `/api/v1`
//! ([`rssr_api::protocol`]), answered from the server's own library. Five network-heavy
//! operations run as background jobs ([`crate::jobs`]): the request starts the job, the client
//! polls it.
//!
//! [`routes`] goes behind the session check of [`crate::auth::with_auth`], which also checks the
//! Origin and the CSRF token of every write. Here:
//! - a write must carry this build's identifier ([`protocol::BUILD_HEADER`]): a page of another
//!   build is refused before anything runs; its reads are answered;
//! - every request gets an id ([`protocol::REQUEST_ID_HEADER`]), which an error carries and the
//!   log records;
//! - bodies and structured reads' parameters are bounded (the limits in [`protocol`]);
//! - at most [`MAX_OPERATIONS`] operations run at once, and [`MAX_IMPORTS`] import; more are
//!   refused at once, never queued. An operation runs in its own task that holds its slot until
//!   it ends, so a client that goes away neither cancels a write halfway nor frees the slot early.
//!   Jobs have their own bounds ([`crate::jobs`]); a poll takes no slot (it only waits);
//! - every answer is JSON and is not cached.

use std::{future::Future, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        DefaultBodyLimit, FromRequestParts, Path, RawQuery, Request, State,
        rejection::{BytesRejection, PathRejection},
    },
    http::{HeaderValue, StatusCode, header, request::Parts},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use rand_core::{OsRng, RngCore};
use rssr_api::{
    Backend, BackendError, ErrorCode,
    protocol::{
        self, AddSubscription, ImportConfigJson, ImportOpml, JobCreated, JobId, JobResult,
        RemoteConfig, SetRead, SetStarred, routes,
    },
};
use rssr_application::{EntriesBootstrapInput, SettingsPatch};
use rssr_domain::{EntriesWorkspaceState, EntryQuery, MarkReadPreview};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{error, info, warn};

use crate::{auth::is_safe, jobs::Jobs};

/// This build's identifier (`build-support/build_id.rs`).
pub(crate) const BUILD_ID: &str = env!("RSSR_BUILD_ID");
/// Operations running at once; each holds store work or network work.
const MAX_OPERATIONS: usize = 32;
/// Imports running at once; each parses a document of up to 4 MiB.
const MAX_IMPORTS: usize = 1;
/// The reader's image localization may take up to 245 s (24 images of 10 s, plus 5 s); a request
/// gives up after this.
const LOCALIZE_LIMIT: Duration = Duration::from_secs(60);

pub(crate) type SharedBackend = Arc<dyn Backend + Send + Sync>;

#[derive(Clone)]
pub(crate) struct ApiState {
    backend: SharedBackend,
    build_id: Arc<str>,
    operations: Arc<Semaphore>,
    imports: Arc<Semaphore>,
    localize_limit: Duration,
    jobs: Jobs,
}

impl ApiState {
    pub(crate) fn new(backend: SharedBackend) -> Self {
        Self::with(backend, BUILD_ID, LOCALIZE_LIMIT)
    }

    pub(crate) fn with(backend: SharedBackend, build_id: &str, localize_limit: Duration) -> Self {
        Self {
            backend,
            build_id: Arc::from(build_id),
            operations: Arc::new(Semaphore::new(MAX_OPERATIONS)),
            imports: Arc::new(Semaphore::new(MAX_IMPORTS)),
            localize_limit,
            jobs: Jobs::new(),
        }
    }

    /// Test only: takes every operation slot.
    #[cfg(test)]
    pub(crate) fn exhaust_operations(&self) -> OwnedSemaphorePermit {
        Arc::clone(&self.operations)
            .try_acquire_many_owned(MAX_OPERATIONS as u32)
            .expect("all operation slots")
    }

    /// Test only: admits jobs that never finish until no more are admitted.
    #[cfg(test)]
    pub(crate) fn fill_jobs(&self) {
        self.jobs.fill();
    }

    /// Test only: takes the import slot.
    #[cfg(test)]
    pub(crate) fn exhaust_imports(&self) -> OwnedSemaphorePermit {
        Arc::clone(&self.imports).try_acquire_many_owned(MAX_IMPORTS as u32).expect("import slot")
    }
}

/// The application's routes (every synchronous [`Backend`] operation), for
/// [`crate::auth::with_auth`] to put behind the session check.
pub(crate) fn routes<S: Clone + Send + Sync + 'static>(state: ApiState) -> Router<S> {
    let imports = DefaultBodyLimit::max(protocol::MAX_IMPORT_BODY_BYTES);
    Router::new()
        .route(
            routes::SETTINGS,
            get(load_settings)
                .patch(update_settings)
                .layer(DefaultBodyLimit::max(protocol::MAX_SETTINGS_BODY_BYTES)),
        )
        .route(routes::STARTUP_TARGET, get(resolve_startup_target))
        .route(routes::ENTRIES_BOOTSTRAP, get(bootstrap_entries))
        .route(routes::OPENED_FEED, put(remember_opened_feed))
        .route(routes::WORKSPACE, put(save_entries_workspace))
        .route(routes::ENTRIES, get(list_entries))
        .route(routes::ENTRY, get(load_reader_entry))
        .route(routes::ENTRY_READ, put(set_entry_read))
        .route(routes::ENTRY_STARRED, put(set_entry_starred))
        .route(routes::ENTRY_ASSETS, post(localize_entry_assets))
        .route(
            routes::MARK_READ,
            get(preview_mark_read)
                .post(mark_read_if_unchanged)
                .layer(DefaultBodyLimit::max(protocol::MAX_MARK_READ_BODY_BYTES)),
        )
        .route(routes::FEEDS, get(load_feeds_snapshot))
        .route(routes::FEED, delete(remove_feed))
        .route(routes::CONFIG_JSON, get(export_config_json).put(import_config_json).layer(imports))
        .route(routes::CONFIG_OPML, get(export_opml).post(import_opml).layer(imports))
        .route(routes::JOB_REFRESH_ALL, post(start_refresh_all))
        .route(routes::JOB_REFRESH_FEED, post(start_refresh_feed))
        .route(routes::JOB_ADD_SUBSCRIPTION, post(start_add_subscription))
        .route(routes::JOB_REMOTE_PUSH, post(start_remote_push))
        .route(routes::JOB_REMOTE_PULL, post(start_remote_pull))
        .route(routes::JOB, get(poll_job))
        // Outside the routes' own limits, which take precedence.
        .route_layer(DefaultBodyLimit::max(protocol::MAX_BODY_BYTES))
        .route_layer(middleware::from_fn_with_state(state.clone(), api_context))
        .with_state(state)
}

/// An error answer under `/api`: the code's HTTP status, the error as JSON, not cached.
pub(crate) fn error_response(error: BackendError) -> Response {
    let status =
        StatusCode::from_u16(error.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response = (status, Json(error)).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// A request's id: 64 random bits as 16 hex digits.
#[derive(Clone)]
struct RequestId(Arc<str>);

impl RequestId {
    fn new() -> Self {
        let mut bytes = [0_u8; 8];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>().into())
    }
}

/// Every answer of the application's routes: a request id, writes only from this build, not
/// cached.
async fn api_context(State(state): State<ApiState>, mut request: Request, next: Next) -> Response {
    let id = RequestId::new();
    let same_build = request
        .headers()
        .get(protocol::BUILD_HEADER)
        .is_some_and(|value| value.as_bytes() == state.build_id.as_bytes());
    let mut response = if is_safe(request.method()) || same_build {
        request.extensions_mut().insert(id.clone());
        next.run(request).await
    } else {
        let message = "页面与服务器的版本不一致，请重新加载页面";
        failure(BackendError::new(ErrorCode::BuildMismatch, message), &id)
    };
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Ok(value) = HeaderValue::from_str(&id.0) {
        headers.insert(protocol::REQUEST_ID_HEADER, value);
    }
    response
}

/// An error answer carrying the request's id; what the user cannot act on is logged as a warning.
fn failure(error: BackendError, id: &RequestId) -> Response {
    let code = error.code.as_str();
    match error.code {
        ErrorCode::Storage | ErrorCode::Internal | ErrorCode::Unavailable => {
            warn!(request_id = %id.0, code, message = %error.message, "请求失败");
        }
        _ => info!(request_id = %id.0, code, message = %error.message, "请求被拒绝"),
    }
    error_response(error.with_request_id(id.0.as_ref()))
}

fn busy() -> BackendError {
    BackendError::new(ErrorCode::Unavailable, "服务器繁忙，请稍后再试")
}

fn internal() -> BackendError {
    BackendError::new(ErrorCode::Internal, "服务器内部错误")
}

/// What every handler has: the state and the request's id.
struct Ctx {
    state: ApiState,
    id: RequestId,
}

impl FromRequestParts<ApiState> for Ctx {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &ApiState) -> Result<Self, Response> {
        // `api_context` puts it there; a handler reached another way still gets an id.
        let id = parts.extensions.get::<RequestId>().cloned().unwrap_or_else(RequestId::new);
        Ok(Self { state: state.clone(), id })
    }
}

/// A handler's error, answered with the request's id.
struct Failure {
    error: BackendError,
    id: RequestId,
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        failure(self.error, &self.id)
    }
}

impl Ctx {
    fn fail(&self, error: BackendError) -> Failure {
        Failure { error, id: self.id.clone() }
    }

    fn invalid(&self, message: impl Into<String>) -> Failure {
        self.fail(BackendError::new(ErrorCode::InvalidInput, message))
    }

    /// The id in the path.
    fn path_id(&self, path: PathParam) -> Result<i64, Failure> {
        path.ok()
            .and_then(|Path(raw)| raw.parse().ok())
            .ok_or_else(|| self.invalid("路径中的 id 无效"))
    }

    /// The JSON in the query parameter `name` of a structured read.
    fn query<T: DeserializeOwned>(&self, raw: Option<String>, name: &str) -> Result<T, Failure> {
        let raw = raw.unwrap_or_default();
        if raw.len() > protocol::MAX_QUERY_PARAMETER_BYTES {
            return Err(self.fail(BackendError::new(ErrorCode::TooLarge, "查询参数过长")));
        }
        let value = url::form_urlencoded::parse(raw.as_bytes())
            .find_map(|(key, value)| (key == name).then_some(value))
            .ok_or_else(|| self.invalid(format!("缺少查询参数 {name}")))?;
        serde_json::from_str(&value)
            .map_err(|error| self.invalid(format!("查询参数 {name} 无效：{error}")))
    }

    /// The JSON body of a write.
    fn body<T: DeserializeOwned>(&self, body: Body) -> Result<T, Failure> {
        let body = body.map_err(|rejection| {
            if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
                self.fail(BackendError::new(ErrorCode::TooLarge, "请求体过大"))
            } else {
                self.invalid("无法读取请求体")
            }
        })?;
        serde_json::from_slice(&body).map_err(|error| self.invalid(format!("请求体无效：{error}")))
    }

    /// The import slot, or `unavailable` at once.
    fn import_slot(&self) -> Result<OwnedSemaphorePermit, Failure> {
        Arc::clone(&self.state.imports).try_acquire_owned().map_err(|_| self.fail(busy()))
    }

    /// Starts `operation` as a background job: `202` and its id, or `unavailable` when no more
    /// jobs are admitted. The operation's future is only made here; it runs once admitted.
    fn start<F, Fut>(self, operation: F) -> Response
    where
        F: FnOnce(SharedBackend) -> Fut,
        Fut: Future<Output = rssr_api::Result<JobResult>> + Send + 'static,
    {
        let job = operation(Arc::clone(&self.state.backend));
        match self.state.jobs.submit(&self.id.0, job) {
            Ok(job_id) => (StatusCode::ACCEPTED, Json(JobCreated { job_id })).into_response(),
            Err(error) => failure(error, &self.id),
        }
    }

    /// Runs `operation` in its own task holding an operation slot (and `import`, an import's
    /// slot) until it ends, and answers its result.
    async fn run<T, F, Fut>(self, import: Option<OwnedSemaphorePermit>, operation: F) -> Response
    where
        T: Serialize + Send + 'static,
        F: FnOnce(SharedBackend) -> Fut + Send + 'static,
        Fut: Future<Output = rssr_api::Result<T>> + Send + 'static,
    {
        let Ok(slot) = Arc::clone(&self.state.operations).try_acquire_owned() else {
            return failure(busy(), &self.id);
        };
        let backend = Arc::clone(&self.state.backend);
        let task = tokio::spawn(async move {
            let _slots = (slot, import);
            operation(backend).await
        });
        match task.await {
            Ok(Ok(value)) => match serde_json::to_vec(&value) {
                Ok(body) => ([(header::CONTENT_TYPE, "application/json")], body).into_response(),
                Err(error) => {
                    error!(request_id = %self.id.0, %error, "无法编码结果");
                    failure(internal(), &self.id)
                }
            },
            Ok(Err(error)) => failure(error, &self.id),
            Err(error) => {
                error!(request_id = %self.id.0, %error, "操作的任务异常结束");
                failure(internal(), &self.id)
            }
        }
    }
}

type Answer = Result<Response, Failure>;
type Body = Result<Bytes, BytesRejection>;
type PathParam = Result<Path<String>, PathRejection>;

async fn load_settings(ctx: Ctx) -> Response {
    ctx.run(None, |backend| async move { backend.load_settings().await }).await
}

async fn update_settings(ctx: Ctx, body: Body) -> Answer {
    let patch: SettingsPatch = ctx.body(body)?;
    Ok(ctx.run(None, move |backend| async move { backend.update_settings(patch).await }).await)
}

async fn resolve_startup_target(ctx: Ctx) -> Response {
    ctx.run(None, |backend| async move { backend.resolve_startup_target().await }).await
}

async fn bootstrap_entries(ctx: Ctx, RawQuery(raw): RawQuery) -> Answer {
    let input: EntriesBootstrapInput = ctx.query(raw, "input")?;
    Ok(ctx.run(None, move |backend| async move { backend.bootstrap_entries(input).await }).await)
}

async fn remember_opened_feed(ctx: Ctx, path: PathParam) -> Answer {
    let feed_id = ctx.path_id(path)?;
    Ok(ctx
        .run(None, move |backend| async move { backend.remember_opened_feed(feed_id).await })
        .await)
}

async fn save_entries_workspace(ctx: Ctx, body: Body) -> Answer {
    let workspace: EntriesWorkspaceState = ctx.body(body)?;
    Ok(ctx
        .run(None, move |backend| async move {
            backend.save_entries_workspace_if_changed(workspace).await
        })
        .await)
}

async fn list_entries(ctx: Ctx, RawQuery(raw): RawQuery) -> Answer {
    let query: EntryQuery = ctx.query(raw, "query")?;
    Ok(ctx.run(None, move |backend| async move { backend.list_entries(&query).await }).await)
}

async fn load_reader_entry(ctx: Ctx, path: PathParam) -> Answer {
    let entry_id = ctx.path_id(path)?;
    Ok(ctx.run(None, move |backend| async move { backend.load_reader_entry(entry_id).await }).await)
}

async fn set_entry_read(ctx: Ctx, path: PathParam, body: Body) -> Answer {
    let entry_id = ctx.path_id(path)?;
    let SetRead { read } = ctx.body(body)?;
    Ok(ctx
        .run(None, move |backend| async move { backend.set_entry_read(entry_id, read).await })
        .await)
}

async fn set_entry_starred(ctx: Ctx, path: PathParam, body: Body) -> Answer {
    let entry_id = ctx.path_id(path)?;
    let SetStarred { starred } = ctx.body(body)?;
    Ok(ctx
        .run(None, move |backend| async move { backend.set_entry_starred(entry_id, starred).await })
        .await)
}

/// The reader's image localization, given up after the state's limit (`unavailable`). Giving up
/// cancels the downloads; the body is written once at the end, compare-and-set, so it is either
/// the original or the localized one, never a mixture.
async fn localize_entry_assets(ctx: Ctx, path: PathParam) -> Answer {
    let entry_id = ctx.path_id(path)?;
    let limit = ctx.state.localize_limit;
    Ok(ctx
        .run(None, move |backend| async move {
            tokio::time::timeout(limit, backend.localize_entry_assets(entry_id))
                .await
                .unwrap_or_else(|_| {
                    Err(BackendError::new(ErrorCode::Unavailable, "图片本地化超过时限，已取消"))
                })
        })
        .await)
}

async fn preview_mark_read(ctx: Ctx, RawQuery(raw): RawQuery) -> Answer {
    let query: EntryQuery = ctx.query(raw, "query")?;
    Ok(ctx.run(None, move |backend| async move { backend.preview_mark_read(&query).await }).await)
}

async fn mark_read_if_unchanged(ctx: Ctx, body: Body) -> Answer {
    let preview: MarkReadPreview = ctx.body(body)?;
    Ok(ctx
        .run(None, move |backend| async move { backend.mark_read_if_unchanged(&preview).await })
        .await)
}

async fn load_feeds_snapshot(ctx: Ctx) -> Response {
    ctx.run(None, |backend| async move { backend.load_feeds_snapshot().await }).await
}

async fn remove_feed(ctx: Ctx, path: PathParam) -> Answer {
    let feed_id = ctx.path_id(path)?;
    Ok(ctx.run(None, move |backend| async move { backend.remove_feed(feed_id).await }).await)
}

async fn export_config_json(ctx: Ctx) -> Response {
    ctx.run(None, |backend| async move { backend.export_config_json().await }).await
}

async fn import_config_json(ctx: Ctx, body: Body) -> Answer {
    let import = ctx.import_slot()?;
    let ImportConfigJson { raw, expected } = ctx.body(body)?;
    Ok(ctx
        .run(Some(import), move |backend| async move {
            backend.import_config_json(&raw, &expected).await
        })
        .await)
}

async fn export_opml(ctx: Ctx) -> Response {
    ctx.run(None, |backend| async move { backend.export_opml().await }).await
}

async fn import_opml(ctx: Ctx, body: Body) -> Answer {
    let import = ctx.import_slot()?;
    let ImportOpml { raw } = ctx.body(body)?;
    Ok(ctx.run(Some(import), move |backend| async move { backend.import_opml(&raw).await }).await)
}

async fn start_refresh_all(ctx: Ctx) -> Response {
    ctx.start(|backend| async move { Ok(JobResult::RefreshAll(backend.refresh_all().await?)) })
}

async fn start_refresh_feed(ctx: Ctx, path: PathParam) -> Answer {
    let feed_id = ctx.path_id(path)?;
    Ok(ctx.start(move |backend| async move {
        Ok(JobResult::RefreshFeed(backend.refresh_feed(feed_id).await?))
    }))
}

async fn start_add_subscription(ctx: Ctx, body: Body) -> Answer {
    let AddSubscription { raw_url, fallback_site_url } = ctx.body(body)?;
    Ok(ctx.start(move |backend| async move {
        let outcome = backend.add_subscription(&raw_url, fallback_site_url).await?;
        Ok(JobResult::AddSubscription(outcome))
    }))
}

async fn start_remote_push(ctx: Ctx, body: Body) -> Answer {
    let RemoteConfig { endpoint, remote_path } = ctx.body(body)?;
    Ok(ctx.start(move |backend| async move {
        let outcome = backend.push_remote_config(&endpoint, &remote_path).await?;
        Ok(JobResult::RemoteConfigPush(outcome))
    }))
}

async fn start_remote_pull(ctx: Ctx, body: Body) -> Answer {
    let RemoteConfig { endpoint, remote_path } = ctx.body(body)?;
    Ok(ctx.start(move |backend| async move {
        let outcome = backend.pull_remote_config(&endpoint, &remote_path).await?;
        Ok(JobResult::RemoteConfigPull(outcome))
    }))
}

/// A job's status, after waiting up to `wait_ms` (a plain integer, at most
/// `protocol::MAX_JOB_WAIT`) for it to finish. An id that is malformed, unknown, expired,
/// evicted or from before a restart is `status_unavailable`.
async fn poll_job(ctx: Ctx, path: PathParam, RawQuery(raw): RawQuery) -> Answer {
    let raw = raw.unwrap_or_default();
    let wait_ms = match url::form_urlencoded::parse(raw.as_bytes())
        .find_map(|(key, value)| (key == "wait_ms").then_some(value))
    {
        None => 0,
        Some(value) => value.parse::<u64>().map_err(|_| ctx.invalid("wait_ms 必须是非负整数"))?,
    };
    let id = path.ok().and_then(|Path(raw)| JobId::parse(&raw));
    let status = match id {
        Some(id) => ctx.state.jobs.status(&id, Duration::from_millis(wait_ms)).await,
        None => None,
    };
    let status = status.ok_or_else(|| {
        ctx.fail(BackendError::new(
            ErrorCode::StatusUnavailable,
            "任务的状态已不可用（已过期、被淘汰或服务器重启过），请重新读取相关数据",
        ))
    })?;
    Ok(([(header::CONTENT_TYPE, "application/json")], status).into_response())
}
