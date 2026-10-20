//! [`HttpBackend`]: the [`Backend`] over the server's `/api/v1` (v0.2 phase C, C4a;
//! `docs/design/v0.2-phase-c-plan.md`, "C4 的实施设计" and "结果未知").
//!
//! One implementation for the browser (the formal Web page) and for native code (the server's
//! tests run it against the real router). It encodes each operation as [`rssr_api::protocol`]
//! says and classifies what comes back:
//! - an answer of the protocol (its JSON error body) is that error;
//! - a read without a complete, parsable answer is `network`;
//! - a write that was sent but got no complete, parsable answer may have taken effect: it is
//!   `outcome_unknown` and is never sent again on its own. A write that provably never left is
//!   `network`: the CSRF token could not be fetched, or (native code only) the connection was not
//!   established. A failed `fetch` in the browser does not say how far it got, so there a write
//!   that started is always unknown;
//! - `csrf_mismatch` is refused before the server runs anything, so the token is fetched again
//!   and the write sent once more.
//!
//! Background jobs are created, then polled until they end; a lost result is `outcome_unknown`.
//! The client stopping its wait does not stop the job on the server.
//!
//! Answers are read up to the protocol's bound for what they answer (review C4A-02, C4A-R2-02:
//! the server sends nothing longer); a longer one is not read further and counts as an answer
//! that was not received. The bound is on the decoded bytes received so far: natively they come
//! in small pieces; in the browser reqwest copies each piece the browser hands over before it is
//! counted, so the memory used before a long answer is given up is the bound plus one such piece,
//! whose size is the browser's (review C4A-R2-04).

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::{Method, header};
use rssr_api::protocol::{
    self, AddSubscription, ImportConfigJson, ImportOpml, JobCreated, JobId, JobResult, JobStatus,
    MAX_ANSWER_BYTES, MAX_JOB_STATUS_BYTES, MAX_SMALL_ANSWER_BYTES, RemoteConfig, SetRead,
    SetStarred, routes,
};
use rssr_api::{Backend, BackendError, ErrorCode, Result};
use rssr_application::{
    AddSubscriptionOutcome, ConfigImportOutcome, EntriesBootstrapInput, EntriesBootstrapOutcome,
    EntriesListOutcome, EntryFlagOutcome, FeedsSnapshotOutcome, OpmlImportOutcome,
    ReaderEntrySnapshot, RefreshAllExecutionOutcome, RefreshFeedExecutionOutcome,
    RemoteConfigPullOutcome, RemoteConfigPushOutcome, SettingsPatch, StartupTarget,
};
use rssr_domain::{
    ConfigFingerprint, EntriesWorkspaceState, EntryQuery, MarkReadOutcome, MarkReadPreview,
    UserSettings,
};
use serde::{Serialize, de::DeserializeOwned};
use url::Url;

/// How long requests may take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// An ordinary request.
    pub request: Duration,
    /// A poll of a job, which waits up to [`protocol::MAX_JOB_WAIT`] on the server.
    pub poll: Duration,
    /// The reader's image localization, which the server gives 60 seconds.
    pub localize: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            request: Duration::from_secs(30),
            poll: Duration::from_secs(20),
            localize: Duration::from_secs(70),
        }
    }
}

/// The waits after consecutive failed polls; one more failure after the last ends the wait
/// (design review C4D-R2 R2-06).
pub const POLL_BACKOFF: [Duration; 5] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(16),
];

/// The [`Backend`] of a page served by `rssr-web`, or of a test talking to one.
pub struct HttpBackend {
    base: Url,
    build_id: String,
    client: reqwest::Client,
    timeouts: Timeouts,
    poll_backoff: Vec<Duration>,
    /// The session's CSRF token, fetched before the first write.
    csrf: Mutex<Option<String>>,
}

/// Whether a request changes anything: decides what a lost answer means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    Read,
    Write,
}

/// One request to send.
struct Call<'a> {
    method: Method,
    path: &'a str,
    query: Option<(&'a str, String)>,
    body: Option<Vec<u8>>,
    effect: Effect,
    timeout: Duration,
    /// The longest successful answer read.
    limit: usize,
}

/// Why an answer's body was not read.
enum BodyFailure {
    /// The connection ended or failed before the end.
    Lost,
    /// Longer than its limit.
    TooLong,
}

