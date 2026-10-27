#![cfg(not(target_arch = "wasm32"))]
//! The refresh/cache invariants of the 2026-10-09 audit, run against the v0.2 library store
//! (ported from `test_audit_20261009_refresh_invariants.rs`). Tests of that file that never
//! touch storage (client identification, GBK feed decoding, Atom link selection) are not
//! storage tests and stay there unchanged. See `docs/engineering-audit-2026-10.md`.
//!
//! The source hash of a body is read back from `entry_contents.content_hash` after the refresh
//! that stored the source: it is what the store itself computed for that source, so the tests
//! do not depend on the v0.1.x hashing helper that goes away with `crate::db`.

#[path = "support/library.rs"]
mod library;

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use library::Library;
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshAllInput, RefreshHttpMetadata, RefreshService, RefreshTarget,
};
use rssr_domain::{EntryContentRepository, EntryIndexRepository, EntryQuery, FeedRepository};
use rssr_infra::store::LocalizedBody;
use tokio::sync::Notify;
use url::Url;

const FEED_URL: &str = "https://example.com/feed.xml";
const ENTRY_URL: &str = "https://example.com/entry-1";

fn update(etag: &str, body_html: &str) -> FeedRefreshUpdate {
    update_at(etag, body_html, ENTRY_URL)
}

