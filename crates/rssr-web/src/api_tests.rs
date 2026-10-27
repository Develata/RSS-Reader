//! The application's routes over real HTTP (v0.2 phase C, C3), behind the real session, Origin
//! and CSRF checks.
//!
//! Every route is compared with an independent [`LocalBackend`] over an identical library: both
//! subscribe to the same local feeds in the same order, then each operation runs on the server
//! through HTTP and on the second backend directly, and the answers must be the same JSON (times
//! of writes aside). Background jobs are started and polled over HTTP the same way. Then the
//! protocol's own rules: the build check, request ids, the bounds of parameters and bodies, the
//! operation and import slots, the mark-read bound, the localization time limit, and the jobs'
//! admission, ids and lost statuses.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Router, extract::Path, http::header::CONTENT_TYPE, response::IntoResponse, routing::get,
};
use reqwest::{Method, StatusCode, header};
use rssr_api::{
    AddSubscriptionOutcome, Backend,
    protocol::{self, JobId, routes},
};
use rssr_application::{EntriesBootstrapInput, SettingsPatch};
use rssr_domain::{
    EntriesWorkspaceState, EntryQuery, MAX_MARK_READ_SELECTION, MarkReadPreview, ReadFilter,
};
use rssr_infra::{
    local_backend::{LocalBackend, NativeHost, native_local_backend},
    store::LibraryStore,
};
use serde_json::{Value, json};

use crate::{
    api::ApiState,
    auth::CSRF_HEADER,
    http_tests::{Server, TEST_BUILD},
};

/// Local feeds and images, and a WebDAV store that keeps what is put.
pub(crate) struct FeedServer {
    pub(crate) base: String,
}

type DavFiles = Arc<Mutex<HashMap<String, String>>>;

fn webdav(files: DavFiles) -> axum::routing::MethodRouter {
    let put_files = Arc::clone(&files);
    get(move |Path(name): Path<String>| {
        let files = Arc::clone(&files);
        async move {
            match files.lock().unwrap().get(&name).cloned() {
                Some(body) => body.into_response(),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    })
    .put(move |Path(name): Path<String>, body: String| {
        let files = Arc::clone(&put_files);
        async move {
            files.lock().unwrap().insert(name, body);
            StatusCode::CREATED
        }
    })
}

impl FeedServer {
    pub(crate) async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let feed = |document: String| {
            get(move || {
                let document = document.clone();
                async move { ([(CONTENT_TYPE, "application/rss+xml")], document) }
            })
        };
        let image = format!(r#"<p>picture <img src="{base}/img.png"></p>"#);
        let stalled = format!(r#"<p>slow <img src="{base}/stall.png"></p>"#);
        let app = Router::new()
            .route(
                "/a.xml",
                feed(rss(
                    "Feed A",
                    vec![
                        item("a1", "Rust news", 1, "<p>first</p>"),
                        item("a2", "Image post", 2, &image),
                        item("a3", "Other", 3, "<p>third</p>"),
                    ],
                )),
            )
            .route(
                "/b.xml",
                feed(rss(
                    "Feed B",
                    vec![item("b1", "Rust tips", 4, "<p>tips</p>"), item("b2", "Misc", 5, "")],
                )),
            )
            .route("/c.xml", feed(rss("Feed C", vec![item("c1", "C", 6, "")])))
            .route("/d.xml", feed(rss("Feed D", vec![item("d1", "D", 7, "")])))
            .route("/slow.xml", feed(rss("Slow", vec![item("s1", "Slow", 8, &stalled)])))
            .route(
                "/many.xml",
                feed(rss(
                    "Many",
                    (0..=MAX_MARK_READ_SELECTION)
                        .map(|index| item(&format!("m{index}"), "Many", 9, ""))
                        .collect(),
                )),
            )
            .route("/dav/{name}", webdav(DavFiles::default()))
            .route(
                "/img.png",
                get(|| async { ([(CONTENT_TYPE, "image/png")], vec![1_u8, 2, 3, 4]) }),
            )
            .route(
                "/stall.png",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                    ""
                }),
            );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { base }
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}/{path}", self.base)
    }
}

/// A final job status's result, checked to be of `kind`.
#[track_caller]
fn outcome(status: &Value, kind: &str) -> Value {
    assert_eq!(status["state"], "succeeded", "{status}");
    assert_eq!(status["result"]["kind"], kind, "{status}");
    status["result"]["outcome"].clone()
}

fn rss(title: &str, items: Vec<String>) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/"><channel>
<title>{title}</title><link>https://example.com/</link><description>fixture</description>
{}
</channel></rss>"#,
        items.concat()
    )
}

