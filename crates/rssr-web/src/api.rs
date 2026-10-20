//! The reader's HTTP API (v0.2 phase C, C3): every [`Backend`] operation under `/api/v1`
//! ([`rssr_api::protocol`]), answered from the server's own library. Five network-heavy
//! operations run as background jobs ([`crate::jobs`]): the request starts the job, the client
//! polls it.
//!
//! [`routes`] goes behind the session check of [`crate::auth::with_auth`], which also checks the
//! Origin and the CSRF token of every write. Here:
//! - every request under `/api` gets an id ([`protocol::REQUEST_ID_HEADER`],
//!   [`assign_request_id`]), which every error carries, the authentication's included;
//! - a write must carry this build's identifier ([`protocol::BUILD_HEADER`]): a page of another
//!   build is refused before anything runs; its reads are answered;
//! - at most [`MAX_OPERATIONS`] operations run at once, and [`MAX_IMPORTS`] import; more are
//!   refused at once, never queued. The slots are taken before the body is read, so bodies being
//!   received are bounded too; a body is received within [`RECEIVE_LIMIT`]. An operation runs in
//!   its own task that holds its slots until it ends, so a client that goes away neither cancels
//!   a write halfway nor frees the slots early. Jobs have their own bounds ([`crate::jobs`]); a
//!   poll takes no operation slot (it only waits) but one of [`MAX_POLLS`];
//! - bodies and structured reads' parameters are bounded (the limits in [`protocol`]); a write
//!   without a body refuses one;
//! - every answer is JSON and is not cached; the log records an error's request id and code,
//!   never its message (it may quote a subscription's address with its credentials).

use std::{future::Future, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        DefaultBodyLimit, FromRequest, FromRequestParts, Path, RawQuery, Request, State,
        rejection::PathRejection,
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
use rssr_application::{EntriesBootstrapInput, MAX_MESSAGE_BYTES, SettingsPatch, truncate_marked};
use rssr_domain::{EntriesWorkspaceState, EntryQuery, MarkReadPreview};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{error, info, warn};

use crate::{
    auth::{is_api, is_safe},
    jobs::Jobs,
};

/// This build's identifier (`build-support/build_id.rs`).
pub(crate) const BUILD_ID: &str = env!("RSSR_BUILD_ID");
/// Operations running at once; each holds store work or network work.
const MAX_OPERATIONS: usize = 32;
/// Imports running at once; each parses a document of up to 4 MiB.
const MAX_IMPORTS: usize = 1;
/// Polls of jobs waiting at once.
const MAX_POLLS: usize = 64;
/// The reader's image localization may take up to 245 s (24 images of 10 s, plus 5 s); a request
/// gives up after this.
const LOCALIZE_LIMIT: Duration = Duration::from_secs(60);
/// How long receiving a body may take once its slots are taken (4 MiB at about 0.5 Mbit/s).
const RECEIVE_LIMIT: Duration = Duration::from_secs(60);

pub(crate) type SharedBackend = Arc<dyn Backend + Send + Sync>;

#[derive(Clone)]
pub(crate) struct ApiState {
    backend: SharedBackend,
    build_id: Arc<str>,
    operations: Arc<Semaphore>,
    imports: Arc<Semaphore>,
    polls: Arc<Semaphore>,
    localize_limit: Duration,
    receive_limit: Duration,
    answer_limit: usize,
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
            polls: Arc::new(Semaphore::new(MAX_POLLS)),
            localize_limit,
            receive_limit: RECEIVE_LIMIT,
            answer_limit: protocol::MAX_ANSWER_BYTES,
            jobs: Jobs::new(),
        }
    }

    /// Test only: a shorter time to receive a body.
    #[cfg(test)]
    pub(crate) fn receiving_within(mut self, limit: Duration) -> Self {
        self.receive_limit = limit;
        self
    }

    /// Test only: a smaller bound on the answers sent.
    #[cfg(test)]
    pub(crate) fn answering_within(mut self, limit: usize) -> Self {
        self.answer_limit = limit;
        self
    }

    /// Test only: takes every operation slot.
    #[cfg(test)]
    pub(crate) fn exhaust_operations(&self) -> OwnedSemaphorePermit {
        Arc::clone(&self.operations)
            .try_acquire_many_owned(MAX_OPERATIONS as u32)
            .expect("all operation slots")
    }

    /// Test only: takes every poll slot.
    #[cfg(test)]
    pub(crate) fn exhaust_polls(&self) -> OwnedSemaphorePermit {
        Arc::clone(&self.polls).try_acquire_many_owned(MAX_POLLS as u32).expect("all poll slots")
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

/// The application's routes (every [`Backend`] operation), for [`crate::auth::with_auth`] to put
/// behind the session check. A route's own body limit takes precedence over the default.
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
        .route_layer(DefaultBodyLimit::max(protocol::MAX_BODY_BYTES))
        .route_layer(middleware::from_fn_with_state(state.clone(), api_context))
        .with_state(state)
}

