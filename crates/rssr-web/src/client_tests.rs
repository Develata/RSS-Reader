//! The Web page's [`HttpBackend`] (v0.2 phase C, C4a) run natively: against the real router behind
//! the real login, Origin and CSRF checks, each operation compared with an independent
//! [`rssr_infra::local_backend::LocalBackend`] over an identical library; and against a scripted
//! server for what a real one does not do on demand — answers lost or cut off, a proxy's error
//! page, a CSRF refusal, polls that fail, answers too long, ids not of the protocol's form, a wait
//! given up — counting the requests that reach it, so a write whose outcome is unknown is shown
//! never to be sent again. Behind the real router an [`Observed`] backend records what ran: a
//! request refused before the application runs nothing, and a localization whose write-back
//! outlives the server's limit still commits after its `outcome_unknown` answer.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use reqwest::header::{COOKIE, HeaderMap, HeaderValue, ORIGIN};
use rssr_api::{
    AddSubscriptionOutcome, Backend, BackendError, ErrorCode, RefreshAllExecutionOutcome,
    RefreshFeedExecutionOutcome,
    protocol::{JobResult, JobStatus},
};
use rssr_application::{
    ConfigImportOutcome, EntriesBootstrapInput, EntriesBootstrapOutcome, EntriesListOutcome,
    EntryFlagOutcome, FeedsSnapshotOutcome, OpmlImportOutcome, ReaderEntrySnapshot,
    RemoteConfigPullOutcome, RemoteConfigPushOutcome, SettingsPatch, StartupTarget,
};
use rssr_domain::{
    ConfigFingerprint, EntriesWorkspaceState, EntryQuery, MarkReadOutcome, MarkReadPreview,
    ReadFilter, UserSettings,
};
use rssr_infra::http_backend::{HttpBackend, Timeouts};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use url::Url;

use crate::{
    api::ApiState,
    api_tests::{FeedServer, Library, at_rest, entry_id, library, localized},
    http_tests::{Server, TEST_BUILD},
};

/// The page's backend as a native client: the session cookie (if any) and the trusted Origin on
/// every request, as the browser sends them; the CSRF token is the backend's own business.
fn http_backend(server: &Server, session: Option<&str>, build: &str) -> HttpBackend {
    let mut headers = HeaderMap::new();
    if let Some(session) = session {
        headers.insert(COOKIE, HeaderValue::from_str(session).unwrap());
    }
    headers.insert(ORIGIN, HeaderValue::from_str(&server.base).unwrap());
    let client = HttpBackend::native_client().default_headers(headers).no_proxy().build().unwrap();
    HttpBackend::new(Url::parse(&server.base).unwrap(), build, client)
}

#[track_caller]
fn same(via_http: impl Serialize, direct: impl Serialize, what: &str) {
    let via_http = serde_json::to_value(via_http).unwrap();
    let direct = serde_json::to_value(direct).unwrap();
    assert_eq!(at_rest(via_http), at_rest(direct), "{what}");
}