fn item(guid: &str, title: &str, day: u32, content: &str) -> String {
    format!(
        "<item><guid>{guid}</guid><title>{title}</title><link>https://example.com/{guid}</link>\
         <pubDate>{day:02} Jun 2026 08:00:00 +0000</pubDate>\
         <content:encoded><![CDATA[{content}]]></content:encoded></item>"
    )
}

pub(crate) type Library = LocalBackend<NativeHost>;

/// A library subscribed to `paths`, one after the other (entry ids follow the order).
pub(crate) async fn library(feeds: &FeedServer, paths: &[&str]) -> Library {
    let backend = native_local_backend(LibraryStore::open_in_memory().await.unwrap());
    for path in paths {
        let outcome = backend.add_subscription(&feeds.url(path), None).await.unwrap();
        assert!(
            matches!(outcome, AddSubscriptionOutcome::SavedAndRefreshed),
            "{path}: {outcome:?}"
        );
    }
    backend
}

/// Waits until the refresh's background image localization has rewritten the entry titled
/// `title`, so both libraries are compared at rest.
pub(crate) async fn localized(library: &Library, title: &str) {
    let id = entry_id(library, title).await;
    for _ in 0..200 {
        let snapshot = library.load_reader_entry(id).await.unwrap();
        if serde_json::to_string(&snapshot).unwrap().contains("data:image/png") {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the background localization of {title} did not finish");
}

pub(crate) async fn entry_id(library: &Library, title: &str) -> i64 {
    let entries = library.list_entries(&EntryQuery::default()).await.unwrap().entries;
    entries.iter().find(|entry| entry.title == title).unwrap().id
}

/// A logged-in client of a server whose application routes run on `library`.
struct Client {
    server: Server,
    session: String,
    csrf: String,
}

impl Client {
    async fn start(api: ApiState) -> Self {
        let server = Server::start_with(None, api).await;
        let session = server.session().await;
        let csrf = server.csrf(&session).await;
        Self { server, session, csrf }
    }

    async fn over(library: Library) -> Self {
        Self::start(ApiState::with(Arc::new(library), TEST_BUILD, Duration::from_secs(60))).await
    }

    /// A request with the session, the trusted origin, the CSRF token and `build`.
    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        build: Option<&str>,
    ) -> reqwest::Response {
        let mut request = self
            .server
            .client
            .request(method, self.server.url(path))
            .header(header::COOKIE, &self.session)
            .header(header::ORIGIN, &self.server.base)
            .header(CSRF_HEADER, &self.csrf);
        if let Some(build) = build {
            request = request.header(protocol::BUILD_HEADER, build);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        request.send().await.unwrap()
    }

    /// A successful answer's JSON.
    async fn ok(&self, method: Method, path: &str, body: Option<Value>) -> Value {
        let response = self.send(method.clone(), path, body.as_ref(), Some(TEST_BUILD)).await;
        let status = response.status();
        let text = response.text().await.unwrap();
        assert_eq!(status, StatusCode::OK, "{method} {path}: {text}");
        serde_json::from_str(&text).unwrap()
    }

    async fn get(&self, path: &str) -> Value {
        self.ok(Method::GET, path, None).await
    }

    /// Starts a job; its id.
    async fn start_job(&self, path: &str, body: Option<Value>) -> JobId {
        let response = self.send(Method::POST, path, body.as_ref(), Some(TEST_BUILD)).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED, "{path}");
        let created: Value = response.json().await.unwrap();
        JobId::parse(created["job_id"].as_str().unwrap()).expect("a well-formed job id")
    }

    /// Starts a job and polls it to its final status.
    async fn job(&self, path: &str, body: Option<Value>) -> Value {
        let id = self.start_job(path, body).await;
        for _ in 0..30 {
            let status = self.get(&format!("{}?wait_ms=10000", protocol::job(&id))).await;
            if matches!(status["state"].as_str(), Some("succeeded" | "failed")) {
                return status;
            }
        }
        panic!("the job {path} did not finish");
    }

    /// An error answer: its status, code and message; the request id in the body is the one in
    /// the header.
    async fn error(&self, response: reqwest::Response) -> (StatusCode, String, String) {
        error_of(response).await
    }

    /// A request whose body stops after `sent` of `declared` bytes, written by hand: its
    /// answer's status and JSON, which must come within `within`.
    async fn partial(
        &self,
        path: &str,
        declared: usize,
        sent: &str,
        within: Duration,
    ) -> (u16, Value) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let address = self.server.base.trim_start_matches("http://");
        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        let head = format!(
            "POST {path} HTTP/1.1\r\nHost: {address}\r\nCookie: {}\r\nOrigin: {}\r\n\
             {CSRF_HEADER}: {}\r\n{}: {TEST_BUILD}\r\nContent-Type: application/json\r\n\
             Content-Length: {declared}\r\n\r\n{sent}",
            self.session,
            self.server.base,
            self.csrf,
            protocol::BUILD_HEADER,
        );
        stream.write_all(head.as_bytes()).await.unwrap();
        let mut answer = Vec::new();
        let read = async {
            let mut buffer = [0_u8; 4096];
            loop {
                let count = stream.read(&mut buffer).await.unwrap();
                answer.extend_from_slice(&buffer[..count]);
                let text = String::from_utf8_lossy(&answer);
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let length = head
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(str::to_owned)
                        })
                        .and_then(|value| value.trim().parse::<usize>().ok());
                    if count == 0 || length.is_some_and(|length| body.len() >= length) {
                        break;
                    }
                }
                assert!(count > 0, "the server closed without an answer");
            }
        };
        tokio::time::timeout(within, read).await.expect("an answer within the limit");
        let text = String::from_utf8_lossy(&answer).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").unwrap();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, serde_json::from_str(body).unwrap())
    }
}

