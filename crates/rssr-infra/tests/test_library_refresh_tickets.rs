#![cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]

//! Persistent refresh tickets (docs/design/v0.2-architecture.md, 存储): refreshes of one
//! subscription from different processes sharing a library never overwrite a newer result with
//! an older one. Two `LibraryStore` handles on one file stand in for two processes.

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[path = "support/library.rs"]
mod library;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use library::Library;
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshCommit, RefreshFailure, RefreshFeedResult, RefreshHttpMetadata,
    RefreshService, RefreshStorePort, RefreshTarget, RefreshTicket, SubscriptionActivationPort,
};
use rssr_domain::{EntryIndexRepository, EntryQuery, Feed, FeedRepository};
use rssr_infra::store::LibraryRefreshStore;
#[cfg(not(target_arch = "wasm32"))]
use rssr_infra::store::LibraryStore;

#[cfg(not(target_arch = "wasm32"))]
struct TempDir(PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl TempDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("rssr-tickets-{label}-{nonce}"));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Two "processes" on one library file, with one subscription.
#[cfg(not(target_arch = "wasm32"))]
async fn two_processes(label: &str) -> (TempDir, Library, Library, Feed) {
    let dir = TempDir::new(label);
    let path = dir.0.join("library.sqlite3");
    let a = Library::over(LibraryStore::open(&path).await.unwrap());
    let b = Library::over(LibraryStore::open(&path).await.unwrap());
    let feed = a.add_feed("https://example.com/feed.xml").await;
    (dir, a, b, feed)
}

// The same ordering cases use two port sets over the page's single connection. Native
// still verifies two independent store instances sharing a real file.
#[cfg(target_arch = "wasm32")]
async fn two_processes(_label: &str) -> ((), Library, Library, Feed) {
    let first = Library::memory().await;
    let second = Library::over(first.store.clone());
    let feed = first.add_feed("https://example.com/feed.xml").await;
    ((), first, second, feed)
}

fn metadata(etag: &str) -> RefreshHttpMetadata {
    RefreshHttpMetadata { etag: Some(etag.into()), last_modified: None }
}

fn fetched(title: &str, etag: &str, keys: &[&str]) -> RefreshCommit {
    RefreshCommit::Updated {
        update: FeedRefreshUpdate {
            metadata: metadata(etag),
            feed: ParsedFeedUpdate {
                title: Some(title.into()),
                site_url: None,
                description: None,
                entries: keys.iter().map(|key| library::entry(key)).collect(),
            },
        },
    }
}

fn failed(message: &str) -> RefreshCommit {
    RefreshCommit::Failed { failure: RefreshFailure::fetch(message, None) }
}

async fn begin(library: &Library, feed_id: i64) -> RefreshTarget {
    library.refresh.begin_refresh(feed_id, None).await.unwrap().expect("active subscription")
}

async fn superseded(library: &Library, target: &RefreshTarget, commit: RefreshCommit) -> bool {
    library.refresh.commit(target, commit).await.unwrap().superseded
}

/// (I, P, D), after checking `0 <= P <= D <= I`.
async fn tickets(library: &Library, feed_id: i64) -> (i64, i64, i64) {
    let issued =
        library.scalar(format!("SELECT refresh_issued FROM feeds WHERE id = {feed_id}")).await;
    let published =
        library.scalar(format!("SELECT refresh_published FROM feeds WHERE id = {feed_id}")).await;
    let diagnosed =
        library.scalar(format!("SELECT refresh_diagnosed FROM feeds WHERE id = {feed_id}")).await;
    assert!(0 <= published && published <= diagnosed && diagnosed <= issued);
    (issued, published, diagnosed)
}