/// Every synchronous operation through the client answers what the backend answers, in the same
/// sequence over identical libraries.
#[tokio::test]
async fn every_operation_through_the_client_answers_what_the_backend_answers() {
    let feeds = FeedServer::start().await;
    let served = library(&feeds, &["a.xml", "b.xml"]).await;
    let direct = library(&feeds, &["a.xml", "b.xml"]).await;
    localized(&served, "Image post").await;
    localized(&direct, "Image post").await;
    let api = ApiState::with(Arc::new(served), TEST_BUILD, Duration::from_secs(60));
    let server = Server::start_with(None, api).await;
    let session = server.session().await;
    let client = http_backend(&server, Some(&session), TEST_BUILD);
    client.ensure_auto_refresh_started();

    same(client.load_settings().await.unwrap(), direct.load_settings().await.unwrap(), "settings");
    let patch = SettingsPatch {
        entries_page_size: Some(50),
        custom_css: Some("body { margin: 0 }".into()),
        ..Default::default()
    };
    same(
        client.update_settings(patch.clone()).await.unwrap(),
        direct.update_settings(patch).await.unwrap(),
        "update settings",
    );
    same(
        client.resolve_startup_target().await.unwrap(),
        direct.resolve_startup_target().await.unwrap(),
        "startup target",
    );
    let input = EntriesBootstrapInput { load_preferences: true, load_feeds: true };
    same(
        client.bootstrap_entries(input).await.unwrap(),
        direct.bootstrap_entries(input).await.unwrap(),
        "bootstrap",
    );
    same(
        client.load_feeds_snapshot().await.unwrap(),
        direct.load_feeds_snapshot().await.unwrap(),
        "feeds",
    );

    let all = EntryQuery::default();
    let feed_a = direct.load_feeds_snapshot().await.unwrap().feeds[0].id;
    let rust_in_a = EntryQuery {
        feed_id: Some(feed_a),
        search_title: Some("Rust".into()),
        ..Default::default()
    };
    for query in [&all, &rust_in_a] {
        same(
            client.list_entries(query).await.unwrap(),
            direct.list_entries(query).await.unwrap(),
            "entries",
        );
    }

    let image = entry_id(&direct, "Image post").await;
    let rust_tips = entry_id(&direct, "Rust tips").await;
    same(
        client.set_entry_read(rust_tips, true).await.unwrap(),
        direct.set_entry_read(rust_tips, true).await.unwrap(),
        "set read",
    );
    same(
        client.set_entry_starred(image, true).await.unwrap(),
        direct.set_entry_starred(image, true).await.unwrap(),
        "set starred",
    );
    same(
        client.load_reader_entry(image).await.unwrap(),
        direct.load_reader_entry(image).await.unwrap(),
        "reader entry",
    );
    same(
        client.localize_entry_assets(image).await.unwrap(),
        direct.localize_entry_assets(image).await.unwrap(),
        "localize (already done in the background)",
    );

    let in_a = EntryQuery { feed_id: Some(feed_a), ..Default::default() };
    let preview = client.preview_mark_read(&in_a).await.unwrap();
    let direct_preview = direct.preview_mark_read(&in_a).await.unwrap();
    same(&preview, &direct_preview, "mark-read preview");
    same(
        client.mark_read_if_unchanged(&preview).await.unwrap(),
        direct.mark_read_if_unchanged(&direct_preview).await.unwrap(),
        "mark read",
    );

    client.remember_opened_feed(feed_a).await.unwrap();
    direct.remember_opened_feed(feed_a).await.unwrap();
    let workspace = EntriesWorkspaceState {
        read_filter: ReadFilter::UnreadOnly,
        show_archived: true,
        ..Default::default()
    };
    same(
        client.save_entries_workspace_if_changed(workspace.clone()).await.unwrap(),
        direct.save_entries_workspace_if_changed(workspace).await.unwrap(),
        "save workspace",
    );

    let exported = client.export_config_json().await.unwrap();
    let direct_exported = direct.export_config_json().await.unwrap();
    same(
        serde_json::from_str::<Value>(&exported).unwrap(),
        serde_json::from_str::<Value>(&direct_exported).unwrap(),
        "config export",
    );
    same(client.export_opml().await.unwrap(), direct.export_opml().await.unwrap(), "opml");

    let mut package: Value = serde_json::from_str(&direct_exported).unwrap();
    let mut added = package["feeds"][0].clone();
    added["url"] = json!(feeds.url("c.xml"));
    package["feeds"].as_array_mut().unwrap().push(added);
    let raw = package.to_string();
    let expected = direct.load_feeds_snapshot().await.unwrap().fingerprint;
    same(
        client.import_config_json(&raw, &expected).await.unwrap(),
        direct.import_config_json(&raw, &expected).await.unwrap(),
        "config import",
    );
    let opml = format!(
        r#"<?xml version="1.0"?><opml version="2.0"><head><title>t</title></head><body><outline type="rss" text="D" xmlUrl="{}"/></body></opml>"#,
        feeds.url("d.xml")
    );
    same(
        client.import_opml(&opml).await.unwrap(),
        direct.import_opml(&opml).await.unwrap(),
        "opml import",
    );

    client.remove_feed(feed_a).await.unwrap();
    direct.remove_feed(feed_a).await.unwrap();
    same(
        client.load_feeds_snapshot().await.unwrap(),
        direct.load_feeds_snapshot().await.unwrap(),
        "after",
    );
    same(
        client.list_entries(&all).await.unwrap(),
        direct.list_entries(&all).await.unwrap(),
        "entries after",
    );
}

/// The five jobs through the client: started, polled and unwrapped to what the backend answers; a
/// failed job carries the backend's error.
#[tokio::test]
async fn every_job_through_the_client_answers_what_the_backend_answers() {
    let feeds = FeedServer::start().await;
    let served = library(&feeds, &["a.xml"]).await;
    let direct = library(&feeds, &["a.xml"]).await;
    let api = ApiState::with(Arc::new(served), TEST_BUILD, Duration::from_secs(60));
    let server = Server::start_with(None, api).await;
    let session = server.session().await;
    let client = http_backend(&server, Some(&session), TEST_BUILD);

    same(client.refresh_all().await.unwrap(), direct.refresh_all().await.unwrap(), "refresh all");
    let feed_id = direct.load_feeds_snapshot().await.unwrap().feeds[0].id;
    same(
        client.refresh_feed(feed_id).await.unwrap(),
        direct.refresh_feed(feed_id).await.unwrap(),
        "refresh",
    );
    let added = client.add_subscription(&feeds.url("c.xml"), None).await.unwrap();
    assert!(matches!(added, AddSubscriptionOutcome::SavedAndRefreshed), "{added:?}");
    same(added, direct.add_subscription(&feeds.url("c.xml"), None).await.unwrap(), "add");

    let endpoint = feeds.url("dav/");
    same(
        client.push_remote_config(&endpoint, "served.json").await.unwrap(),
        direct.push_remote_config(&endpoint, "direct.json").await.unwrap(),
        "push",
    );
    same(
        client.pull_remote_config(&endpoint, "served.json").await.unwrap(),
        direct.pull_remote_config(&endpoint, "direct.json").await.unwrap(),
        "pull",
    );

    let error = client.refresh_feed(999_999).await.unwrap_err();
    let expected = direct.refresh_feed(999_999).await.unwrap_err();
    assert_eq!((error.code, &error.message), (expected.code, &expected.message));
    assert!(error.request_id.is_some(), "the job's error carries the request id");
}