/// A request's id: 64 random bits as 16 hex digits.
#[derive(Clone)]
pub(crate) struct RequestId(Arc<str>);

impl RequestId {
    fn new() -> Self {
        let mut bytes = [0_u8; 8];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>().into())
    }
}

/// Every request under `/api` gets an id, before anything can refuse it: errors carry it in
/// their body ([`failure`]) and every answer in [`protocol::REQUEST_ID_HEADER`] (review C3-R1
/// F14). The outermost layer of the application.
pub(crate) async fn assign_request_id(mut request: Request, next: Next) -> Response {
    if !is_api(request.uri().path()) {
        return next.run(request).await;
    }
    let id = RequestId::new();
    request.extensions_mut().insert(id.clone());
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&id.0) {
        response.headers_mut().insert(protocol::REQUEST_ID_HEADER, value);
    }
    response
}

/// An error answer under `/api`: the code's HTTP status, the error as JSON with the request's
/// id, not cached; its message shortened to [`MAX_MESSAGE_BYTES`], so the answer fits
/// [`protocol::MAX_SMALL_ANSWER_BYTES`] (review C4A-R2-02). The log records the id and the code;
/// never the message, which may quote a subscription's address with its credentials (review
/// C3-R1 F05).
pub(crate) fn failure(error: BackendError, id: Option<&RequestId>) -> Response {
    let error =
        BackendError { message: truncate_marked(error.message, MAX_MESSAGE_BYTES), ..error };
    let request_id = id.map_or("-", |id| &id.0);
    let code = error.code.as_str();
    match error.code {
        ErrorCode::Storage
        | ErrorCode::Internal
        | ErrorCode::Unavailable
        | ErrorCode::OutcomeUnknown => {
            warn!(request_id, code, "请求失败");
        }
        _ => info!(request_id, code, "请求被拒绝"),
    }
    let error = match id {
        Some(id) => error.with_request_id(id.0.as_ref()),
        None => error,
    };
    let status =
        StatusCode::from_u16(error.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response = (status, Json(error)).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Every answer of the application's routes: writes only from this build, not cached.
async fn api_context(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let same_build = request
        .headers()
        .get(protocol::BUILD_HEADER)
        .is_some_and(|value| value.as_bytes() == state.build_id.as_bytes());
    let mut response = if is_safe(request.method()) || same_build {
        next.run(request).await
    } else {
        let message = "页面与服务器的版本不一致，请重新加载页面";
        failure(
            BackendError::new(ErrorCode::BuildMismatch, message),
            request.extensions().get::<RequestId>(),
        )
    };
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn busy() -> BackendError {
    BackendError::new(ErrorCode::Unavailable, "服务器繁忙，请稍后再试")
}

fn internal() -> BackendError {
    BackendError::new(ErrorCode::Internal, "服务器内部错误")
}

/// What every handler has: the state, the request's id and whether it is a write.
struct Ctx {
    state: ApiState,
    id: RequestId,
    write: bool,
}

impl FromRequestParts<ApiState> for Ctx {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &ApiState) -> Result<Self, Response> {
        // `assign_request_id` puts it there; a handler reached another way still gets an id.
        let id = parts.extensions.get::<RequestId>().cloned().unwrap_or_else(RequestId::new);
        Ok(Self { state: state.clone(), id, write: !is_safe(&parts.method) })
    }
}

/// A handler's error, answered with the request's id.
struct Failure {
    error: BackendError,
    id: RequestId,
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        failure(self.error, Some(&self.id))
    }
}

/// The slots an operation holds until it ends.
struct Slots {
    _operation: OwnedSemaphorePermit,
    _import: Option<OwnedSemaphorePermit>,
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

    /// The value of the query parameter `name`, which appears at most once.
    fn parameter(&self, raw: Option<&str>, name: &str) -> Result<Option<String>, Failure> {
        let raw = raw.unwrap_or_default();
        if raw.len() > protocol::MAX_QUERY_PARAMETER_BYTES {
            return Err(self.fail(BackendError::new(ErrorCode::TooLarge, "查询参数过长")));
        }
        let mut values = url::form_urlencoded::parse(raw.as_bytes())
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned());
        let value = values.next();
        if values.next().is_some() {
            return Err(self.invalid(format!("查询参数 {name} 重复")));
        }
        Ok(value)
    }

    /// The JSON in the query parameter `name` of a structured read.
    fn query<T: DeserializeOwned>(&self, raw: Option<String>, name: &str) -> Result<T, Failure> {
        let value = self
            .parameter(raw.as_deref(), name)?
            .ok_or_else(|| self.invalid(format!("缺少查询参数 {name}")))?;
        serde_json::from_str(&value)
            .map_err(|error| self.invalid(format!("查询参数 {name} 无效：{error}")))
    }

    /// An operation slot, or `unavailable` at once.
    fn admit(&self) -> Result<Slots, Failure> {
        let operation = Arc::clone(&self.state.operations)
            .try_acquire_owned()
            .map_err(|_| self.fail(busy()))?;
        Ok(Slots { _operation: operation, _import: None })
    }

    /// The import slot and an operation slot, or `unavailable` at once.
    fn admit_import(&self) -> Result<Slots, Failure> {
        let import =
            Arc::clone(&self.state.imports).try_acquire_owned().map_err(|_| self.fail(busy()))?;
        let slots = self.admit()?;
        Ok(Slots { _import: Some(import), ..slots })
    }

    /// The body of `request`, bounded by its route's limit and received within the state's
    /// limit. Called with the request's slots held, so the bodies being received are as many as
    /// the slots (review C3-R1 F03).
    async fn receive(&self, request: Request) -> Result<Bytes, Failure> {
        match tokio::time::timeout(self.state.receive_limit, Bytes::from_request(request, &()))
            .await
        {
            Ok(Ok(body)) => Ok(body),
            Ok(Err(rejection)) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
                Err(self.fail(BackendError::new(ErrorCode::TooLarge, "请求体过大")))
            }
            Ok(Err(_)) => Err(self.invalid("无法读取请求体")),
            Err(_) => Err(self.invalid("接收请求体超时")),
        }
    }

    /// The JSON body of a write.
    async fn json<T: DeserializeOwned>(&self, request: Request) -> Result<T, Failure> {
        let body = self.receive(request).await?;
        serde_json::from_slice(&body).map_err(|error| self.invalid(format!("请求体无效：{error}")))
    }

    /// A write that takes no body refuses one (review C3-R1 F13).
    async fn no_body(&self, request: Request) -> Result<(), Failure> {
        let body = self.receive(request).await?;
        if body.is_empty() { Ok(()) } else { Err(self.invalid("此接口不接受请求体")) }
    }

    /// A job's input, received under an operation slot that is released before the job is
    /// submitted (jobs have their own admission).
    async fn job_input<T: DeserializeOwned>(&self, request: Request) -> Result<T, Failure> {
        let _slots = self.admit()?;
        self.json(request).await
    }

    /// A job that takes no input refuses a body, received under an operation slot.
    async fn job_without_input(&self, request: Request) -> Result<(), Failure> {
        let _slots = self.admit()?;
        self.no_body(request).await
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
            Err(error) => failure(error, Some(&self.id)),
        }
    }

    /// A result over the answers' bound is not sent (review C4A-R2-02): a read says so; a write
    /// has run, and the page cannot see what it changed.
    fn too_large(&self) -> BackendError {
        let mib = self.state.answer_limit / (1024 * 1024);
        if self.write {
            let message =
                format!("操作已执行，但它的结果超过网页端能接收的 {mib} MiB，无法返回，请刷新查看");
            BackendError::new(ErrorCode::OutcomeUnknown, message)
        } else {
            let message = format!("结果超过网页端能接收的 {mib} MiB，无法在网页端打开");
            BackendError::new(ErrorCode::TooLarge, message)
        }
    }

    /// Runs `operation` in its own task holding `slots` until it ends, and answers its result.
    async fn run<T, F, Fut>(self, slots: Slots, operation: F) -> Response
    where
        T: Serialize + Send + 'static,
        F: FnOnce(SharedBackend) -> Fut + Send + 'static,
        Fut: Future<Output = rssr_api::Result<T>> + Send + 'static,
    {
        let backend = Arc::clone(&self.state.backend);
        let task = tokio::spawn(async move {
            let _slots = slots;
            operation(backend).await
        });
        match task.await {
            Ok(Ok(value)) => match serde_json::to_vec(&value) {
                Ok(body) if body.len() > self.state.answer_limit => {
                    failure(self.too_large(), Some(&self.id))
                }
                Ok(body) => ([(header::CONTENT_TYPE, "application/json")], body).into_response(),
                Err(error) => {
                    error!(request_id = %self.id.0, %error, "无法编码结果");
                    failure(internal(), Some(&self.id))
                }
            },
            Ok(Err(error)) => failure(error, Some(&self.id)),
            Err(error) => {
                error!(request_id = %self.id.0, %error, "操作的任务异常结束");
                failure(internal(), Some(&self.id))
            }
        }
    }
}