async fn entry_titles(library: &Library) -> Vec<String> {
    let mut titles: Vec<String> = library
        .entries
        .list_entries(&EntryQuery::default())
        .await
        .unwrap()
        .into_iter()
        .map(|entry| entry.title)
        .collect();
    titles.sort();
    titles
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn tickets_are_issued_in_fetch_start_order_with_the_published_baseline() {
    let (_dir, a, b, feed) = two_processes("order").await;
    let first = begin(&a, feed.id).await;
    let second = begin(&b, feed.id).await;
    assert_eq!(first.ticket, RefreshTicket { issued: 1, baseline: 0 });
    assert_eq!(second.ticket, RefreshTicket { issued: 2, baseline: 0 });
    assert!(!superseded(&b, &second, fetched("B", "etag-b", &["b"])).await);
    let third = begin(&a, feed.id).await;
    assert_eq!(third.ticket, RefreshTicket { issued: 3, baseline: 2 });
    assert_eq!(tickets(&a, feed.id).await, (3, 2, 2));
}

/// A response fetched by a refresh that started earlier does not replace a newer publication.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_late_fetched_feed_is_superseded_by_a_newer_publication() {
    let (_dir, a, b, feed) = two_processes("late-200").await;
    let older = begin(&a, feed.id).await;
    let newer = begin(&b, feed.id).await;
    assert!(!superseded(&b, &newer, fetched("Newer", "etag-new", &["new"])).await);
    assert!(superseded(&a, &older, fetched("Older", "etag-old", &["old"])).await);

    assert_eq!(entry_titles(&a).await, ["new"]);
    let stored = a.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(stored.source_title.as_deref(), Some("Newer"));
    assert_eq!(stored.etag.as_deref(), Some("etag-new"));
    assert_eq!(tickets(&a, feed.id).await, (2, 2, 2));
}

/// A 304 confirms the version whose validators were sent; after a newer publication it confirms
/// nothing and is discarded.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_304_for_an_older_version_is_discarded() {
    let (_dir, a, b, feed) = two_processes("stale-304").await;
    let initial = begin(&a, feed.id).await;
    assert!(!superseded(&a, &initial, fetched("V1", "etag-1", &["v1"])).await);

    let conditional = begin(&a, feed.id).await;
    assert_eq!(conditional.etag.as_deref(), Some("etag-1"));
    let newer = begin(&b, feed.id).await;
    assert!(!superseded(&b, &newer, fetched("V2", "etag-2", &["v2"])).await);
    let not_modified = RefreshCommit::NotModified { metadata: metadata("etag-1") };
    assert!(superseded(&a, &conditional, not_modified).await);

    let stored = a.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(stored.etag.as_deref(), Some("etag-2"), "the newer validators stay");
    assert_eq!(tickets(&a, feed.id).await, (3, 3, 3));
}

/// A 304 that confirms the published version is recorded and becomes the published ticket.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_304_for_the_published_version_is_recorded() {
    let (_dir, a, _b, feed) = two_processes("current-304").await;
    let initial = begin(&a, feed.id).await;
    assert!(!superseded(&a, &initial, fetched("V1", "etag-1", &["v1"])).await);
    let conditional = begin(&a, feed.id).await;
    let not_modified = RefreshCommit::NotModified {
        metadata: RefreshHttpMetadata { etag: None, last_modified: Some("lm".into()) },
    };
    assert!(!superseded(&a, &conditional, not_modified).await);
    let stored = a.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(stored.etag.as_deref(), Some("etag-1"), "a validator the 304 omits survives");
    assert_eq!(stored.last_modified.as_deref(), Some("lm"));
    assert_eq!(tickets(&a, feed.id).await, (2, 2, 2));
}

/// An older failure arriving after a newer success is not recorded over it.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_stale_failure_does_not_hide_a_newer_success() {
    let (_dir, a, b, feed) = two_processes("stale-failure").await;
    let older = begin(&a, feed.id).await;
    let newer = begin(&b, feed.id).await;
    assert!(!superseded(&b, &newer, fetched("Fine", "etag", &["fine"])).await);
    assert!(superseded(&a, &older, failed("timeout")).await);

    let stored = a.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(stored.fetch_error, None);
    assert_eq!(tickets(&a, feed.id).await, (2, 2, 2));
}