/// The session's rules seen from the client: without a session every request, the CSRF token
/// first, is `unauthenticated`; a page of another build reads but cannot write; the
/// localization cut off by the server's limit is `outcome_unknown` (design review C4D-R1 B1).
#[tokio::test]
async fn the_client_sees_the_servers_rules() {
    let feeds = FeedServer::start().await;
    let served = library(&feeds, &["b.xml"]).await;
    let id = entry_id(&served, "Rust tips").await;
    let api = ApiState::with(Arc::new(served), TEST_BUILD, Duration::from_secs(60));
    let server = Server::start_with(None, api).await;
    let session = server.session().await;

    let anonymous = http_backend(&server, None, TEST_BUILD);
    assert_eq!(anonymous.load_settings().await.unwrap_err().code, ErrorCode::Unauthenticated);
    let error = anonymous.set_entry_read(id, true).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Unauthenticated, "the token fetch's own error: {error}");

    let stale = http_backend(&server, Some(&session), "another-build");
    assert!(stale.load_reader_entry(id).await.is_ok(), "reads do not need this build");
    assert_eq!(stale.set_entry_read(id, true).await.unwrap_err().code, ErrorCode::BuildMismatch);
    let current = http_backend(&server, Some(&session), TEST_BUILD);
    assert!(
        !current.load_reader_entry(id).await.unwrap().entry.unwrap().is_read,
        "nothing written"
    );

    let slow = library(&feeds, &["slow.xml"]).await;
    let slow_id = entry_id(&slow, "Slow").await;
    let api = ApiState::with(Arc::new(slow), TEST_BUILD, Duration::from_millis(300));
    let server = Server::start_with(None, api).await;
    let session = server.session().await;
    let client = http_backend(&server, Some(&session), TEST_BUILD);
    let error = client.localize_entry_assets(slow_id).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::OutcomeUnknown, "{error}");
}

/// Waits until `done` holds, checking every 10 ms for up to 10 s.
async fn until(what: &str, done: impl Fn() -> bool) {
    for _ in 0..1000 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("{what} did not happen");
}

/// A library behind the real router that records each operation run on it, so a request refused
/// before the application is seen to have run nothing.
struct Observed {
    library: Library,
    ran: Mutex<Vec<&'static str>>,
    /// When set, the reader's localization is only its write-back, taking this long; like a
    /// store write that has begun committing (`rssr-infra/src/store/executor.rs`), it ends even
    /// when its caller stops waiting.
    write_back: Option<Duration>,
    committed: Arc<AtomicBool>,
}

impl Observed {
    fn new(library: Library, write_back: Option<Duration>) -> Arc<Self> {
        let ran = Mutex::new(Vec::new());
        Arc::new(Self { library, ran, write_back, committed: Arc::default() })
    }

    fn note(&self, operation: &'static str) {
        self.ran.lock().unwrap().push(operation);
    }

    /// How many times `operation` ran.
    fn times(&self, operation: &str) -> usize {
        self.ran.lock().unwrap().iter().filter(|ran| **ran == operation).count()
    }