impl HttpBackend {
    /// The backend of the server at `base` (its origin), for pages of the build `build_id`
    /// (computed where the page or the test is built, `build-support/build_id.rs`).
    pub fn new(base: Url, build_id: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            base,
            build_id: build_id.into(),
            client,
            timeouts: Timeouts::default(),
            poll_backoff: POLL_BACKOFF.to_vec(),
            csrf: Mutex::new(None),
        }
    }

    /// Other time limits (tests use short ones).
    pub fn with_timeouts(mut self, timeouts: Timeouts) -> Self {
        self.timeouts = timeouts;
        self
    }

    /// Other waits between failed polls (tests use short ones); the wait ends after one more
    /// failure than there are waits.
    pub fn with_poll_backoff(mut self, backoff: Vec<Duration>) -> Self {
        self.poll_backoff = backoff;
        self
    }

    /// Starts with `token` as the session's CSRF token instead of fetching one (tests start with
    /// a stale one to see the refusal and the retry).
    pub fn with_csrf_token(self, token: impl Into<String>) -> Self {
        *self.csrf.lock().unwrap_or_else(PoisonError::into_inner) = Some(token.into());
        self
    }

    /// A native client as this backend needs it: it never retries a request on its own and
    /// never follows a redirect (the browser's `fetch` has neither of these knobs; the server
    /// answers `/api` without redirects).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native_client() -> reqwest::ClientBuilder {
        reqwest::Client::builder()
            .retry(reqwest::retry::never())
            .redirect(reqwest::redirect::Policy::none())
    }

    async fn read<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Option<(&str, String)>,
    ) -> Result<T> {
        let bytes = self.send(self.read_call(path, query, MAX_ANSWER_BYTES), None).await?;
        decode(&bytes, Effect::Read)
    }

    fn read_call<'a>(
        &self,
        path: &'a str,
        query: Option<(&'a str, String)>,
        limit: usize,
    ) -> Call<'a> {
        Call {
            method: Method::GET,
            path,
            query,
            body: None,
            effect: Effect::Read,
            timeout: self.timeouts.request,
            limit,
        }
    }

    /// A structured read: its input as JSON in the query parameter `name`.
    async fn read_with<T: DeserializeOwned>(
        &self,
        path: &str,
        name: &str,
        input: &impl Serialize,
    ) -> Result<T> {
        self.read(path, Some((name, encode_text(input)?))).await
    }

    async fn write<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
        timeout: Duration,
        limit: usize,
    ) -> Result<T> {
        let mut fetched_again = false;
        loop {
            let token = self.csrf_token().await?;
            let call = Call {
                method: method.clone(),
                path,
                query: None,
                body: body.clone(),
                effect: Effect::Write,
                timeout,
                limit,
            };
            match self.send(call, Some(&token)).await {
                Err(error) if error.code == ErrorCode::CsrfMismatch && !fetched_again => {
                    // Refused before anything ran: fetch the token again and send once more.
                    fetched_again = true;
                    self.forget_token(&token);
                }
                sent => return decode(&sent?, Effect::Write),
            }
        }
    }

    async fn csrf_token(&self) -> Result<String> {
        if let Some(token) = self.csrf.lock().unwrap_or_else(PoisonError::into_inner).clone() {
            return Ok(token);
        }
        let call = self.read_call(routes::SESSION, None, MAX_SMALL_ANSWER_BYTES);
        let session: protocol::Session = decode(&self.send(call, None).await?, Effect::Read)?;
        *self.csrf.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(session.csrf_token.clone());
        Ok(session.csrf_token)
    }

    fn forget_token(&self, used: &str) {
        let mut token = self.csrf.lock().unwrap_or_else(PoisonError::into_inner);
        if token.as_deref() == Some(used) {
            *token = None;
        }
    }

    /// Sends one request; the body of a success, or the error the answer means.
    async fn send(&self, call: Call<'_>, csrf: Option<&str>) -> Result<Vec<u8>> {
        let effect = call.effect;
        let url = self.base.join(call.path).map_err(|error| {
            BackendError::new(ErrorCode::Internal, format!("接口地址无效：{error}"))
        })?;
        let mut request = self
            .client
            .request(call.method, url)
            .timeout(call.timeout)
            .header(protocol::BUILD_HEADER, &self.build_id);
        if let Some((name, value)) = call.query {
            request = request.query(&[(name, value)]);
        }
        if let Some(token) = csrf {
            request = request.header(protocol::CSRF_HEADER, token);
        }
        if let Some(body) = call.body {
            request = request.header(header::CONTENT_TYPE, "application/json").body(body);
        }
        let response = request.send().await.map_err(|error| send_failure(&error, effect))?;
        let status = response.status();
        // An error answer is small (the server shortens its message); a longer one is not this
        // protocol's.
        let limit = if status.is_success() { call.limit } else { MAX_SMALL_ANSWER_BYTES };
        let body = read_body(response, limit).await;
        if status.is_success() {
            return body.map_err(|failure| match failure {
                BodyFailure::Lost => lost(effect),
                BodyFailure::TooLong => too_long(effect),
            });
        }
        match body.ok().and_then(|body| serde_json::from_slice::<BackendError>(&body).ok()) {
            Some(error) => Err(error),
            // Not this protocol's answer (a proxy's error page, an error cut off or too long):
            // nothing says what happened.
            None => Err(match effect {
                Effect::Read => BackendError::new(
                    ErrorCode::Network,
                    format!("服务器没有给出本程序的回答（HTTP {status}），请稍后重试"),
                ),
                Effect::Write => unknown(),
            }),
        }
    }

    /// Starts a background job and waits until it ends.
    async fn run_job(&self, path: &str, body: Option<Vec<u8>>) -> Result<JobResult> {
        let created: JobCreated = self
            .write(Method::POST, path, body, self.timeouts.request, MAX_SMALL_ANSWER_BYTES)
            .await?;
        // An id not of the protocol's form is no id: nothing to poll (review C4A-03).
        let id = JobId::parse(&created.job_id.0).ok_or_else(|| {
            BackendError::new(
                ErrorCode::OutcomeUnknown,
                "服务器没有给出有效的任务编号，这次操作的结果未知，请刷新查看",
            )
        })?;
        self.wait_for(&id).await
    }

    /// Polls the job `id` until it ends (design "C4a" 3).
    async fn wait_for(&self, id: &JobId) -> Result<JobResult> {
        let wait_ms = protocol::MAX_JOB_WAIT.as_millis().to_string();
        let path = protocol::job(id);
        let mut failures = 0;
        loop {
            let call = Call {
                method: Method::GET,
                path: &path,
                query: Some(("wait_ms", wait_ms.clone())),
                body: None,
                effect: Effect::Read,
                timeout: self.timeouts.poll,
                limit: MAX_JOB_STATUS_BYTES,
            };
            let polled = self
                .send(call, None)
                .await
                .and_then(|bytes| decode::<JobStatus>(&bytes, Effect::Read));
            match polled {
                Ok(JobStatus::Succeeded { result }) => return Ok(result),
                // The job's own failure, as the server reports it (it may have committed part).
                Ok(JobStatus::Failed { error }) => return Err(error),
                Ok(JobStatus::Queued | JobStatus::Running) => failures = 0,
                Err(error) if error.code == ErrorCode::StatusUnavailable => {
                    return Err(BackendError::new(
                        ErrorCode::OutcomeUnknown,
                        "后台任务的结果已不可用（已过期、被淘汰或服务器重启过），结果未知，请刷新查看",
                    ));
                }
                Err(error) if error.code == ErrorCode::Unauthenticated => {
                    return Err(BackendError::new(
                        ErrorCode::OutcomeUnknown,
                        "登录已失效，后台任务的结果未知，请重新登录后查看",
                    )
                    .with_reason(ErrorCode::Unauthenticated));
                }
                Err(_) => {
                    let Some(wait) = self.poll_backoff.get(failures) else {
                        return Err(BackendError::new(
                            ErrorCode::OutcomeUnknown,
                            "与服务器失去联系，后台任务的结果未知，请刷新查看",
                        ));
                    };
                    failures += 1;
                    sleep(*wait).await;
                }
            }
        }
    }
}