/// An older success arriving after a newer failure publishes its content and validators, but the
/// newer failure stays the current diagnosis.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn an_older_success_after_a_newer_failure_keeps_the_newer_diagnosis() {
    let (_dir, a, b, feed) = two_processes("success-after-failure").await;
    let older = begin(&a, feed.id).await;
    let newer = begin(&b, feed.id).await;
    assert!(!superseded(&b, &newer, failed("newer failure")).await);
    let fetched_at = a.feeds.get_feed(feed.id).await.unwrap().unwrap().last_fetched_at;
    assert!(!superseded(&a, &older, fetched("Older", "etag-old", &["old"])).await);

    assert_eq!(entry_titles(&a).await, ["old"]);
    let stored = a.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(stored.etag.as_deref(), Some("etag-old"));
    assert!(stored.last_success_at.is_some());
    assert_eq!(stored.fetch_error.as_deref(), Some("newer failure"));
    assert_eq!(stored.last_fetched_at, fetched_at);
    assert_eq!(tickets(&a, feed.id).await, (2, 1, 2));
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn committing_one_ticket_twice_is_superseded_the_second_time() {
    let (_dir, a, _b, feed) = two_processes("duplicate").await;
    let target = begin(&a, feed.id).await;
    assert!(!superseded(&a, &target, fetched("Once", "etag", &["one"])).await);
    assert!(superseded(&a, &target, fetched("Twice", "etag-2", &["two"])).await);
    assert_eq!(entry_titles(&a).await, ["one"]);
}

/// A commit must answer a ticketed request: one without a ticket (a target from `list_targets`)
/// is refused and writes nothing, and `get_target` hands out ticketed targets.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_commit_without_a_ticket_is_refused() {
    let (_dir, a, _b, feed) = two_processes("untracked").await;
    let listed = a.refresh.list_targets().await.unwrap().remove(0);
    assert_eq!(listed.ticket, RefreshTicket::UNISSUED);
    for commit in [
        fetched("Untracked", "etag-u", &["u"]),
        RefreshCommit::NotModified { metadata: metadata("etag-u") },
        failed("untracked"),
    ] {
        assert!(a.refresh.commit(&listed, commit).await.is_err());
    }
    assert_eq!(tickets(&a, feed.id).await, (0, 0, 0));
    assert!(entry_titles(&a).await.is_empty());
    let stored = a.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!((stored.etag, stored.fetch_error), (None, None));

    let handed_out = a.refresh.get_target(feed.id).await.unwrap().unwrap();
    assert_eq!(handed_out.ticket, RefreshTicket { issued: 1, baseline: 0 });
}

/// A failure that stored its response's validators does not let a later 304 (which omits them)
/// bind those validators to content they never described: the 304 completes its validators from
/// the request it answers.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_304_after_a_failure_keeps_the_validators_of_the_published_version() {
    let (_dir, a, b, feed) = two_processes("failure-then-304").await;
    let initial = begin(&a, feed.id).await;
    assert!(!superseded(&a, &initial, fetched("A", "etag-a", &["a"])).await);

    let failing = begin(&a, feed.id).await;
    let conditional = begin(&b, feed.id).await;
    assert_eq!(conditional.etag.as_deref(), Some("etag-a"));
    let failure = RefreshCommit::Failed {
        failure: RefreshFailure::fetch("write failed", Some(metadata("etag-b"))),
    };
    assert!(!superseded(&a, &failing, failure).await);
    assert_eq!(
        a.feeds.get_feed(feed.id).await.unwrap().unwrap().etag.as_deref(),
        Some("etag-b"),
        "the failure stored its response's validators (shared contract)"
    );
    let not_modified = RefreshCommit::NotModified { metadata: RefreshHttpMetadata::default() };
    assert!(!superseded(&b, &conditional, not_modified).await);

    let stored = a.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(stored.etag.as_deref(), Some("etag-a"), "validators of the stored content");
    assert_eq!(stored.fetch_error, None);
    let next = begin(&a, feed.id).await;
    assert_eq!(
        next.etag.as_deref(),
        Some("etag-a"),
        "the next request asks about the stored content"
    );
    assert_eq!(entry_titles(&a).await, ["a"]);
}