type Answer = Result<Response, Failure>;
type PathParam = Result<Path<String>, PathRejection>;

async fn load_settings(ctx: Ctx) -> Answer {
    let slots = ctx.admit()?;
    Ok(ctx.run(slots, |backend| async move { backend.load_settings().await }).await)
}

async fn update_settings(ctx: Ctx, request: Request) -> Answer {
    let slots = ctx.admit()?;
    let patch: SettingsPatch = ctx.json(request).await?;
    Ok(ctx.run(slots, move |backend| async move { backend.update_settings(patch).await }).await)
}

async fn resolve_startup_target(ctx: Ctx) -> Answer {
    let slots = ctx.admit()?;
    Ok(ctx.run(slots, |backend| async move { backend.resolve_startup_target().await }).await)
}

async fn bootstrap_entries(ctx: Ctx, RawQuery(raw): RawQuery) -> Answer {
    let input: EntriesBootstrapInput = ctx.query(raw, "input")?;
    let slots = ctx.admit()?;
    Ok(ctx.run(slots, move |backend| async move { backend.bootstrap_entries(input).await }).await)
}

async fn remember_opened_feed(ctx: Ctx, path: PathParam, request: Request) -> Answer {
    let feed_id = ctx.path_id(path)?;
    let slots = ctx.admit()?;
    ctx.no_body(request).await?;
    Ok(ctx
        .run(slots, move |backend| async move { backend.remember_opened_feed(feed_id).await })
        .await)
}