/// The body of `response`, read piece by piece and given up past `limit` bytes.
async fn read_body(
    response: reqwest::Response,
    limit: usize,
) -> std::result::Result<Vec<u8>, BodyFailure> {
    // Natively the declared length is the body's own (a compressed answer, decoded on the way,
    // has none); in the browser it is the encoded length while the body comes decoded, so it is
    // not checked there (review C4A-R2-03).
    #[cfg(not(target_arch = "wasm32"))]
    if response.content_length().is_some_and(|length| length > limit as u64) {
        return Err(BodyFailure::TooLong);
    }
    let mut body = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|_| BodyFailure::Lost)?;
        if body.len() + chunk.len() > limit {
            return Err(BodyFailure::TooLong);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// A successful answer too long to read.
fn too_long(effect: Effect) -> BackendError {
    match effect {
        Effect::Read => BackendError::new(ErrorCode::Network, "服务器的回答过长，没有读取"),
        Effect::Write => unknown(),
    }
}

/// What a request that failed before an answer means.
fn send_failure(error: &reqwest::Error, effect: Effect) -> BackendError {
    match effect {
        Effect::Write if !never_left(error) => unknown(),
        _ => BackendError::new(ErrorCode::Network, "无法连接服务器，请检查网络后重试"),
    }
}

/// Whether the request provably did not reach the server.
#[cfg(not(target_arch = "wasm32"))]
fn never_left(error: &reqwest::Error) -> bool {
    error.is_builder() || error.is_connect()
}

/// A failed `fetch` does not say how far it got; only a request that could not be built is known
/// not to have left.
#[cfg(target_arch = "wasm32")]
fn never_left(error: &reqwest::Error) -> bool {
    error.is_builder()
}

/// An answer that was cut off.
fn lost(effect: Effect) -> BackendError {
    match effect {
        Effect::Read => BackendError::new(ErrorCode::Network, "没有收到完整的回答，请稍后重试"),
        Effect::Write => unknown(),
    }
}

fn unknown() -> BackendError {
    BackendError::new(
        ErrorCode::OutcomeUnknown,
        "与服务器的连接中断，这次操作的结果未知，请刷新查看",
    )
}

fn decode<T: DeserializeOwned>(bytes: &[u8], effect: Effect) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|_| match effect {
        Effect::Read => BackendError::new(ErrorCode::Network, "服务器的回答无法解析，请稍后重试"),
        // A success whose content is not what was asked for is no success.
        Effect::Write => unknown(),
    })
}