/// An older failure overtaken by a newer success is reported as not modified by the refresh
/// service, not as a failure.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn the_refresh_service_reports_a_superseded_failure_as_not_modified() {
    let (_dir, a, b, feed) = two_processes("service-failure").await;
    let service = RefreshService::new(
        Arc::new(RacingSource {
            other: b,
            feed_id: feed.id,
            output: FeedRefreshSourceOutput::Failed(RefreshFailure::fetch("timeout", None)),
            race: true,
        }),
        Arc::new(LibraryRefreshStore::new(a.store.clone())),
    );
    let outcome = service.refresh_feed(feed.id).await.unwrap();
    assert!(matches!(outcome.result, RefreshFeedResult::NotModified), "{outcome:?}");
    assert_eq!(a.feeds.get_feed(feed.id).await.unwrap().unwrap().fetch_error, None);
}

/// In one process, a refresh whose write failed after the subscription was added does not make
/// the add flow drop its prepared fetch: the persistent tickets decide, and they accept it.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_prepared_fetch_survives_a_failed_refresh_in_the_same_process() {
    #[cfg(not(target_arch = "wasm32"))]
    let dir = TempDir::new("prepared-after-failure");
    #[cfg(not(target_arch = "wasm32"))]
    let library = Library::over(LibraryStore::open(dir.0.join("library.sqlite3")).await.unwrap());
    #[cfg(target_arch = "wasm32")]
    let library = Library::memory().await;
    let activated = library
        .mutations
        .activate_subscription(library::subscription("https://example.com/new.xml"))
        .await
        .unwrap();
    let store: Arc<dyn RefreshStorePort> =
        Arc::new(LibraryRefreshStore::new(library.store.clone()));
    let service = RefreshService::new(
        Arc::new(RacingSource {
            other: Library::over(library.store.clone()),
            feed_id: activated.feed.id,
            output: service_update("background"),
            race: false,
        }),
        store,
    );
    // The background refresh fetches, but its write fails.
    library
        .exec(
            "CREATE TRIGGER fail_background BEFORE INSERT ON entries WHEN NEW.dedup_key = 'background'
             BEGIN SELECT RAISE(ABORT, 'disk full'); END",
        )
        .await;
    // `refresh_feed` reports a failed write as an error (refresh_all turns it into a Failed
    // outcome per feed); the failure is recorded under the background refresh's ticket.
    assert!(service.refresh_feed(activated.feed.id).await.is_err());
    library.exec("DROP TRIGGER fail_background").await;

    let prepared = RefreshTarget {
        feed_id: activated.feed.id,
        generation: activated.generation,
        url: activated.feed.url.clone(),
        etag: None,
        last_modified: None,
        ticket: activated.ticket,
    };
    let FeedRefreshSourceOutput::Updated(update) = service_update("prepared") else {
        unreachable!()
    };
    let outcome = service.apply_prepared_update(prepared, update).await;
    assert!(
        matches!(outcome.result, RefreshFeedResult::Updated { inserted_count: 1, .. }),
        "{outcome:?}"
    );
    assert_eq!(entry_titles(&library).await, ["prepared"]);
    let stored = library.feeds.get_feed(activated.feed.id).await.unwrap().unwrap();
    assert!(stored.fetch_error.is_some(), "the newer failure stays the diagnosis");
}