async fn save_entries_workspace(ctx: Ctx, request: Request) -> Answer {
    let slots = ctx.admit()?;
    let workspace: EntriesWorkspaceState = ctx.json(request).await?;
    Ok(ctx
        .run(slots, move |backend| async move {
            backend.save_entries_workspace_if_changed(workspace).await
        })
        .await)
}

async fn list_entries(ctx: Ctx, RawQuery(raw): RawQuery) -> Answer {
    let query: EntryQuery = ctx.query(raw, "query")?;
    let slots = ctx.admit()?;
    Ok(ctx.run(slots, move |backend| async move { backend.list_entries(&query).await }).await)
}

async fn load_reader_entry(ctx: Ctx, path: PathParam) -> Answer {
    let entry_id = ctx.path_id(path)?;
    let slots = ctx.admit()?;
    Ok(ctx
        .run(slots, move |backend| async move { backend.load_reader_entry(entry_id).await })
        .await)
}

async fn set_entry_read(ctx: Ctx, path: PathParam, request: Request) -> Answer {
    let entry_id = ctx.path_id(path)?;
    let slots = ctx.admit()?;
    let SetRead { read } = ctx.json(request).await?;
    Ok(ctx
        .run(slots, move |backend| async move { backend.set_entry_read(entry_id, read).await })
        .await)
}

