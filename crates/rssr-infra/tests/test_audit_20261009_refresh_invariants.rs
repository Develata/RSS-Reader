#![cfg(not(target_arch = "wasm32"))]
//! Deterministic regressions for refresh/cache defects found by the 2026-10-09 audit.
//! Each test states the invariant that should hold; `#[ignore]` marks a defect that is still
//! open. Run them with `cargo test -p rssr-infra --test test_audit_20261009_refresh_invariants
//! -- --ignored`. See `docs/engineering-audit-2026-10.md`.

#[path = "support/request_capture.rs"]
mod request_capture;

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use request_capture::{Response, Server};
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshAllInput, RefreshHttpMetadata, RefreshService, RefreshTarget,
};
use rssr_domain::{EntryQuery, FeedRepository, NewFeedSubscription};
use rssr_infra::{
    application_adapters::SqliteRefreshStore,
    db::{
        SqlitePool,
        entry_repository::{
            LocalizedEntryUpdate, SqliteEntryRepository, compute_entry_content_hash,
        },
        feed_repository::SqliteFeedRepository,
        migrate,
        sqlite_native::NativeSqliteBackend,
        storage_backend::StorageBackend,
    },
    fetch::{FetchClient, FetchRequest, FetchResult},
    parser::FeedParser,
};
use tokio::sync::Notify;
use url::Url;

async fn memory_pool() -> SqlitePool {
    let pool = NativeSqliteBackend::new("sqlite::memory:").connect().await.expect("connect");
    migrate(&pool).await.expect("migrate");
    pool
}

async fn seed_feed(feeds: &SqliteFeedRepository) -> i64 {
    feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/feed.xml").expect("feed url"),
            title: None,
            folder: None,
        })
        .await
        .expect("seed feed")
        .id
}

fn update(etag: &str, body_html: &str) -> FeedRefreshUpdate {
    FeedRefreshUpdate {
        metadata: RefreshHttpMetadata { etag: Some(etag.to_string()), last_modified: None },
        feed: ParsedFeedUpdate {
            title: Some("Feed".to_string()),
            site_url: None,
            description: None,
            entries: vec![ParsedEntryData {
                external_id: "entry-1".to_string(),
                dedup_key: "entry-1".to_string(),
                url: Some(Url::parse("https://example.com/entry-1").expect("entry url")),
                title: "Entry".to_string(),
                author: None,
                summary: Some("summary".to_string()),
                content_html: Some(body_html.to_string()),
                content_text: Some("summary".to_string()),
                published_at: None,
                updated_at_source: None,
            }],
        },
    }
}

struct RepeatSource(FeedRefreshUpdate);

#[async_trait::async_trait]
impl FeedRefreshSourcePort for RepeatSource {
    async fn refresh(&self, _target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
        Ok(FeedRefreshSourceOutput::Updated(self.0.clone()))
    }
}

/// A derived cache (localized images) must survive a refresh whose *source* body is unchanged;
/// only a changed source may invalidate it.
#[tokio::test]
#[ignore = "audit-2026-10-09: an unchanged-source refresh overwrites the localized body because content_hash identifies the derived HTML, not the source"]
async fn unchanged_source_refresh_keeps_localized_body() {
    let pool = memory_pool().await;
    let feeds = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entries = Arc::new(SqliteEntryRepository::new(pool));
    let feed_id = seed_feed(&feeds).await;
    let source_html = r#"<p><img src="https://cdn.example.com/a.png"></p>"#;
    let service = RefreshService::new(
        Arc::new(RepeatSource(update("v1", source_html))),
        Arc::new(SqliteRefreshStore::new(feeds.clone(), entries.clone())),
    );

    service.refresh_feed(feed_id).await.expect("first refresh");
    let entry_id = entries.list_entries(&EntryQuery::default()).await.expect("list")[0].id;
    let (_, generation) =
        entries.active_entry_generation(entry_id).await.expect("generation").expect("active");
    let localized_html = r#"<p><img src="data:image/png;base64,AAAA"></p>"#;
    let source_hash =
        compute_entry_content_hash(Some(source_html), Some("summary"), Some("Entry")).unwrap();
    let localized_hash =
        compute_entry_content_hash(Some(localized_html), Some("summary"), Some("Entry")).unwrap();
    assert!(
        entries
            .update_localized_html_if_hash_matches_for_generation(
                feed_id,
                generation,
                &LocalizedEntryUpdate {
                    dedup_key: "entry-1",
                    expected_content_hash: &source_hash,
                    localized_html,
                    localized_content_hash: &localized_hash,
                },
            )
            .await
            .expect("write localized body")
    );

    service.refresh_feed(feed_id).await.expect("unchanged-source refresh");

    let body = entries.get_content(entry_id).await.expect("content").expect("row").content_html;
    assert_eq!(body.as_deref(), Some(localized_html), "localized cache was discarded");
}