/// The add flow's prepared fetch takes its ticket at activation, so a refresh that starts later
/// and publishes first wins.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_prepared_response_cannot_overwrite_a_commit_whose_caller_was_cancelled() {
    let library = Library::memory().await;
    let activated = library
        .mutations
        .activate_subscription(library::subscription("https://example.com/lost-ack"))
        .await
        .unwrap();
    let prepared = RefreshTarget {
        feed_id: activated.feed.id,
        generation: activated.generation,
        url: activated.feed.url.clone(),
        etag: None,
        last_modified: None,
        ticket: activated.ticket,
    };
    let later = begin(&library, activated.feed.id).await;
    let committed = tokio::sync::Notify::new();
    let caller = async {
        library.refresh.commit(&later, fetched("Later", "etag-l", &["later"])).await.unwrap();
        committed.notify_one();
        // The caller disappears after publication, before using its success result.
        std::future::pending::<()>().await;
    };
    match futures_util::future::select(Box::pin(caller), Box::pin(committed.notified())).await {
        futures_util::future::Either::Right(((), abandoned)) => drop(abandoned),
        _ => panic!("the caller must wait after publication"),
    }
    let service = RefreshService::new(
        Arc::new(RacingSource {
            other: Library::over(library.store.clone()),
            feed_id: activated.feed.id,
            output: service_update("unused"),
            race: false,
        }),
        Arc::new(library.refresh.clone()),
    );
    let FeedRefreshSourceOutput::Updated(update) = service_update("prepared") else {
        unreachable!()
    };
    let outcome = service.apply_prepared_update(prepared, update).await;
    assert!(matches!(outcome.result, RefreshFeedResult::NotModified), "{outcome:?}");
    assert_eq!(entry_titles(&library).await, ["later"]);
}

/// Activation and later publication are ordered across independent native stores (one
/// page connection on WASM).
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn the_prepared_fetch_at_activation_loses_to_a_later_refresh() {
    #[cfg(not(target_arch = "wasm32"))]
    let dir = TempDir::new("prepared");
    #[cfg(not(target_arch = "wasm32"))]
    let (a, b) = {
        let path = dir.0.join("library.sqlite3");
        (
            Library::over(LibraryStore::open(&path).await.unwrap()),
            Library::over(LibraryStore::open(&path).await.unwrap()),
        )
    };
    #[cfg(target_arch = "wasm32")]
    let (a, b) = {
        let a = Library::memory().await;
        let b = Library::over(a.store.clone());
        (a, b)
    };
    let activated = a
        .mutations
        .activate_subscription(library::subscription("https://example.com/new.xml"))
        .await
        .unwrap();
    assert_eq!(activated.ticket, RefreshTicket { issued: 1, baseline: 0 });
    let prepared = RefreshTarget {
        feed_id: activated.feed.id,
        generation: activated.generation,
        url: activated.feed.url.clone(),
        etag: None,
        last_modified: None,
        ticket: activated.ticket,
    };
    let later = begin(&b, activated.feed.id).await;
    assert!(!superseded(&b, &later, fetched("Later", "etag-l", &["later"])).await);
    assert!(superseded(&a, &prepared, fetched("Prepared", "etag-p", &["prepared"])).await);
    assert_eq!(entry_titles(&a).await, ["later"]);
}

/// A refresh that was started for a subscription that is removed and added again gets no target.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn begin_refresh_requires_the_expected_generation() {
    let (_dir, a, _b, feed) = two_processes("generation").await;
    assert!(a.refresh.begin_refresh(feed.id, Some(1)).await.unwrap().is_none());
    assert!(a.refresh.begin_refresh(feed.id, Some(0)).await.unwrap().is_some());
    a.feeds.set_deleted(feed.id, true).await.unwrap();
    assert!(a.refresh.begin_refresh(feed.id, None).await.unwrap().is_none());
}

/// A source whose fetch races with another process: while it fetches, the other process begins,
/// fetches and publishes a newer version.
struct RacingSource {
    other: Library,
    feed_id: i64,
    output: FeedRefreshSourceOutput,
    race: bool,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl FeedRefreshSourcePort for RacingSource {
    async fn refresh(&self, _target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
        if self.race {
            let newer = begin(&self.other, self.feed_id).await;
            assert!(!superseded(&self.other, &newer, fetched("Other", "etag-o", &["other"])).await);
        }
        Ok(self.output.clone())
    }
}

fn service_update(key: &str) -> FeedRefreshSourceOutput {
    FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
        metadata: metadata("etag-service"),
        feed: ParsedFeedUpdate {
            title: Some("Service".into()),
            site_url: None,
            description: None,
            entries: vec![ParsedEntryData {
                content_html: Some(format!("<p>{key}</p>")),
                ..library::entry(key)
            }],
        },
    })
}