/// An error answer: its status, code and message. It is not cached, and carries a request id,
/// the same in the header and the body.
async fn error_of(response: reqwest::Response) -> (StatusCode, String, String) {
    let status = response.status();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let header_id = response
        .headers()
        .get(protocol::REQUEST_ID_HEADER)
        .map(|value| value.to_str().unwrap().to_owned())
        .expect("a request id header");
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["request_id"].as_str(), Some(header_id.as_str()), "{body}");
    (status, body["code"].as_str().unwrap().to_owned(), body["message"].to_string())
}

fn with_json(route: &str, name: &str, value: &impl serde::Serialize) -> String {
    let json = serde_json::to_string(value).unwrap();
    format!("{route}?{name}={}", urlencoding::encode(&json))
}

/// Times written by the operations themselves differ between the two libraries.
pub(crate) fn at_rest(mut value: Value) -> Value {
    fn walk(value: &mut Value) {
        match value {
            Value::Object(fields) => {
                for (key, field) in fields.iter_mut() {
                    if key.ends_with("_at") && key != "published_at" {
                        *field = Value::Null;
                    } else {
                        walk(field);
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut value);
    value
}

#[track_caller]
fn same(via_http: Value, direct: impl serde::Serialize, what: &str) {
    let direct = serde_json::to_value(direct).unwrap();
    assert_eq!(at_rest(via_http), at_rest(direct), "{what}");
}

/// Every route answers what the backend answers, over the same library, in the same sequence.
#[tokio::test]
async fn every_route_answers_what_the_backend_answers() {
    let feeds = FeedServer::start().await;
    let served = library(&feeds, &["a.xml", "b.xml"]).await;
    let direct = library(&feeds, &["a.xml", "b.xml"]).await;
    localized(&served, "Image post").await;
    localized(&direct, "Image post").await;
    let client = Client::over(served).await;

    same(client.get(routes::SETTINGS).await, direct.load_settings().await.unwrap(), "settings");
    let patch = SettingsPatch {
        entries_page_size: Some(50),
        custom_css: Some("body { margin: 0 }".into()),
        ..Default::default()
    };
    same(
        client.ok(Method::PATCH, routes::SETTINGS, Some(json!(patch))).await,
        direct.update_settings(patch).await.unwrap(),
        "update settings",
    );
    same(
        client.get(routes::STARTUP_TARGET).await,
        direct.resolve_startup_target().await.unwrap(),
        "startup target",
    );
    let input = EntriesBootstrapInput { load_preferences: true, load_feeds: true };
    same(
        client.get(&with_json(routes::ENTRIES_BOOTSTRAP, "input", &input)).await,
        direct.bootstrap_entries(input).await.unwrap(),
        "bootstrap",
    );
    same(client.get(routes::FEEDS).await, direct.load_feeds_snapshot().await.unwrap(), "feeds");

    let all = EntryQuery::default();
    let feed_a = direct.load_feeds_snapshot().await.unwrap().feeds[0].id;
    let rust_in_a = EntryQuery {
        feed_id: Some(feed_a),
        search_title: Some("Rust".into()),
        ..Default::default()
    };
    for query in [&all, &rust_in_a] {
        same(
            client.get(&with_json(routes::ENTRIES, "query", query)).await,
            direct.list_entries(query).await.unwrap(),
            "entries",
        );
    }

    let image = entry_id(&direct, "Image post").await;
    let rust_tips = entry_id(&direct, "Rust tips").await;
    same(
        client
            .ok(Method::PUT, &protocol::entry_read(rust_tips), Some(json!({ "read": true })))
            .await,
        direct.set_entry_read(rust_tips, true).await.unwrap(),
        "set read",
    );
    same(
        client
            .ok(Method::PUT, &protocol::entry_starred(image), Some(json!({ "starred": true })))
            .await,
        direct.set_entry_starred(image, true).await.unwrap(),
        "set starred",
    );
    same(
        client.get(&protocol::entry(image)).await,
        direct.load_reader_entry(image).await.unwrap(),
        "reader entry",
    );
    same(
        client.ok(Method::POST, &protocol::entry_assets(image), None).await,
        direct.localize_entry_assets(image).await.unwrap(),
        "localize (already done in the background)",
    );

    let in_a = EntryQuery { feed_id: Some(feed_a), ..Default::default() };
    let preview = client.get(&with_json(routes::MARK_READ, "query", &in_a)).await;
    let direct_preview = direct.preview_mark_read(&in_a).await.unwrap();
    same(preview.clone(), &direct_preview, "mark-read preview");
    same(
        client.ok(Method::POST, routes::MARK_READ, Some(preview)).await,
        direct.mark_read_if_unchanged(&direct_preview).await.unwrap(),
        "mark read",
    );

    // An operation without a result answers `null`.
    direct.remember_opened_feed(feed_a).await.unwrap();
    assert_eq!(client.ok(Method::PUT, &protocol::opened_feed(feed_a), None).await, Value::Null);
    let workspace = EntriesWorkspaceState {
        read_filter: ReadFilter::UnreadOnly,
        show_archived: true,
        ..Default::default()
    };
    same(
        client.ok(Method::PUT, routes::WORKSPACE, Some(json!(workspace))).await,
        direct.save_entries_workspace_if_changed(workspace).await.unwrap(),
        "save workspace",
    );

    // Exports: the document is a JSON string; the configuration carries its export time.
    let exported = client.get(routes::CONFIG_JSON).await;
    let direct_exported = direct.export_config_json().await.unwrap();
    same(
        serde_json::from_str(exported.as_str().unwrap()).unwrap(),
        serde_json::from_str::<Value>(&direct_exported).unwrap(),
        "config export",
    );
    same(client.get(routes::CONFIG_OPML).await, direct.export_opml().await.unwrap(), "opml");

    // Imports: the configuration gains a subscription, the OPML another.
    let mut package: Value = serde_json::from_str(&direct_exported).unwrap();
    let mut added = package["feeds"][0].clone();
    added["url"] = json!(feeds.url("c.xml"));
    package["feeds"].as_array_mut().unwrap().push(added);
    let raw = package.to_string();
    let expected = direct.load_feeds_snapshot().await.unwrap().fingerprint;
    same(
        client
            .ok(Method::PUT, routes::CONFIG_JSON, Some(json!({ "raw": raw, "expected": expected })))
            .await,
        direct.import_config_json(&raw, &expected).await.unwrap(),
        "config import",
    );
    let opml = format!(
        r#"<?xml version="1.0"?><opml version="2.0"><head><title>t</title></head><body><outline type="rss" text="D" xmlUrl="{}"/></body></opml>"#,
        feeds.url("d.xml")
    );
    same(
        client.ok(Method::POST, routes::CONFIG_OPML, Some(json!({ "raw": opml }))).await,
        direct.import_opml(&opml).await.unwrap(),
        "opml import",
    );

    direct.remove_feed(feed_a).await.unwrap();
    assert_eq!(client.ok(Method::DELETE, &protocol::feed(feed_a), None).await, Value::Null);
    same(client.get(routes::FEEDS).await, direct.load_feeds_snapshot().await.unwrap(), "after");
    same(
        client.get(&with_json(routes::ENTRIES, "query", &all)).await,
        direct.list_entries(&all).await.unwrap(),
        "entries after",
    );
}

/// A write needs this build's identifier; reads do not. A refused write changes nothing.
#[tokio::test]
async fn writes_need_this_build() {
    let feeds = FeedServer::start().await;
    let client = Client::over(library(&feeds, &["b.xml"]).await).await;
    let entries = client.get(&with_json(routes::ENTRIES, "query", &EntryQuery::default())).await;
    let id = entries["entries"][0]["id"].as_i64().unwrap();
    let path = protocol::entry_read(id);
    for build in [None, Some("another-build"), Some("")] {
        let response = client.send(Method::PUT, &path, Some(&json!({ "read": true })), build).await;
        let (status, code, _) = client.error(response).await;
        assert_eq!((status, code.as_str()), (StatusCode::CONFLICT, "build_mismatch"), "{build:?}");
    }
    let response = client.send(Method::GET, &protocol::entry(id), None, None).await;
    assert_eq!(response.status(), StatusCode::OK, "a read without the identifier");
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["entry"]["is_read"], false, "the refused write changed nothing");

    let response = client.send(Method::GET, routes::SETTINGS, None, None).await;
    assert!(response.headers().contains_key(protocol::REQUEST_ID_HEADER));
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
}

/// Parameters and bodies are bounded and checked, with the larger bounds where a legal request
/// needs them.
#[tokio::test]
async fn parameters_and_bodies_are_bounded() {
    let feeds = FeedServer::start().await;
    let client = Client::over(library(&feeds, &["b.xml"]).await).await;
    let build = Some(TEST_BUILD);

    let long = EntryQuery {
        search_title: Some("x".repeat(protocol::MAX_QUERY_PARAMETER_BYTES)),
        ..Default::default()
    };
    for (path, expected) in [
        (with_json(routes::ENTRIES, "query", &long), (StatusCode::PAYLOAD_TOO_LARGE, "too_large")),
        (format!("{}?query=not-json", routes::ENTRIES), (StatusCode::BAD_REQUEST, "invalid_input")),
        (routes::ENTRIES.to_owned(), (StatusCode::BAD_REQUEST, "invalid_input")),
        (
            format!("{}?input=%7B%7D", routes::ENTRIES_BOOTSTRAP),
            (StatusCode::BAD_REQUEST, "invalid_input"),
        ),
        (format!("{}/abc", routes::ENTRIES), (StatusCode::BAD_REQUEST, "invalid_input")),
    ] {
        let (status, code, _) =
            client.error(client.send(Method::GET, &path, None, build).await).await;
        assert_eq!((status, code.as_str()), expected, "{path}");
    }

    let large = json!({ "selected_feed_urls": ["x".repeat(protocol::MAX_BODY_BYTES)] });
    let response = client.send(Method::PUT, routes::WORKSPACE, Some(&large), build).await;
    let (status, code, _) = client.error(response).await;
    assert_eq!((status, code.as_str()), (StatusCode::PAYLOAD_TOO_LARGE, "too_large"));
    let response = client.send(Method::PUT, routes::WORKSPACE, Some(&json!([1])), build).await;
    assert_eq!(client.error(response).await.1, "invalid_input");

    // The new workspace writes retain the same body and build gates as other mutations.
    for (method, path) in
        [(Method::POST, routes::ENTRY_BATCH.to_owned()), (Method::PATCH, protocol::feed(1))]
    {
        let oversized = json!({"padding": "x".repeat(protocol::MAX_BODY_BYTES)});
        let response = client.send(method.clone(), &path, Some(&oversized), build).await;
        assert_eq!(client.error(response).await.1, "too_large", "{path}");
        let response = client.send(method.clone(), &path, Some(&json!({})), build).await;
        assert_eq!(client.error(response).await.1, "invalid_input", "{path}");
        let response = client.send(method, &path, Some(&json!({})), Some("obsolete-build")).await;
        assert_eq!(client.error(response).await.1, "build_mismatch", "{path}");
    }

    // The largest legal custom CSS escapes to twice its size: over the default bound, accepted.
    let css = "\n".repeat(rssr_domain::MAX_CUSTOM_CSS_BYTES);
    let settings =
        client.ok(Method::PATCH, routes::SETTINGS, Some(json!({ "custom_css": css }))).await;
    assert_eq!(settings["custom_css"].as_str().unwrap().len(), rssr_domain::MAX_CUSTOM_CSS_BYTES);

    // A confirmation of the largest selection is accepted (the selection changed: none of these
    // ids exist); one id more is refused before the store.
    let mut forged = MarkReadPreview {
        query: EntryQuery::default(),
        unread_entry_ids: (1_000_000_000..).take(MAX_MARK_READ_SELECTION).collect(),
    };
    let outcome = client.ok(Method::POST, routes::MARK_READ, Some(json!(forged))).await;
    assert_eq!(outcome["type"], "selection_changed", "{outcome}");
    forged.unread_entry_ids.push(1);
    let response = client.send(Method::POST, routes::MARK_READ, Some(&json!(forged)), build).await;
    let (status, code, message) = client.error(response).await;
    assert_eq!((status, code.as_str()), (StatusCode::BAD_REQUEST, "invalid_input"));
    assert!(message.contains("范围过大"), "{message}");

    // Imports have their own bound: a document over the default one reaches the import (and is
    // refused there as not a package), one over the import bound does not.
    let padding = " ".repeat(2 * protocol::MAX_BODY_BYTES);
    let expected = client.get(routes::FEEDS).await["fingerprint"].clone();
    for (method, route, body) in [
        (Method::POST, routes::CONFIG_OPML, json!({ "raw": padding })),
        (Method::PUT, routes::CONFIG_JSON, json!({ "raw": padding, "expected": expected })),
    ] {
        let response = client.send(method.clone(), route, Some(&body), build).await;
        let (status, code, _) = client.error(response).await;
        assert_eq!((status, code.as_str()), (StatusCode::BAD_REQUEST, "invalid_input"), "{route}");

        let raw = " ".repeat(protocol::MAX_IMPORT_BODY_BYTES);
        let body = json!({ "raw": raw, "expected": expected });
        let response = client.send(method, route, Some(&body), build).await;
        let (status, code, _) = client.error(response).await;
        assert_eq!(
            (status, code.as_str()),
            (StatusCode::PAYLOAD_TOO_LARGE, "too_large"),
            "{route}"
        );
    }
}

/// More unread entries than one bulk mark-read covers: the preview is refused.
#[tokio::test]
async fn mark_read_over_the_bound_is_refused() {
    let feeds = FeedServer::start().await;
    let client = Client::over(library(&feeds, &["many.xml"]).await).await;
    let path = with_json(routes::MARK_READ, "query", &EntryQuery::default());
    let response = client.send(Method::GET, &path, None, None).await;
    let (status, code, message) = client.error(response).await;
    assert_eq!((status, code.as_str()), (StatusCode::BAD_REQUEST, "invalid_input"));
    assert!(message.contains("范围过大"), "{message}");
}

/// With every operation slot taken, operations are refused at once; with the import slot taken,
/// imports are, while other operations run.
#[tokio::test]
async fn operations_and_imports_are_bounded() {
    let library = native_local_backend(LibraryStore::open_in_memory().await.unwrap());
    let api = ApiState::with(Arc::new(library), TEST_BUILD, Duration::from_secs(60));
    let client = Client::start(api.clone()).await;

    let busy = api.exhaust_operations();
    let response = client.send(Method::GET, routes::SETTINGS, None, None).await;
    let (status, code, _) = client.error(response).await;
    assert_eq!((status, code.as_str()), (StatusCode::SERVICE_UNAVAILABLE, "unavailable"));
    drop(busy);
    client.get(routes::SETTINGS).await;

    let importing = api.exhaust_imports();
    let opml = json!({ "raw": "<opml version=\"2.0\"><body/></opml>" });
    let response =
        client.send(Method::POST, routes::CONFIG_OPML, Some(&opml), Some(TEST_BUILD)).await;
    let (status, code, _) = client.error(response).await;
    assert_eq!((status, code.as_str()), (StatusCode::SERVICE_UNAVAILABLE, "unavailable"));
    client.get(routes::SETTINGS).await;
    drop(importing);
    client.ok(Method::POST, routes::CONFIG_OPML, Some(opml)).await;
}

/// Unknown routes and methods answer the API's JSON; without a session, 401 first.
#[tokio::test]
async fn unknown_routes_and_methods_answer_json() {
    let library = native_local_backend(LibraryStore::open_in_memory().await.unwrap());
    let client = Client::over(library).await;
    let response = client
        .server
        .client
        .post(client.server.url(routes::SETTINGS))
        .header(header::ORIGIN, &client.server.base)
        .send()
        .await
        .unwrap();
    let (status, code, _) = error_of(response).await;
    assert_eq!((status, code.as_str()), (StatusCode::UNAUTHORIZED, "unauthenticated"));

    for (method, path) in [(Method::POST, routes::SETTINGS), (Method::GET, routes::WORKSPACE)] {
        let response = client.send(method.clone(), path, None, Some(TEST_BUILD)).await;
        let (status, code, _) = error_of(response).await;
        assert_eq!(
            (status, code.as_str()),
            (StatusCode::NOT_FOUND, "not_found"),
            "{method} {path}"
        );
    }

    // The authentication's refusals carry ids too (review C3-R1 F14).
    let untrusted = client
        .server
        .client
        .put(client.server.url(routes::WORKSPACE))
        .header(header::COOKIE, &client.session)
        .header(header::ORIGIN, "http://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(error_of(untrusted).await.1, "untrusted_origin");
    let no_token = client
        .server
        .client
        .put(client.server.url(routes::WORKSPACE))
        .header(header::COOKIE, &client.session)
        .header(header::ORIGIN, &client.server.base)
        .send()
        .await
        .unwrap();
    assert_eq!(error_of(no_token).await.1, "csrf_mismatch");
}

/// The reader's image localization is given up at the time limit, answering `outcome_unknown`
/// (the write-back could have fallen on the limit; here it had not started, so the body is
/// unchanged; C4 design review C4D-R1 B1).
#[tokio::test]
async fn a_slow_localization_is_given_up() {
    let feeds = FeedServer::start().await;
    let library = library(&feeds, &["slow.xml"]).await;
    let id = entry_id(&library, "Slow").await;
    let api = ApiState::with(Arc::new(library), TEST_BUILD, Duration::from_millis(300));
    let client = Client::start(api).await;
    let before = client.get(&protocol::entry(id)).await;

    let started = std::time::Instant::now();
    let response =
        client.send(Method::POST, &protocol::entry_assets(id), None, Some(TEST_BUILD)).await;
    let (status, code, _) = client.error(response).await;
    assert_eq!((status, code.as_str()), (StatusCode::GATEWAY_TIMEOUT, "outcome_unknown"));
    assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    assert_eq!(client.get(&protocol::entry(id)).await, before);
}

/// Every job answers what the backend answers, over the same library, in the same sequence; a
/// failed job carries the backend's error.
#[tokio::test]
async fn every_job_answers_what_the_backend_answers() {
    let feeds = FeedServer::start().await;
    let served = library(&feeds, &["a.xml"]).await;
    let direct = library(&feeds, &["a.xml"]).await;
    let client = Client::over(served).await;

    let status = client.job(routes::JOB_REFRESH_ALL, None).await;
    same(outcome(&status, "refresh_all"), direct.refresh_all().await.unwrap(), "refresh all");
    let feed_id = direct.load_feeds_snapshot().await.unwrap().feeds[0].id;
    let status = client.job(&protocol::job_refresh_feed(feed_id), None).await;
    same(outcome(&status, "refresh_feed"), direct.refresh_feed(feed_id).await.unwrap(), "refresh");

    let added = json!({ "raw_url": feeds.url("c.xml"), "fallback_site_url": null });
    let status = client.job(routes::JOB_ADD_SUBSCRIPTION, Some(added)).await;
    same(
        outcome(&status, "add_subscription"),
        direct.add_subscription(&feeds.url("c.xml"), None).await.unwrap(),
        "add subscription",
    );
    same(client.get(routes::FEEDS).await, direct.load_feeds_snapshot().await.unwrap(), "feeds");

    // Each side has its own remote file.
    let endpoint = feeds.url("dav/");
    for (kind, route) in [("push", routes::JOB_REMOTE_PUSH), ("pull", routes::JOB_REMOTE_PULL)] {
        let body = json!({ "endpoint": endpoint, "remote_path": "served.json" });
        let status = client.job(route, Some(body)).await;
        let expected = match kind {
            "push" => serde_json::to_value(
                direct.push_remote_config(&endpoint, "direct.json").await.unwrap(),
            ),
            _ => serde_json::to_value(
                direct.pull_remote_config(&endpoint, "direct.json").await.unwrap(),
            ),
        }
        .unwrap();
        same(outcome(&status, &format!("remote_config_{kind}")), expected, kind);
    }

    let status = client.job(&protocol::job_refresh_feed(999_999), None).await;
    let error = direct.refresh_feed(999_999).await.unwrap_err();
    assert_eq!(status["state"], "failed", "{status}");
    assert_eq!(status["error"]["code"], error.code.as_str());
    assert_eq!(status["error"]["message"], error.message);
    assert!(status["error"]["request_id"].is_string());
}

/// Starting a job is a write of this build; full admission refuses it; a status that is gone,
/// or an id that is not one, is `status_unavailable`; a restarted server knows no old job.
#[tokio::test]
async fn jobs_follow_the_protocol() {
    let library = native_local_backend(LibraryStore::open_in_memory().await.unwrap());
    let backend: crate::api::SharedBackend = Arc::new(library);
    let api = ApiState::with(Arc::clone(&backend), TEST_BUILD, Duration::from_secs(60));
    let client = Client::start(api.clone()).await;

    for build in [None, Some("another-build")] {
        let response = client.send(Method::POST, routes::JOB_REFRESH_ALL, None, build).await;
        let (status, code, _) = client.error(response).await;
        assert_eq!((status, code.as_str()), (StatusCode::CONFLICT, "build_mismatch"), "{build:?}");
    }
    let response = client.send(Method::GET, routes::JOB_REFRESH_ALL, None, Some(TEST_BUILD)).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let finished = client.job(routes::JOB_REFRESH_ALL, None).await;
    assert_eq!(outcome(&finished, "refresh_all")["total_count"], 0);
    let id = client.start_job(routes::JOB_REFRESH_ALL, None).await;
    let path = protocol::job(&id);
    let response = client.send(Method::GET, &format!("{path}?wait_ms=soon"), None, None).await;
    assert_eq!(client.error(response).await.1, "invalid_input");
    client.get(&format!("{path}?wait_ms=99999999")).await;

    let unknown = JobId::parse(&"0".repeat(32)).unwrap();
    for gone in [protocol::job(&unknown), "/api/v1/jobs/not-a-job".to_owned()] {
        let response = client.send(Method::GET, &gone, None, None).await;
        let (status, code, _) = client.error(response).await;
        assert_eq!((status, code.as_str()), (StatusCode::GONE, "status_unavailable"), "{gone}");
    }

    api.fill_jobs();
    let response = client.send(Method::POST, routes::JOB_REFRESH_ALL, None, Some(TEST_BUILD)).await;
    let (status, code, _) = client.error(response).await;
    assert_eq!((status, code.as_str()), (StatusCode::SERVICE_UNAVAILABLE, "unavailable"));

    let restarted = ApiState::with(backend, TEST_BUILD, Duration::from_secs(60));
    let client = Client::start(restarted).await;
    let response = client.send(Method::GET, &path, None, None).await;
    let (status, code, _) = client.error(response).await;
    assert_eq!((status, code.as_str()), (StatusCode::GONE, "status_unavailable"));
}

/// A data directory that does not exist yet is created (private on Unix) before the policy and
/// the library are written; the library is private (review C3-R1 F01, F02).
#[tokio::test]
async fn a_new_data_directory_is_created_before_anything_is_written() {
    let scratch = crate::auth::test_support::ScratchDir::new("fresh");
    let data_dir = scratch.path().join("not").join("yet");
    let (backend, library) =
        crate::open_backend(&data_dir).await.expect("starts on a new directory");
    drop(backend);
    library.close().await.expect("closed");
    for file in ["outbound.json", rssr_infra::store::local::LIBRARY_FILE] {
        assert!(data_dir.join(file).is_file(), "{file}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode();
        assert_eq!(mode(&data_dir) & 0o777, 0o700);
        let library = data_dir.join(rssr_infra::store::local::LIBRARY_FILE);
        assert_eq!(mode(&library) & 0o077, 0, "the library is its owner's alone");
    }
}

/// The slots are taken before a body is read: with the import slot taken, an import whose body
/// never completes is refused at once; a body that does not arrive within the receive limit is
/// refused and frees its slots (review C3-R1 F03).
#[tokio::test]
async fn bodies_are_read_only_with_their_slots_and_within_a_limit() {
    let library = native_local_backend(LibraryStore::open_in_memory().await.unwrap());
    let api = ApiState::with(Arc::new(library), TEST_BUILD, Duration::from_secs(60))
        .receiving_within(Duration::from_millis(300));
    let client = Client::start(api.clone()).await;
    let declared = 3 * 1024 * 1024;

    let importing = api.exhaust_imports();
    let (status, body) =
        client.partial(routes::CONFIG_OPML, declared, "{\"raw\":\"", Duration::from_secs(2)).await;
    assert_eq!((status, body["code"].as_str()), (503, Some("unavailable")), "{body}");
    drop(importing);

    let (status, body) =
        client.partial(routes::CONFIG_OPML, declared, "{\"raw\":\"", Duration::from_secs(5)).await;
    assert_eq!((status, body["code"].as_str()), (400, Some("invalid_input")), "{body}");
    let opml = json!({ "raw": "<opml version=\"2.0\"><body/></opml>" });
    client.ok(Method::POST, routes::CONFIG_OPML, Some(opml)).await;

    let busy = api.exhaust_operations();
    let (status, _) = client.partial(routes::MARK_READ, 1024, "{", Duration::from_secs(2)).await;
    assert_eq!(status, 503, "no operation slot: refused before the body");
    drop(busy);
}

/// A write that takes no body refuses one; a poll takes one of a bounded number of slots; a
/// parameter given twice is refused (review C3-R1 F13, F10).
#[tokio::test]
async fn writes_without_a_body_polls_and_parameters_are_checked() {
    let library = native_local_backend(LibraryStore::open_in_memory().await.unwrap());
    let api = ApiState::with(Arc::new(library), TEST_BUILD, Duration::from_secs(60));
    let client = Client::start(api.clone()).await;
    let body = json!({ "unexpected": true });
    for path in [routes::JOB_REFRESH_ALL.to_owned(), protocol::opened_feed(1)] {
        let method = if path == routes::JOB_REFRESH_ALL { Method::POST } else { Method::PUT };
        let response = client.send(method, &path, Some(&body), Some(TEST_BUILD)).await;
        assert_eq!(error_of(response).await.1, "invalid_input", "{path}");
    }

    let id = client.start_job(routes::JOB_REFRESH_ALL, None).await;
    let polls = api.exhaust_polls();
    let response = client.send(Method::GET, &protocol::job(&id), None, None).await;
    let (status, code, _) = error_of(response).await;
    assert_eq!((status, code.as_str()), (StatusCode::SERVICE_UNAVAILABLE, "unavailable"));
    drop(polls);
    client.get(&protocol::job(&id)).await;

    let query = serde_json::to_string(&EntryQuery::default()).unwrap();
    let twice = format!(
        "{}?query={}&query={}",
        routes::ENTRIES,
        urlencoding::encode(&query),
        urlencoding::encode(&query)
    );
    let response = client.send(Method::GET, &twice, None, None).await;
    assert_eq!(error_of(response).await.1, "invalid_input");
}