fn encode(value: &impl Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(value)
        .map_err(|error| BackendError::new(ErrorCode::Internal, format!("无法编码请求：{error}")))
}

fn encode_text(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value)
        .map_err(|error| BackendError::new(ErrorCode::Internal, format!("无法编码请求：{error}")))
}

/// A finished job whose result is of another kind than the one started: no success is made up.
fn wrong_kind() -> BackendError {
    BackendError::new(
        ErrorCode::OutcomeUnknown,
        "服务器回答了另一种任务的结果，这次操作的结果未知，请刷新查看",
    )
}

#[cfg(not(target_arch = "wasm32"))]
async fn sleep(wait: Duration) {
    tokio::time::sleep(wait).await;
}

#[cfg(target_arch = "wasm32")]
async fn sleep(wait: Duration) {
    gloo_timers::future::sleep(wait).await;
}

// `async_trait` marks its boxed futures `#[must_use]` twice.
#[allow(clippy::double_must_use)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl Backend for HttpBackend {
    async fn load_settings(&self) -> Result<UserSettings> {
        self.read(routes::SETTINGS, None).await
    }

    async fn update_settings(&self, patch: SettingsPatch) -> Result<UserSettings> {
        self.write(
            Method::PATCH,
            routes::SETTINGS,
            Some(encode(&patch)?),
            self.timeouts.request,
            MAX_ANSWER_BYTES,
        )
        .await
    }

    /// The server runs the one periodic refresh (phase C, C5).
    fn ensure_auto_refresh_started(&self) {}

    async fn resolve_startup_target(&self) -> Result<StartupTarget> {
        self.read(routes::STARTUP_TARGET, None).await
    }

    async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPushOutcome> {
        let body = encode(&RemoteConfig {
            endpoint: endpoint.to_owned(),
            remote_path: remote_path.to_owned(),
        })?;
        match self.run_job(routes::JOB_REMOTE_PUSH, Some(body)).await? {
            JobResult::RemoteConfigPush(outcome) => Ok(outcome),
            _ => Err(wrong_kind()),
        }
    }

    async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPullOutcome> {
        let body = encode(&RemoteConfig {
            endpoint: endpoint.to_owned(),
            remote_path: remote_path.to_owned(),
        })?;
        match self.run_job(routes::JOB_REMOTE_PULL, Some(body)).await? {
            JobResult::RemoteConfigPull(outcome) => Ok(outcome),
            _ => Err(wrong_kind()),
        }
    }

    async fn bootstrap_entries(
        &self,
        input: EntriesBootstrapInput,
    ) -> Result<EntriesBootstrapOutcome> {
        self.read_with(routes::ENTRIES_BOOTSTRAP, "input", &input).await
    }

    async fn remember_opened_feed(&self, feed_id: i64) -> Result<()> {
        self.write(
            Method::PUT,
            &protocol::opened_feed(feed_id),
            None,
            self.timeouts.request,
            MAX_ANSWER_BYTES,
        )
        .await
    }

    async fn save_entries_workspace_if_changed(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> Result<bool> {
        self.write(
            Method::PUT,
            routes::WORKSPACE,
            Some(encode(&workspace)?),
            self.timeouts.request,
            MAX_ANSWER_BYTES,
        )
        .await
    }

    async fn list_entries(&self, query: &EntryQuery) -> Result<EntriesListOutcome> {
        self.read_with(routes::ENTRIES, "query", query).await
    }

    async fn set_entry_read(&self, entry_id: i64, read: bool) -> Result<EntryFlagOutcome> {
        let body = encode(&SetRead { read })?;
        self.write(
            Method::PUT,
            &protocol::entry_read(entry_id),
            Some(body),
            self.timeouts.request,
            MAX_ANSWER_BYTES,
        )
        .await
    }

    async fn set_entry_starred(&self, entry_id: i64, starred: bool) -> Result<EntryFlagOutcome> {
        let body = encode(&SetStarred { starred })?;
        self.write(
            Method::PUT,
            &protocol::entry_starred(entry_id),
            Some(body),
            self.timeouts.request,
            MAX_ANSWER_BYTES,
        )
        .await
    }

    async fn preview_mark_read(&self, query: &EntryQuery) -> Result<MarkReadPreview> {
        self.read_with(routes::MARK_READ, "query", query).await
    }

    async fn mark_read_if_unchanged(&self, preview: &MarkReadPreview) -> Result<MarkReadOutcome> {
        self.write(
            Method::POST,
            routes::MARK_READ,
            Some(encode(preview)?),
            self.timeouts.request,
            MAX_ANSWER_BYTES,
        )
        .await
    }

    async fn load_reader_entry(&self, entry_id: i64) -> Result<ReaderEntrySnapshot> {
        self.read(&protocol::entry(entry_id), None).await
    }

    async fn localize_entry_assets(&self, entry_id: i64) -> Result<bool> {
        self.write(
            Method::POST,
            &protocol::entry_assets(entry_id),
            None,
            self.timeouts.localize,
            MAX_ANSWER_BYTES,
        )
        .await
    }

    async fn load_feeds_snapshot(&self) -> Result<FeedsSnapshotOutcome> {
        self.read(routes::FEEDS, None).await
    }

    async fn add_subscription(
        &self,
        raw_url: &str,
        fallback_site_url: Option<Url>,
    ) -> Result<AddSubscriptionOutcome> {
        let body = encode(&AddSubscription { raw_url: raw_url.to_owned(), fallback_site_url })?;
        match self.run_job(routes::JOB_ADD_SUBSCRIPTION, Some(body)).await? {
            JobResult::AddSubscription(outcome) => Ok(outcome),
            _ => Err(wrong_kind()),
        }
    }

    async fn refresh_all(&self) -> Result<RefreshAllExecutionOutcome> {
        match self.run_job(routes::JOB_REFRESH_ALL, None).await? {
            JobResult::RefreshAll(outcome) => Ok(outcome),
            _ => Err(wrong_kind()),
        }
    }

    async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedExecutionOutcome> {
        match self.run_job(&protocol::job_refresh_feed(feed_id), None).await? {
            JobResult::RefreshFeed(outcome) => Ok(outcome),
            _ => Err(wrong_kind()),
        }
    }

    async fn remove_feed(&self, feed_id: i64) -> Result<()> {
        self.write(
            Method::DELETE,
            &protocol::feed(feed_id),
            None,
            self.timeouts.request,
            MAX_ANSWER_BYTES,
        )
        .await
    }

    async fn export_config_json(&self) -> Result<String> {
        self.read(routes::CONFIG_JSON, None).await
    }

    async fn import_config_json(
        &self,
        raw: &str,
        expected: &ConfigFingerprint,
    ) -> Result<ConfigImportOutcome> {
        let body = encode(&ImportConfigJson { raw: raw.to_owned(), expected: expected.clone() })?;
        self.write(
            Method::PUT,
            routes::CONFIG_JSON,
            Some(body),
            self.timeouts.request,
            MAX_ANSWER_BYTES,
        )
        .await
    }

    async fn export_opml(&self) -> Result<String> {
        self.read(routes::CONFIG_OPML, None).await
    }

    async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome> {
        let body = encode(&ImportOpml { raw: raw.to_owned() })?;
        self.write(
            Method::POST,
            routes::CONFIG_OPML,
            Some(body),
            self.timeouts.request,
            MAX_ANSWER_BYTES,
        )
        .await
    }
}