    fn committed(&self) -> bool {
        self.committed.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl Backend for Observed {
    async fn load_settings(&self) -> rssr_api::Result<UserSettings> {
        self.note("load_settings");
        self.library.load_settings().await
    }

    async fn update_settings(&self, patch: SettingsPatch) -> rssr_api::Result<UserSettings> {
        self.note("update_settings");
        self.library.update_settings(patch).await
    }

    fn ensure_auto_refresh_started(&self) {
        self.library.ensure_auto_refresh_started();
    }

    async fn resolve_startup_target(&self) -> rssr_api::Result<StartupTarget> {
        self.note("resolve_startup_target");
        self.library.resolve_startup_target().await
    }

    async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> rssr_api::Result<RemoteConfigPushOutcome> {
        self.note("push_remote_config");
        self.library.push_remote_config(endpoint, remote_path).await
    }

    async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> rssr_api::Result<RemoteConfigPullOutcome> {
        self.note("pull_remote_config");
        self.library.pull_remote_config(endpoint, remote_path).await
    }

    async fn bootstrap_entries(
        &self,
        input: EntriesBootstrapInput,
    ) -> rssr_api::Result<EntriesBootstrapOutcome> {
        self.note("bootstrap_entries");
        self.library.bootstrap_entries(input).await
    }

    async fn remember_opened_feed(&self, feed_id: i64) -> rssr_api::Result<()> {
        self.note("remember_opened_feed");
        self.library.remember_opened_feed(feed_id).await
    }

    async fn save_entries_workspace_if_changed(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> rssr_api::Result<bool> {
        self.note("save_entries_workspace_if_changed");
        self.library.save_entries_workspace_if_changed(workspace).await
    }

    async fn list_entries(&self, query: &EntryQuery) -> rssr_api::Result<EntriesListOutcome> {
        self.note("list_entries");
        self.library.list_entries(query).await
    }

    async fn set_entry_read(
        &self,
        entry_id: i64,
        read: bool,
    ) -> rssr_api::Result<EntryFlagOutcome> {
        self.note("set_entry_read");
        self.library.set_entry_read(entry_id, read).await
    }

    async fn set_entry_starred(
        &self,
        entry_id: i64,
        starred: bool,
    ) -> rssr_api::Result<EntryFlagOutcome> {
        self.note("set_entry_starred");
        self.library.set_entry_starred(entry_id, starred).await
    }

    async fn preview_mark_read(&self, query: &EntryQuery) -> rssr_api::Result<MarkReadPreview> {
        self.note("preview_mark_read");
        self.library.preview_mark_read(query).await
    }

    async fn mark_read_if_unchanged(
        &self,
        preview: &MarkReadPreview,
    ) -> rssr_api::Result<MarkReadOutcome> {
        self.note("mark_read_if_unchanged");
        self.library.mark_read_if_unchanged(preview).await
    }

    async fn load_reader_entry(&self, entry_id: i64) -> rssr_api::Result<ReaderEntrySnapshot> {
        self.note("load_reader_entry");
        self.library.load_reader_entry(entry_id).await
    }

    async fn localize_entry_assets(&self, entry_id: i64) -> rssr_api::Result<bool> {
        self.note("localize_entry_assets");
        let Some(write_back) = self.write_back else {
            return self.library.localize_entry_assets(entry_id).await;
        };
        let committed = Arc::clone(&self.committed);
        let (done, ended) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            std::thread::sleep(write_back);
            committed.store(true, Ordering::SeqCst);
            let _ = done.send(());
        });
        ended.await.map_err(|_| BackendError::new(ErrorCode::Internal, "no write-back"))?;
        Ok(true)
    }

    async fn load_feeds_snapshot(&self) -> rssr_api::Result<FeedsSnapshotOutcome> {
        self.note("load_feeds_snapshot");
        self.library.load_feeds_snapshot().await
    }

    async fn add_subscription(
        &self,
        raw_url: &str,
        fallback_site_url: Option<Url>,
    ) -> rssr_api::Result<AddSubscriptionOutcome> {
        self.note("add_subscription");
        self.library.add_subscription(raw_url, fallback_site_url).await
    }

    async fn refresh_all(&self) -> rssr_api::Result<RefreshAllExecutionOutcome> {
        self.note("refresh_all");
        self.library.refresh_all().await
    }

    async fn refresh_feed(&self, feed_id: i64) -> rssr_api::Result<RefreshFeedExecutionOutcome> {
        self.note("refresh_feed");
        self.library.refresh_feed(feed_id).await
    }

    async fn remove_feed(&self, feed_id: i64) -> rssr_api::Result<()> {
        self.note("remove_feed");
        self.library.remove_feed(feed_id).await
    }

    async fn export_config_json(&self) -> rssr_api::Result<String> {
        self.note("export_config_json");
        self.library.export_config_json().await
    }

    async fn import_config_json(
        &self,
        raw: &str,
        expected: &ConfigFingerprint,
    ) -> rssr_api::Result<ConfigImportOutcome> {
        self.note("import_config_json");
        self.library.import_config_json(raw, expected).await
    }

    async fn export_opml(&self) -> rssr_api::Result<String> {
        self.note("export_opml");
        self.library.export_opml().await
    }

    async fn import_opml(&self, raw: &str) -> rssr_api::Result<OpmlImportOutcome> {
        self.note("import_opml");
        self.library.import_opml(raw).await
    }
}

/// A write sent with a stale CSRF token is refused before the application runs it; the client
/// fetches the token again and sends once more, and the operation runs once. Writes started
/// together with the same stale token, or before any token was fetched, each run once (design
/// "C4a" 7; review C4A-01).
#[tokio::test]
async fn a_csrf_refusal_runs_nothing_and_the_retry_runs_once() {
    let feeds = FeedServer::start().await;
    let served = library(&feeds, &["b.xml"]).await;
    let id = entry_id(&served, "Rust tips").await;
    let observed = Observed::new(served, None);
    let api = ApiState::with(observed.clone(), TEST_BUILD, Duration::from_secs(60));
    let server = Server::start_with(None, api).await;
    let session = server.session().await;

    let refused = reqwest::Client::new()
        .put(format!("{}{}", server.base, rssr_api::protocol::entry_read(id)))
        .header(COOKIE, &session)
        .header(ORIGIN, &server.base)
        .header(rssr_api::protocol::BUILD_HEADER, TEST_BUILD)
        .header(rssr_api::protocol::CSRF_HEADER, "stale")
        .json(&json!({ "read": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), reqwest::StatusCode::FORBIDDEN);
    assert_eq!(refused.json::<Value>().await.unwrap()["code"], "csrf_mismatch");
    assert_eq!(observed.times("set_entry_read"), 0, "a refused write runs nothing");

    let stale = http_backend(&server, Some(&session), TEST_BUILD).with_csrf_token("stale");
    stale.set_entry_read(id, true).await.unwrap();
    assert_eq!(observed.times("set_entry_read"), 1, "refused, then run once");

    let stale = http_backend(&server, Some(&session), TEST_BUILD).with_csrf_token("stale");
    let (read, starred) =
        tokio::join!(stale.set_entry_read(id, false), stale.set_entry_starred(id, true));
    read.unwrap();
    starred.unwrap();
    assert_eq!((observed.times("set_entry_read"), observed.times("set_entry_starred")), (2, 1));

    let fresh = http_backend(&server, Some(&session), TEST_BUILD);
    let (read, starred) =
        tokio::join!(fresh.set_entry_read(id, true), fresh.set_entry_starred(id, false));
    read.unwrap();
    starred.unwrap();
    assert_eq!((observed.times("set_entry_read"), observed.times("set_entry_starred")), (3, 2));

    let entry = fresh.load_reader_entry(id).await.unwrap().entry.unwrap();
    assert_eq!((entry.is_read, entry.is_starred), (true, false));
}

/// The server's limit on the reader's localization falling on its write-back: the answer is
/// `outcome_unknown`, and the write commits after it, so the answer must not read as "nothing
/// happened" (design review C4D-R1 B1; review C4A-01).
#[tokio::test]
async fn a_limit_falling_on_the_write_back_is_unknown_and_the_write_still_commits() {
    let feeds = FeedServer::start().await;
    let served = library(&feeds, &["b.xml"]).await;
    let id = entry_id(&served, "Rust tips").await;
    let observed = Observed::new(served, Some(Duration::from_secs(2)));
    let api = ApiState::with(observed.clone(), TEST_BUILD, Duration::from_millis(200));
    let server = Server::start_with(None, api).await;
    let session = server.session().await;
    let client = http_backend(&server, Some(&session), TEST_BUILD);

    let error = client.localize_entry_assets(id).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::OutcomeUnknown, "{error}");
    assert!(!observed.committed(), "the answer came before the write-back ended");
    until("the write-back's commit", || observed.committed()).await;
    assert_eq!(observed.times("localize_entry_assets"), 1);
}

/// One answer of the scripted server.
#[derive(Debug, Clone)]
enum Reply {
    /// A complete answer: status and JSON body.
    Json(u16, String),
    /// A complete answer that is not this protocol's: an HTML page.
    Html(u16),
    /// The head and part of a JSON body, then the connection closes.
    Truncated(String),
    /// The head and part of a JSON body, then nothing more for longer than the client waits.
    StallBody(String),
    /// A complete JSON answer without a declared length: it ends where the connection closes.
    Unsized(u16, String),
    /// The connection closes without an answer.
    Close,
    /// No answer for longer than the client waits.
    Stall,
}

/// A request the scripted server received.
#[derive(Debug, Clone)]
struct Seen {
    method: String,
    path: String,
    csrf: Option<String>,
}

type Script = Arc<dyn Fn(&Seen, usize) -> Reply + Send + Sync>;

/// A server that answers each request as `script` says, given the request and how many requests
/// for the same path came before it; it keeps what it received.
struct Scripted {
    base: Url,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Scripted {
    async fn start(script: impl Fn(&Seen, usize) -> Reply + Send + Sync + 'static) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let script: Script = Arc::new(script);
        let counts = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
        {
            let seen = Arc::clone(&seen);
            tokio::spawn(async move {
                while let Ok((stream, _)) = listener.accept().await {
                    let (seen, script, counts) =
                        (Arc::clone(&seen), Arc::clone(&script), Arc::clone(&counts));
                    tokio::spawn(answer(stream, seen, script, counts));
                }
            });
        }
        Self { base, seen }
    }

    fn client(&self) -> HttpBackend {
        let client = HttpBackend::native_client().no_proxy().build().unwrap();
        HttpBackend::new(self.base.clone(), TEST_BUILD, client)
            .with_timeouts(Timeouts {
                request: Duration::from_millis(500),
                poll: Duration::from_millis(500),
                localize: Duration::from_millis(500),
            })
            .with_poll_backoff(vec![Duration::from_millis(10); 5])
    }

    /// The requests received for paths starting with `prefix`.
    fn count(&self, method: &str, prefix: &str) -> usize {
        let seen = self.seen.lock().unwrap();
        seen.iter().filter(|seen| seen.method == method && seen.path.starts_with(prefix)).count()
    }

    fn csrf_tokens(&self, prefix: &str) -> Vec<Option<String>> {
        let seen = self.seen.lock().unwrap();
        seen.iter()
            .filter(|seen| seen.path.starts_with(prefix))
            .map(|seen| seen.csrf.clone())
            .collect()
    }
}

async fn answer(
    mut stream: tokio::net::TcpStream,
    seen: Arc<Mutex<Vec<Seen>>>,
    script: Script,
    counts: Arc<Mutex<HashMap<String, usize>>>,
) {
    let mut received = Vec::new();
    let mut buffer = [0_u8; 4096];
    let (head, body_start) = loop {
        let count = stream.read(&mut buffer).await.unwrap_or(0);
        if count == 0 {
            return;
        }
        received.extend_from_slice(&buffer[..count]);
        if let Some(end) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break (String::from_utf8_lossy(&received[..end]).into_owned(), end + 4);
        }
    };
    let header = |name: &str| {
        head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_owned())
        })
    };
    let length = header("content-length").and_then(|value| value.parse::<usize>().ok());
    while received.len() < body_start + length.unwrap_or(0) {
        let count = stream.read(&mut buffer).await.unwrap_or(0);
        if count == 0 {
            break;
        }
        received.extend_from_slice(&buffer[..count]);
    }
    let mut request_line = head.lines().next().unwrap_or_default().split(' ');
    let method = request_line.next().unwrap_or_default().to_owned();
    let path = request_line.next().unwrap_or_default().to_owned();
    let request = Seen { method, path, csrf: header(rssr_api::protocol::CSRF_HEADER) };
    let route = request.path.split('?').next().unwrap_or_default().to_owned();
    let index = {
        let mut counts = counts.lock().unwrap();
        let count = counts.entry(format!("{} {route}", request.method)).or_default();
        *count += 1;
        *count - 1
    };
    seen.lock().unwrap().push(request.clone());
    let write = |status: u16, kind: &str, body: &str, declared: Option<usize>| {
        let length = declared.map(|length| format!("Content-Length: {length}\r\n"));
        let length = length.unwrap_or_default();
        format!(
            "HTTP/1.1 {status} X\r\nContent-Type: {kind}\r\n{length}Connection: close\r\n\r\n\
             {body}"
        )
    };
    let json = "application/json";
    let (text, stall) = match script(&request, index) {
        Reply::Json(status, body) => (write(status, json, &body, Some(body.len())), false),
        Reply::Html(status) => {
            let body = "<html><body>Bad gateway</body></html>";
            (write(status, "text/html", body, Some(body.len())), false)
        }
        Reply::Truncated(body) => (write(200, json, &body, Some(body.len() + 100)), false),
        Reply::StallBody(body) => (write(200, json, &body, Some(body.len() + 100)), true),
        Reply::Unsized(status, body) => (write(status, json, &body, None), false),
        Reply::Close => return,
        Reply::Stall => {
            tokio::time::sleep(Duration::from_secs(5)).await;
            return;
        }
    };
    let _ = stream.write_all(text.as_bytes()).await;
    if stall {
        let _ = stream.flush().await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    let _ = stream.shutdown().await;
}