/// Two refreshes of the same feed (single-feed refresh vs refresh-all) are not coordinated.
///
/// Strategy-neutral invariant: the stored result belongs to the fetch that *started last*.
/// A single-flight fix (the second request joins the in-flight one, so the source runs once)
/// and a commit-sequence guard (the stale commit is rejected) both satisfy it. Today the newer
/// refresh commits first and the older response, released afterwards, overwrites it.
struct OrderedSource {
    calls: AtomicUsize,
    first_started: Notify,
    release_first: Notify,
}

#[async_trait::async_trait]
impl FeedRefreshSourcePort for OrderedSource {
    async fn refresh(&self, _target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.first_started.notify_one();
            self.release_first.notified().await;
            Ok(FeedRefreshSourceOutput::Updated(update("v1-older", "<p>older body</p>")))
        } else {
            Ok(FeedRefreshSourceOutput::Updated(update("v2-newer", "<p>newer body</p>")))
        }
    }
}

/// Every wait is bounded: no coordination strategy may hang the test.
const WATCHDOG: Duration = Duration::from_secs(10);
/// Grace for a newer request that joined the in-flight refresh instead of fetching. Uncoordinated
/// code finishes the newer refresh well within it, so the release order below stays fixed.
const JOIN_GRACE: Duration = Duration::from_secs(2);

async fn within<T>(what: &str, future: impl Future<Output = T>) -> T {
    tokio::time::timeout(WATCHDOG, future)
        .await
        .unwrap_or_else(|_| panic!("watchdog: {what} did not finish"))
}

#[tokio::test]
#[ignore = "audit-2026-10-09: refresh_feed and refresh_all of the same feed run concurrently; the older response, committed last, overwrites the newer one"]
async fn the_fetch_that_started_last_owns_the_stored_result() {
    let pool = memory_pool().await;
    let feeds = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entries = Arc::new(SqliteEntryRepository::new(pool));
    let feed_id = seed_feed(&feeds).await;
    let source = Arc::new(OrderedSource {
        calls: AtomicUsize::new(0),
        first_started: Notify::new(),
        release_first: Notify::new(),
    });
    let service = RefreshService::new(
        source.clone(),
        Arc::new(SqliteRefreshStore::new(feeds.clone(), entries.clone())),
    );

    let older = tokio::spawn({
        let service = service.clone();
        async move { service.refresh_feed(feed_id).await }
    });
    within("first fetch to start", source.first_started.notified()).await;
    let mut newer = tokio::spawn({
        let service = service.clone();
        async move { service.refresh_all(RefreshAllInput { max_concurrency: 1 }).await }
    });
    // Release the older fetch once the newer request has finished, or after the grace period if
    // it is waiting on the in-flight refresh (single-flight).
    let newer_finished_first = tokio::select! {
        result = &mut newer => Some(result),
        _ = tokio::time::sleep(JOIN_GRACE) => None,
    };
    source.release_first.notify_one();
    let newer_result = match newer_finished_first {
        Some(result) => result,
        None => within("newer refresh", newer).await,
    };
    newer_result.expect("join newer refresh task").expect("newer refresh");
    // A sequence guard may legitimately reject the stale commit, so only completion is required.
    let _ = within("older refresh", older).await.expect("join older refresh task");

    let (expected_etag, expected_body) = match source.calls.load(Ordering::SeqCst) {
        1 => ("v1-older", "<p>older body</p>"),
        2 => ("v2-newer", "<p>newer body</p>"),
        calls => panic!("unexpected source call count {calls}"),
    };
    let stored = feeds.get_feed(feed_id).await.expect("feed").expect("active");
    let entry_id = entries.list_entries(&EntryQuery::default()).await.expect("list")[0].id;
    let body = entries.get_content(entry_id).await.expect("content").expect("row").content_html;
    assert_eq!(stored.etag.as_deref(), Some(expected_etag), "stale validators won");
    assert_eq!(body.as_deref(), Some(expected_body), "stale body won");
}