/// The refresh service begins every refresh with a ticket; a response overtaken by another
/// process's publication is reported as not modified, with nothing stored or localized.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn the_refresh_service_takes_a_ticket_and_reports_a_superseded_response() {
    let (_dir, a, b, feed) = two_processes("service").await;
    let store: Arc<dyn RefreshStorePort> = Arc::new(LibraryRefreshStore::new(a.store.clone()));
    let calm = RefreshService::new(
        Arc::new(RacingSource {
            other: Library::over(b.store.clone()),
            feed_id: feed.id,
            output: service_update("first"),
            race: false,
        }),
        store.clone(),
    );
    let outcome = calm.refresh_feed(feed.id).await.unwrap();
    assert!(matches!(outcome.result, RefreshFeedResult::Updated { inserted_count: 1, .. }));
    assert_eq!(tickets(&a, feed.id).await, (1, 1, 1));

    let raced = RefreshService::new(
        Arc::new(RacingSource {
            other: b,
            feed_id: feed.id,
            output: service_update("overtaken"),
            race: true,
        }),
        store,
    );
    let outcome = raced.refresh_feed(feed.id).await.unwrap();
    assert!(matches!(outcome.result, RefreshFeedResult::NotModified), "{outcome:?}");
    assert_eq!(entry_titles(&a).await, ["first", "other"]);
    assert_eq!(a.feeds.get_feed(feed.id).await.unwrap().unwrap().etag.as_deref(), Some("etag-o"));
    assert_eq!(tickets(&a, feed.id).await, (3, 3, 3));
}

/// Cost of the write transaction that issues a ticket, on a file library (run explicitly:
/// `cargo test --release --test test_library_refresh_tickets -- --ignored --nocapture`).
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[ignore = "measurement"]
#[cfg(not(target_arch = "wasm32"))]
async fn measure_ticket_issuing() {
    let (_dir, a, _b, feed) = two_processes("measure").await;
    // Read on the writer: that connection issues the tickets (readers may differ).
    let synchronous: i64 = a
        .store
        .write(|tx| Ok(tx.query_row("PRAGMA synchronous", [], |row| row.get(0))?))
        .await
        .unwrap();
    let rounds = 500;
    let started = std::time::Instant::now();
    for _ in 0..rounds {
        begin(&a, feed.id).await;
    }
    let elapsed = started.elapsed();
    eprintln!(
        "TICKETS rounds={rounds} synchronous={synchronous} journal={} total_ms={} per_ticket_us={}",
        a.store.journal_mode(),
        elapsed.as_millis(),
        elapsed.as_micros() / rounds
    );
}

fn fetched_last_modified(title: &str, last_modified: &str, keys: &[&str]) -> RefreshCommit {
    let RefreshCommit::Updated { mut update } = fetched(title, "unused", keys) else {
        unreachable!()
    };
    update.metadata = RefreshHttpMetadata { etag: None, last_modified: Some(last_modified.into()) };
    RefreshCommit::Updated { update }
}

/// The `Last-Modified` counterpart of `a_304_after_a_failure_keeps_the_validators_of_the_published_version`.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_304_after_a_failure_keeps_the_last_modified_of_the_published_version() {
    let (_dir, a, b, feed) = two_processes("failure-then-304-lm").await;
    let initial = begin(&a, feed.id).await;
    assert!(!superseded(&a, &initial, fetched_last_modified("A", "lm-a", &["a"])).await);

    let failing = begin(&a, feed.id).await;
    let conditional = begin(&b, feed.id).await;
    assert_eq!(conditional.last_modified.as_deref(), Some("lm-a"));
    let failure = RefreshCommit::Failed {
        failure: RefreshFailure::fetch(
            "write failed",
            Some(RefreshHttpMetadata { etag: None, last_modified: Some("lm-b".into()) }),
        ),
    };
    assert!(!superseded(&a, &failing, failure).await);
    let not_modified = RefreshCommit::NotModified { metadata: RefreshHttpMetadata::default() };
    assert!(!superseded(&b, &conditional, not_modified).await);

    let stored = a.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(stored.last_modified.as_deref(), Some("lm-a"));
    assert_eq!(begin(&a, feed.id).await.last_modified.as_deref(), Some("lm-a"));
}