fn session_token(index: usize) -> Reply {
    Reply::Json(200, json!({ "csrf_token": format!("t{index}") }).to_string())
}

fn error_body(code: &str) -> String {
    json!({ "code": code, "message": code, "request_id": "r1" }).to_string()
}

const KIB: usize = 1024;

/// `json` followed by spaces up to `length` bytes: still the same JSON, only longer.
fn padded(json: String, length: usize) -> String {
    let padding = length.saturating_sub(json.len());
    json + &" ".repeat(padding)
}

const OPENED: &str = "/api/v1/workspace/opened-feed/1";
const SESSION: &str = "/api/v1/session";

/// A write sent without a complete, parsable answer may have taken effect: `outcome_unknown`, and
/// it is sent exactly once (phase C, "结果未知").
#[tokio::test]
async fn a_write_without_a_complete_answer_is_unknown_and_sent_once() {
    for reply in [
        Reply::Close,
        Reply::Stall,
        Reply::Truncated("nul".to_owned()),
        Reply::StallBody("nu".to_owned()),
        Reply::Html(502),
        Reply::Json(200, "\"not what was asked\"".to_owned()),
    ] {
        let server = Scripted::start({
            let reply = reply.clone();
            move |seen, index| match seen.path.as_str() {
                SESSION => session_token(index),
                _ => reply.clone(),
            }
        })
        .await;
        let error = server.client().remember_opened_feed(1).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::OutcomeUnknown, "{reply:?}: {error}");
        assert_eq!(server.count("PUT", OPENED), 1, "{reply:?}: sent once");
    }
}