async fn set_entry_starred(ctx: Ctx, path: PathParam, request: Request) -> Answer {
    let entry_id = ctx.path_id(path)?;
    let slots = ctx.admit()?;
    let SetStarred { starred } = ctx.json(request).await?;
    Ok(ctx
        .run(
            slots,
            move |backend| async move { backend.set_entry_starred(entry_id, starred).await },
        )
        .await)
}

/// The reader's image localization, given up after the state's limit. Giving up cancels the
/// downloads, but the body is written once at the end, compare-and-set: when the limit falls on
/// that write, it may still commit. The body is then either the original or the localized one,
/// never a mixture, and the answer is `outcome_unknown` (review C3-R1 F06; C4 design review
/// C4D-R1 B1: a client must tell it from a refusal before anything ran, which stays
/// `unavailable`).
async fn localize_entry_assets(ctx: Ctx, path: PathParam, request: Request) -> Answer {
    let entry_id = ctx.path_id(path)?;
    let slots = ctx.admit()?;
    ctx.no_body(request).await?;
    let limit = ctx.state.localize_limit;
    Ok(ctx
        .run(slots, move |backend| async move {
            tokio::time::timeout(limit, backend.localize_entry_assets(entry_id))
                .await
                .unwrap_or_else(|_| {
                    let message = "图片本地化超过时限，结果未知：请重新打开文章";
                    Err(BackendError::new(ErrorCode::OutcomeUnknown, message))
                })
        })
        .await)
}

async fn preview_mark_read(ctx: Ctx, RawQuery(raw): RawQuery) -> Answer {
    let query: EntryQuery = ctx.query(raw, "query")?;
    let slots = ctx.admit()?;
    Ok(ctx.run(slots, move |backend| async move { backend.preview_mark_read(&query).await }).await)
}

async fn mark_read_if_unchanged(ctx: Ctx, request: Request) -> Answer {
    let slots = ctx.admit()?;
    let preview: MarkReadPreview = ctx.json(request).await?;
    Ok(ctx
        .run(slots, move |backend| async move { backend.mark_read_if_unchanged(&preview).await })
        .await)
}

async fn load_feeds_snapshot(ctx: Ctx) -> Answer {
    let slots = ctx.admit()?;
    Ok(ctx.run(slots, |backend| async move { backend.load_feeds_snapshot().await }).await)
}

async fn remove_feed(ctx: Ctx, path: PathParam, request: Request) -> Answer {
    let feed_id = ctx.path_id(path)?;
    let slots = ctx.admit()?;
    ctx.no_body(request).await?;
    Ok(ctx.run(slots, move |backend| async move { backend.remove_feed(feed_id).await }).await)
}

async fn export_config_json(ctx: Ctx) -> Answer {
    let slots = ctx.admit()?;
    Ok(ctx.run(slots, |backend| async move { backend.export_config_json().await }).await)
}

async fn import_config_json(ctx: Ctx, request: Request) -> Answer {
    let slots = ctx.admit_import()?;
    let ImportConfigJson { raw, expected } = ctx.json(request).await?;
    Ok(ctx
        .run(slots, move |backend| async move { backend.import_config_json(&raw, &expected).await })
        .await)
}