/// RFC 9110 §10.1.5: a user agent SHOULD send User-Agent unless configured not to. Feed refresh,
/// subscription discovery and WebDAV identify the client the same way (image localization keeps
/// its own browser-compatible User-Agent).
#[tokio::test]
async fn feed_fetch_identifies_the_client() {
    use rssr_application::SubscriptionProbePort;
    use rssr_infra::{
        config_sync::webdav::WebDavConfigSync, subscription_probe::HttpSubscriptionProbe,
    };

    let server = Server::start(|request| match request.path.as_str() {
        "/feed.xml" => Response::new(
            200,
            "Content-Type: application/rss+xml\r\n",
            r#"<rss version="2.0"><channel><title>T</title></channel></rss>"#,
        ),
        _ => Response::new(404, "", []),
    })
    .await;
    let feed_url = server.url.join("feed.xml").unwrap();
    FetchClient::new()
        .fetch(&FetchRequest { url: feed_url.to_string(), etag: None, last_modified: None })
        .await
        .expect("fetch feed");
    HttpSubscriptionProbe::with_feed_client().probe(&feed_url).await.expect("probe feed");
    let remote = WebDavConfigSync::new(server.url.as_str(), "dav/state.json").expect("webdav");
    assert_eq!(remote.download_text().await.expect("webdav download"), None);

    let requests = server.requests();
    let seen = requests
        .iter()
        .map(|request| (request.method.as_str(), request.path.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(seen, [("GET", "/feed.xml"), ("GET", "/feed.xml"), ("GET", "/dav/state.json")]);
    for request in &requests {
        let user_agent = request.header("user-agent").unwrap_or_default();
        assert!(user_agent.starts_with("RSS-Reader/"), "{} {user_agent:?}", request.path);
    }
}

/// Serves a GBK-encoded feed whose XML prolog declares `encoding="GBK"`, then fetches and parses
/// it through the production transport and parser.
async fn fetch_and_parse_gbk_feed(
    content_type_header: &'static str,
) -> rssr_infra::parser::ParsedFeed {
    let xml = r#"<?xml version="1.0" encoding="GBK"?><rss version="2.0"><channel><title>中文标题</title><item><title>条目一</title><description>正文</description><guid>g1</guid></item></channel></rss>"#;
    let (bytes, _, _) = encoding_rs::GBK.encode(xml);
    let body = bytes.into_owned();
    let server =
        Server::start(move |_| Response::new(200, content_type_header, body.clone())).await;
    let FetchResult::Fetched { body, .. } = FetchClient::new()
        .fetch(&FetchRequest {
            url: server.url.join("gbk.xml").unwrap().to_string(),
            etag: None,
            last_modified: None,
        })
        .await
        .expect("fetch feed")
    else {
        panic!("expected a body");
    };
    FeedParser::new().parse(&body).expect("parse feed")
}

/// XML may declare its encoding only in the prolog (no charset parameter in Content-Type).
/// Decoding the bytes as UTF-8 before the XML parser sees them destroys GBK/Big5/Shift_JIS text.
#[tokio::test]
async fn prolog_declared_gbk_feed_keeps_its_text() {
    let parsed = fetch_and_parse_gbk_feed("Content-Type: application/rss+xml\r\n").await;
    assert_eq!(parsed.title.as_deref(), Some("中文标题"));
    assert_eq!(parsed.entries[0].title, "条目一");
}

/// With both an HTTP charset and a matching prolog declaration, the transport transcodes to UTF-8
/// but the parser still honours the stale `encoding="GBK"` declaration and decodes a second time.
#[tokio::test]
async fn http_and_prolog_both_gbk_feed_keeps_its_text() {
    let parsed =
        fetch_and_parse_gbk_feed("Content-Type: application/rss+xml; charset=GBK\r\n").await;
    assert_eq!(parsed.title.as_deref(), Some("中文标题"));
    assert_eq!(parsed.entries[0].title, "条目一");
}

/// Atom lists links by relation. The feed's site is `rel="alternate"` (not `rel="self"`), and an
/// entry's article is `rel="alternate"` (not `replies`/`enclosure`), regardless of order.
#[test]
#[ignore = "audit-2026-10-09: normalization takes the first <link> regardless of rel"]
fn atom_links_are_selected_by_relation_not_position() {
    let parsed = FeedParser::new()
        .parse(
            r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>F</title><id>f</id>
 <link rel="self" href="https://example.com/feed.atom"/>
 <link rel="alternate" href="https://example.com/"/>
 <entry><id>e1</id><title>E</title><summary>S</summary>
  <link rel="replies" href="https://example.com/e1/comments"/>
  <link rel="alternate" href="https://example.com/e1"/>
 </entry></feed>"#,
        )
        .expect("parse atom");
    assert_eq!(parsed.site_url.as_ref().map(Url::as_str), Some("https://example.com/"));
    assert_eq!(parsed.entries[0].url.as_ref().map(Url::as_str), Some("https://example.com/e1"));
}