/// A read without a complete answer is `network`; so is a write that never left (no server to
/// connect to, so no token and no write).
#[tokio::test]
async fn a_read_without_an_answer_and_a_write_that_never_left_are_network_errors() {
    for reply in [
        Reply::Close,
        Reply::Stall,
        Reply::Truncated("{".to_owned()),
        Reply::StallBody("{".to_owned()),
        Reply::Html(502),
    ] {
        let server = Scripted::start({
            let reply = reply.clone();
            move |_, _| reply.clone()
        })
        .await;
        let error = server.client().load_settings().await.unwrap_err();
        assert_eq!(error.code, ErrorCode::Network, "{reply:?}: {error}");
    }

    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = closed.local_addr().unwrap();
    drop(closed);
    let client = HttpBackend::new(
        Url::parse(&format!("http://{address}")).unwrap(),
        TEST_BUILD,
        HttpBackend::native_client().no_proxy().build().unwrap(),
    );
    assert_eq!(client.remember_opened_feed(1).await.unwrap_err().code, ErrorCode::Network);
}

/// The token's own refusal reaches the caller as it is, and no write is sent (design review
/// C4D-R2 R2-01).
#[tokio::test]
async fn a_refused_token_fetch_is_its_own_error_and_sends_no_write() {
    let server = Scripted::start(|seen, _| match seen.path.as_str() {
        SESSION => Reply::Json(401, error_body("unauthenticated")),
        _ => Reply::Json(200, "null".to_owned()),
    })
    .await;
    let error = server.client().remember_opened_feed(1).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Unauthenticated);
    assert_eq!(server.count("PUT", OPENED), 0);
}