async fn export_opml(ctx: Ctx) -> Answer {
    let slots = ctx.admit()?;
    Ok(ctx.run(slots, |backend| async move { backend.export_opml().await }).await)
}

async fn import_opml(ctx: Ctx, request: Request) -> Answer {
    let slots = ctx.admit_import()?;
    let ImportOpml { raw } = ctx.json(request).await?;
    Ok(ctx.run(slots, move |backend| async move { backend.import_opml(&raw).await }).await)
}

async fn start_refresh_all(ctx: Ctx, request: Request) -> Answer {
    ctx.job_without_input(request).await?;
    Ok(ctx.start(|backend| async move { Ok(JobResult::RefreshAll(backend.refresh_all().await?)) }))
}

async fn start_refresh_feed(ctx: Ctx, path: PathParam, request: Request) -> Answer {
    let feed_id = ctx.path_id(path)?;
    ctx.job_without_input(request).await?;
    Ok(ctx.start(move |backend| async move {
        Ok(JobResult::RefreshFeed(backend.refresh_feed(feed_id).await?))
    }))
}

async fn start_add_subscription(ctx: Ctx, request: Request) -> Answer {
    let AddSubscription { raw_url, fallback_site_url } = ctx.job_input(request).await?;
    Ok(ctx.start(move |backend| async move {
        let outcome = backend.add_subscription(&raw_url, fallback_site_url).await?;
        Ok(JobResult::AddSubscription(outcome))
    }))
}

async fn start_remote_push(ctx: Ctx, request: Request) -> Answer {
    let RemoteConfig { endpoint, remote_path } = ctx.job_input(request).await?;
    Ok(ctx.start(move |backend| async move {
        let outcome = backend.push_remote_config(&endpoint, &remote_path).await?;
        Ok(JobResult::RemoteConfigPush(outcome))
    }))
}

async fn start_remote_pull(ctx: Ctx, request: Request) -> Answer {
    let RemoteConfig { endpoint, remote_path } = ctx.job_input(request).await?;
    Ok(ctx.start(move |backend| async move {
        let outcome = backend.pull_remote_config(&endpoint, &remote_path).await?;
        Ok(JobResult::RemoteConfigPull(outcome))
    }))
}

/// A job's status, after waiting up to `wait_ms` (a plain integer, at most
/// `protocol::MAX_JOB_WAIT`) for it to finish. An id that is malformed, unknown, expired,
/// evicted or from before a restart is `status_unavailable`. A poll takes one of [`MAX_POLLS`]
/// slots while it waits (review C3-R1 F10).
async fn poll_job(ctx: Ctx, path: PathParam, RawQuery(raw): RawQuery) -> Answer {
    let wait_ms = match ctx.parameter(raw.as_deref(), "wait_ms")? {
        None => 0,
        Some(value) => value.parse::<u64>().map_err(|_| ctx.invalid("wait_ms 必须是非负整数"))?,
    };
    let _slot = Arc::clone(&ctx.state.polls).try_acquire_owned().map_err(|_| ctx.fail(busy()))?;
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Captured {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// The log records an error's request id and code, not its message (review C3-R1 F05).
    #[test]
    fn errors_are_logged_without_their_message() {
        let captured = Captured::default();
        let writer = captured.clone();
        let subscriber =
            tracing_subscriber::fmt().with_writer(move || writer.clone()).with_ansi(false).finish();
        let id = RequestId::new();
        tracing::subscriber::with_default(subscriber, || {
            for code in [ErrorCode::InvalidInput, ErrorCode::Unavailable] {
                let secret = "https://user:password@example.test/rss?token=secret";
                failure(BackendError::new(code, format!("重复的 feed URL：{secret}")), Some(&id));
            }
        });
        let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        assert!(log.contains(&*id.0) && log.contains("invalid_input"), "{log}");
        assert!(!log.contains("password") && !log.contains("token"), "{log}");
    }
}