fn update_at(etag: &str, body_html: &str, entry_url: &str) -> FeedRefreshUpdate {
    FeedRefreshUpdate {
        metadata: RefreshHttpMetadata { etag: Some(etag.to_string()), last_modified: None },
        feed: ParsedFeedUpdate {
            title: Some("Feed".to_string()),
            site_url: None,
            description: None,
            entries: vec![ParsedEntryData {
                external_id: "entry-1".to_string(),
                dedup_key: "entry-1".to_string(),
                url: Some(Url::parse(entry_url).expect("entry url")),
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

fn service(library: &Library, update: FeedRefreshUpdate) -> RefreshService {
    RefreshService::new(Arc::new(RepeatSource(update)), Arc::new(library.refresh.clone()))
}

/// The source hash the store recorded for the (only) stored body.
async fn stored_source_hash(library: &Library) -> String {
    library
        .text("SELECT content_hash FROM entry_contents")
        .await
        .expect("the stored body carries a source hash")
}

async fn first_entry_id(library: &Library) -> i64 {
    library.entries.list_entries(&EntryQuery::default()).await.expect("list")[0].id
}

async fn stored_html(library: &Library, entry_id: i64) -> Option<String> {
    library.entries.get_content(entry_id).await.expect("content").expect("row").content_html
}

async fn write_localized(
    library: &Library,
    feed_id: i64,
    source_hash: Option<&str>,
    base_html: &str,
    localized_html: &str,
) -> bool {
    let entry_id = first_entry_id(library).await;
    let (_, generation) = library
        .entries
        .active_entry_generation(entry_id)
        .await
        .expect("generation")
        .expect("active");
    library
        .entries
        .update_localized_body(
            feed_id,
            generation,
            LocalizedBody {
                dedup_key: "entry-1".to_string(),
                expected_content_hash: source_hash.map(str::to_owned),
                base_html: Arc::from(base_html),
                localized_html: localized_html.to_string(),
            },
        )
        .await
        .expect("write localized body")
}

/// A derived cache (localized images) must survive a refresh whose *source* body is unchanged;
/// only a changed source may invalidate it.
#[tokio::test]
async fn unchanged_source_refresh_keeps_localized_body() {
    let library = Library::memory().await;
    let feed_id = library.add_feed(FEED_URL).await.id;
    let source_html = r#"<p><img src="https://cdn.example.com/a.png"></p>"#;
    let refresher = service(&library, update("v1", source_html));

    refresher.refresh_feed(feed_id).await.expect("first refresh");
    let entry_id = first_entry_id(&library).await;
    let source_hash = stored_source_hash(&library).await;
    let localized_html = r#"<p><img src="data:image/png;base64,AAAA"></p>"#;
    let derived = || {
        library.entries.has_derived_body(
            feed_id,
            "entry-1".to_string(),
            source_hash.clone(),
            Arc::from(source_html),
        )
    };
    assert!(!derived().await.expect("derived check"), "the stored body is still the source");
    assert!(
        write_localized(&library, feed_id, Some(&source_hash), source_html, localized_html).await
    );

    assert!(derived().await.expect("derived check"), "background localization must skip it");

    refresher.refresh_feed(feed_id).await.expect("unchanged-source refresh");

    let body = stored_html(&library, entry_id).await;
    assert_eq!(body.as_deref(), Some(localized_html), "localized cache was discarded");

    // A changed source still replaces the derived body.
    let changed_html = r#"<p>changed <img src="https://cdn.example.com/b.png"></p>"#;
    service(&library, update("v2", changed_html))
        .refresh_feed(feed_id)
        .await
        .expect("changed-source refresh");
    let body = stored_html(&library, entry_id).await;
    assert_eq!(body.as_deref(), Some(changed_html), "a changed source must replace the cache");
}

/// The background worker and the reader can localize the same source at the same time; the
/// source hash is the same for both. The slower task started from the source body and must not
/// replace the result the faster one already stored (independent review).
#[tokio::test]
async fn a_slower_localization_of_the_same_source_does_not_replace_a_finished_one() {
    let library = Library::memory().await;
    let feed_id = library.add_feed(FEED_URL).await.id;
    let source_html = r#"<p><img src="https://cdn.example.com/a.png"><img src="https://cdn.example.com/b.png"></p>"#;
    service(&library, update("v1", source_html)).refresh_feed(feed_id).await.expect("refresh");
    let source_hash = stored_source_hash(&library).await;
    let complete =
        r#"<p><img src="data:image/png;base64,AAAA"><img src="data:image/png;base64,BBBB"></p>"#;
    let partial =
        r#"<p><img src="data:image/png;base64,AAAA"><img src="https://cdn.example.com/b.png"></p>"#;

    assert!(write_localized(&library, feed_id, Some(&source_hash), source_html, complete).await);
    assert!(
        !write_localized(&library, feed_id, Some(&source_hash), source_html, partial).await,
        "a task that read the source body must not overwrite a stored derivation"
    );

    let body = stored_html(&library, first_entry_id(&library).await).await;
    assert_eq!(body.as_deref(), Some(complete));
}

/// Relative image addresses resolve against the article URL, so a body localized against the
/// old URL no longer belongs to the entry once only its URL changes (independent review).
#[tokio::test]
async fn an_article_url_change_invalidates_the_localized_body() {
    let library = Library::memory().await;
    let feed_id = library.add_feed(FEED_URL).await.id;
    let source_html = r#"<p><img src="cover.png"></p>"#;
    let refresh = |etag: &'static str, entry_url: &'static str| {
        service(&library, update_at(etag, source_html, entry_url))
    };

    refresh("v1", "https://example.com/a/post").refresh_feed(feed_id).await.expect("refresh");
    let source_hash = stored_source_hash(&library).await;
    let localized = r#"<p><img src="data:image/png;base64,AAAA"></p>"#;
    assert!(write_localized(&library, feed_id, Some(&source_hash), source_html, localized).await);

    refresh("v2", "https://example.com/b/post").refresh_feed(feed_id).await.expect("refresh");

    let body = stored_html(&library, first_entry_id(&library).await).await;
    assert_eq!(body.as_deref(), Some(source_html), "the body localized for the old URL was kept");
}

/// Bodies stored by older versions may have no source hash (NULL). The reader localizes such a
/// body by expecting the NULL it read; a recomputed hash never matches it, so the downloaded
/// images were thrown away (independent review).
#[tokio::test]
async fn a_body_without_a_source_hash_can_still_be_localized() {
    let library = Library::memory().await;
    let feed_id = library.add_feed(FEED_URL).await.id;
    let source_html = r#"<p><img src="https://cdn.example.com/a.png"></p>"#;
    service(&library, update("v1", source_html)).refresh_feed(feed_id).await.expect("refresh");
    let recomputed = stored_source_hash(&library).await;
    library.exec("UPDATE entry_contents SET content_hash = NULL").await;
    let localized = r#"<p><img src="data:image/png;base64,AAAA"></p>"#;

    assert!(!write_localized(&library, feed_id, Some(&recomputed), source_html, localized).await);
    assert!(write_localized(&library, feed_id, None, source_html, localized).await);

    let body = stored_html(&library, first_entry_id(&library).await).await;
    assert_eq!(body.as_deref(), Some(localized));
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
async fn the_fetch_that_started_last_owns_the_stored_result() {
    let library = Library::memory().await;
    let feed_id = library.add_feed(FEED_URL).await.id;
    let source = Arc::new(OrderedSource {
        calls: AtomicUsize::new(0),
        first_started: Notify::new(),
        release_first: Notify::new(),
    });
    let service = RefreshService::new(source.clone(), Arc::new(library.refresh.clone()));

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
    let stored = library.feeds.get_feed(feed_id).await.expect("feed").expect("active");
    let body = stored_html(&library, first_entry_id(&library).await).await;
    assert_eq!(stored.etag.as_deref(), Some(expected_etag), "stale validators won");
    assert_eq!(body.as_deref(), Some(expected_body), "stale body won");
}