/// A CSRF refusal (made before the server runs anything) gets a new token and one more attempt;
/// a second refusal is the answer.
#[tokio::test]
async fn a_csrf_refusal_is_retried_once_with_a_new_token() {
    let server = Scripted::start(|seen, index| match (seen.path.as_str(), index) {
        (SESSION, _) => session_token(index),
        (_, 0) => Reply::Json(403, error_body("csrf_mismatch")),
        _ => Reply::Json(200, "null".to_owned()),
    })
    .await;
    server.client().remember_opened_feed(1).await.unwrap();
    assert_eq!(server.count("GET", SESSION), 2);
    assert_eq!(server.csrf_tokens(OPENED), [Some("t0".to_owned()), Some("t1".to_owned())]);

    let server = Scripted::start(|seen, index| match seen.path.as_str() {
        SESSION => session_token(index),
        _ => Reply::Json(403, error_body("csrf_mismatch")),
    })
    .await;
    let error = server.client().remember_opened_feed(1).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::CsrfMismatch);
    assert_eq!(server.count("PUT", OPENED), 2, "one retry, no more");
}

const JOB_ID: &str = "0123456789abcdef0123456789abcdef";
const REFRESH_ALL: &str = "/api/v1/jobs/refresh-all";
const POLL: &str = "/api/v1/jobs/0123456789abcdef0123456789abcdef";

fn created() -> Reply {
    Reply::Json(202, json!({ "job_id": JOB_ID }).to_string())
}

fn status(status: &JobStatus) -> Reply {
    Reply::Json(200, serde_json::to_string(status).unwrap())
}

fn refreshed() -> RefreshAllExecutionOutcome {
    RefreshAllExecutionOutcome::bounded(0, 1, 1, Vec::new())
}

/// A job is polled until it ends: a failed poll (no answer, the server busy) is retried after a
/// wait, and a poll that answers starts the count of failures again (R2-06).
#[tokio::test]
async fn a_job_is_polled_until_it_ends() {
    let done = status(&JobStatus::Succeeded { result: JobResult::RefreshAll(refreshed()) });
    let server = Scripted::start(move |seen, index| match seen.path.split('?').next().unwrap() {
        SESSION => session_token(index),
        REFRESH_ALL => created(),
        _ => match index {
            0 => status(&JobStatus::Running),
            1..=5 => Reply::Close,
            6 => status(&JobStatus::Queued),
            7..=10 => Reply::Json(503, error_body("unavailable")),
            11 => Reply::Close,
            _ => done.clone(),
        },
    })
    .await;
    assert_eq!(server.client().refresh_all().await.unwrap(), refreshed());
    assert_eq!(server.count("POST", REFRESH_ALL), 1);
    assert_eq!(server.count("GET", POLL), 13);
}

/// Polls that keep failing end the wait as `outcome_unknown` after the first failure and one
/// retry per wait.
#[tokio::test]
async fn polls_that_keep_failing_end_as_unknown() {
    let server = Scripted::start(|seen, index| match seen.path.split('?').next().unwrap() {
        SESSION => session_token(index),
        REFRESH_ALL => created(),
        _ => Reply::Close,
    })
    .await;
    let error = server.client().refresh_all().await.unwrap_err();
    assert_eq!(error.code, ErrorCode::OutcomeUnknown);
    assert_eq!(server.count("GET", POLL), 6);
    assert_eq!(server.count("POST", REFRESH_ALL), 1, "never started again");
}

/// A lost status is `outcome_unknown`; a session that ended after the job was accepted is
/// `outcome_unknown` with the reason `unauthenticated`; a job's own failure is its error, not a
/// failed poll; a job created without an id, or ended with another kind of result, is
/// `outcome_unknown`, never a made-up success.
#[tokio::test]
async fn how_a_job_can_end() {
    let ending = |poll: Reply| {
        move |seen: &Seen, index| match seen.path.split('?').next().unwrap() {
            SESSION => session_token(index),
            REFRESH_ALL => created(),
            _ => poll.clone(),
        }
    };

    let server = Scripted::start(ending(Reply::Json(410, error_body("status_unavailable")))).await;
    let error = server.client().refresh_all().await.unwrap_err();
    assert_eq!((error.code, error.reason), (ErrorCode::OutcomeUnknown, None));

    let server = Scripted::start(ending(Reply::Json(401, error_body("unauthenticated")))).await;
    let error = server.client().refresh_all().await.unwrap_err();
    assert_eq!(
        (error.code, error.reason),
        (ErrorCode::OutcomeUnknown, Some(ErrorCode::Unauthenticated))
    );

    let failed = rssr_api::BackendError::new(ErrorCode::Unavailable, "刷新任务已中断");
    let server = Scripted::start(ending(status(&JobStatus::Failed { error: failed }))).await;
    let error = server.client().refresh_all().await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Unavailable);
    assert_eq!(server.count("GET", POLL), 1, "the job's failure is not retried");

    let other = JobResult::RemoteConfigPush(RemoteConfigPushOutcome { exported_feed_count: 1 });
    let server = Scripted::start(ending(status(&JobStatus::Succeeded { result: other }))).await;
    assert_eq!(server.client().refresh_all().await.unwrap_err().code, ErrorCode::OutcomeUnknown);

    let server = Scripted::start(|seen, index| match seen.path.split('?').next().unwrap() {
        SESSION => session_token(index),
        _ => Reply::Json(202, "\"no id here\"".to_owned()),
    })
    .await;
    assert_eq!(server.client().refresh_all().await.unwrap_err().code, ErrorCode::OutcomeUnknown);
    assert_eq!(server.count("GET", "/api/v1/jobs/"), 0, "nothing to poll");
}