/// A 304 or a failure committed twice with one ticket is recorded once.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_304_or_a_failure_committed_twice_is_superseded_the_second_time() {
    let (_dir, a, _b, feed) = two_processes("duplicate-304-failure").await;
    let initial = begin(&a, feed.id).await;
    assert!(!superseded(&a, &initial, fetched("V1", "etag-1", &["v1"])).await);

    let conditional = begin(&a, feed.id).await;
    let not_modified = || RefreshCommit::NotModified { metadata: metadata("etag-1") };
    assert!(!superseded(&a, &conditional, not_modified()).await);
    assert!(superseded(&a, &conditional, not_modified()).await);
    assert_eq!(tickets(&a, feed.id).await, (2, 2, 2));

    let failing = begin(&a, feed.id).await;
    assert!(!superseded(&a, &failing, failed("first")).await);
    assert!(superseded(&a, &failing, failed("second")).await);
    let stored = a.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(stored.fetch_error.as_deref(), Some("first"));
    assert_eq!(tickets(&a, feed.id).await, (3, 2, 3));
}

/// A refresh started before the subscription was removed and added again writes nothing into the
/// new subscription, whatever it reports.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_refresh_from_before_a_removal_and_re_add_writes_nothing() {
    let (_dir, a, b, feed) = two_processes("re-add").await;
    let initial = begin(&a, feed.id).await;
    assert!(!superseded(&a, &initial, fetched("V1", "etag-1", &["v1"])).await);
    let stale = [begin(&a, feed.id).await, begin(&a, feed.id).await, begin(&a, feed.id).await];

    b.feeds.set_deleted(feed.id, true).await.unwrap();
    let again = b.add_feed("https://example.com/feed.xml").await;
    assert_eq!(again.id, feed.id);
    let before = (tickets(&a, feed.id).await, entry_titles(&a).await);
    let stored_before = a.feeds.get_feed(feed.id).await.unwrap().unwrap();

    let commits = [
        fetched("Stale", "etag-stale", &["stale"]),
        RefreshCommit::NotModified { metadata: metadata("etag-stale") },
        failed("stale failure"),
    ];
    for (target, commit) in stale.iter().zip(commits) {
        let result = a.refresh.commit(target, commit).await;
        assert!(result.is_err() || result.unwrap().superseded);
    }
    assert_eq!((tickets(&a, feed.id).await, entry_titles(&a).await), before);
    let stored = a.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(
        (stored.etag, stored.last_modified, stored.fetch_error),
        (stored_before.etag, stored_before.last_modified, stored_before.fetch_error)
    );
}

/// Refreshes started at once from two processes get distinct tickets, in one sequence.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test(flavor = "multi_thread"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn concurrent_refreshes_get_distinct_tickets() {
    let (_dir, a, b, feed) = two_processes("concurrent-issue").await;
    let (a, b) = (Arc::new(a), Arc::new(b));
    let mut tasks = Vec::new();
    for index in 0..40 {
        let library = Arc::clone(if index % 2 == 0 { &a } else { &b });
        tasks.push(async move { begin(&library, feed.id).await.ticket.issued });
    }
    let mut issued = futures_util::future::join_all(tasks).await;
    issued.sort_unstable();
    assert_eq!(issued, (1..=40).collect::<Vec<i64>>());
    assert_eq!(tickets(&a, feed.id).await, (40, 0, 0));
}