/// A created job whose id is not of the protocol's form (32 lowercase hex digits) is not polled:
/// nothing to wait for, and such an id could name another path (review C4A-03).
#[tokio::test]
async fn a_job_id_not_of_the_protocols_form_is_never_polled() {
    for id in [
        "",
        "not hex",
        "0123456789ABCDEF0123456789ABCDEF",
        "0123456789abcdef0123456789abcde",
        "0123456789abcdef0123456789abcdef0",
        "../settings",
        "../../../api/v1/settings?",
    ] {
        let server =
            Scripted::start(move |seen, index| match seen.path.split('?').next().unwrap() {
                SESSION => session_token(index),
                REFRESH_ALL => Reply::Json(202, json!({ "job_id": id }).to_string()),
                _ => Reply::Json(200, "null".to_owned()),
            })
            .await;
        let error = server.client().refresh_all().await.unwrap_err();
        assert_eq!(error.code, ErrorCode::OutcomeUnknown, "{id:?}: {error}");
        assert_eq!(server.count("POST", REFRESH_ALL), 1, "{id:?}");
        assert_eq!(server.count("GET", "/"), server.count("GET", SESSION), "{id:?}: no poll");
    }
}

/// An answer longer than what it answers can be is not read, whether its length is declared or
/// only found while reading: a read is then `network` (a token never read sends no write), a
/// write `outcome_unknown`; a status that long is a failed poll; an error body that long is not
/// this protocol's error. Within its limit the same answer is read (review C4A-02).
#[tokio::test]
async fn answers_longer_than_their_limit_are_not_read() {
    for declared in [true, false] {
        let reply = move |status: u16, body: String| {
            if declared { Reply::Json(status, body) } else { Reply::Unsized(status, body) }
        };
        let token = |index: usize, length: usize| {
            padded(json!({ "csrf_token": format!("t{index}") }).to_string(), length)
        };

        let server = Scripted::start(move |seen, index| match seen.path.as_str() {
            SESSION => reply(200, token(index, 100 * KIB)),
            _ => Reply::Json(200, "null".to_owned()),
        })
        .await;
        let error = server.client().remember_opened_feed(1).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::Network, "declared {declared}: {error}");
        assert_eq!(server.count("PUT", OPENED), 0, "declared {declared}");

        let server = Scripted::start(move |seen, index| match seen.path.as_str() {
            SESSION => reply(200, token(index, 32 * KIB)),
            _ => Reply::Json(200, "null".to_owned()),
        })
        .await;
        server.client().remember_opened_feed(1).await.unwrap();

        let server =
            Scripted::start(move |seen, index| match seen.path.split('?').next().unwrap() {
                SESSION => session_token(index),
                REFRESH_ALL => {
                    reply(202, padded(json!({ "job_id": JOB_ID }).to_string(), 100 * KIB))
                }
                _ => status(&JobStatus::Running),
            })
            .await;
        let error = server.client().refresh_all().await.unwrap_err();
        assert_eq!(error.code, ErrorCode::OutcomeUnknown, "declared {declared}: {error}");
        assert_eq!((server.count("POST", REFRESH_ALL), server.count("GET", POLL)), (1, 0));

        let running = serde_json::to_string(&JobStatus::Running).unwrap();
        let server =
            Scripted::start(move |seen, index| match seen.path.split('?').next().unwrap() {
                SESSION => session_token(index),
                REFRESH_ALL => created(),
                _ => reply(200, padded(running.clone(), 600 * KIB)),
            })
            .await;
        let error = server.client().refresh_all().await.unwrap_err();
        assert_eq!(error.code, ErrorCode::OutcomeUnknown, "declared {declared}: {error}");
        assert_eq!(server.count("GET", POLL), 6, "declared {declared}");

        let server =
            Scripted::start(move |_, _| reply(503, padded(error_body("unavailable"), 100 * KIB)))
                .await;
        let error = server.client().load_settings().await.unwrap_err();
        assert_eq!(error.code, ErrorCode::Network, "declared {declared}: {error}");
    }
}

/// Giving up the wait (the page drops the call) during a poll or between polls sends no further
/// poll, and the job is never started again (design "C4a" 3; review C4A-01).
#[tokio::test]
async fn a_wait_given_up_sends_nothing_more() {
    for poll in [Reply::Close, Reply::Stall] {
        let server = Scripted::start({
            let poll = poll.clone();
            move |seen, index| match seen.path.split('?').next().unwrap() {
                SESSION => session_token(index),
                REFRESH_ALL => created(),
                _ => poll.clone(),
            }
        })
        .await;
        let client = server.client().with_poll_backoff(vec![Duration::from_millis(300); 5]);
        let waiting = tokio::spawn(async move { client.refresh_all().await });
        until("the first poll", || server.count("GET", POLL) == 1).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        waiting.abort();
        assert!(waiting.await.unwrap_err().is_cancelled(), "{poll:?}");
        // Unless given up, the next poll would come within 500 ms (poll limit) + 300 ms (wait).
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let counts = (server.count("POST", REFRESH_ALL), server.count("GET", POLL));
        assert_eq!(counts, (1, 1), "{poll:?}");
    }
}
