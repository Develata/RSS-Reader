use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshAllInput, RefreshFailure, RefreshHttpMetadata, RefreshService,
    RefreshTarget,
};
use rssr_domain::{EntryQuery, EntryRepository, FeedRepository, NewFeedSubscription};
use rssr_infra::{
    application_adapters::SqliteRefreshStore,
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
};
use time::OffsetDateTime;
use url::Url;

#[derive(Clone)]
struct ScriptedSource {
    outputs: Arc<Mutex<Vec<FeedRefreshSourceOutput>>>,
}

#[async_trait::async_trait]
impl FeedRefreshSourcePort for ScriptedSource {
    async fn refresh(&self, _target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
        let mut outputs = self.outputs.lock().expect("lock outputs");
        if outputs.is_empty() {
            anyhow::bail!("no scripted source output left");
        }
        Ok(outputs.remove(0))
    }
}

struct SqliteFixture {
    service: RefreshService,
    feed_repository: Arc<SqliteFeedRepository>,
    entry_repository: Arc<SqliteEntryRepository>,
    feed_ids: Vec<i64>,
}

async fn build_sqlite_fixture(
    feed_urls: &[&str],
    outputs: Vec<FeedRefreshSourceOutput>,
) -> Result<SqliteFixture> {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.context("connect sqlite memory")?;
    migrate(&pool).await.context("run migrations")?;

    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entry_repository = Arc::new(SqliteEntryRepository::new(pool));
    let store =
        Arc::new(SqliteRefreshStore::new(feed_repository.clone(), entry_repository.clone()));
    let source = Arc::new(ScriptedSource { outputs: Arc::new(Mutex::new(outputs)) });
    let service = RefreshService::new(source, store);

    let mut feed_ids = Vec::with_capacity(feed_urls.len());
    for raw in feed_urls {
        let feed = feed_repository
            .upsert_subscription(&NewFeedSubscription {
                url: Url::parse(raw).with_context(|| format!("parse seeded feed url: {raw}"))?,
                title: None,
                folder: None,
            })
            .await
            .with_context(|| format!("seed feed: {raw}"))?;
        feed_ids.push(feed.id);
    }

    Ok(SqliteFixture { service, feed_repository, entry_repository, feed_ids })
}

fn metadata(etag: &str, last_modified: &str) -> RefreshHttpMetadata {
    RefreshHttpMetadata {
        etag: Some(etag.to_string()),
        last_modified: Some(last_modified.to_string()),
    }
}

fn sample_entry(index: usize) -> ParsedEntryData {
    ParsedEntryData {
        external_id: format!("entry-{index}"),
        dedup_key: format!("entry-{index}"),
        url: Some(Url::parse(&format!("https://example.com/articles/{index}")).expect("valid url")),
        title: format!("Entry {index}"),
        author: Some("RSSR".to_string()),
        summary: Some(format!("Summary {index}")),
        content_html: Some(format!("<p>Summary {index}</p>")),
        content_text: Some(format!("Summary {index}")),
        published_at: Some(OffsetDateTime::UNIX_EPOCH + time::Duration::days(index as i64)),
        updated_at_source: None,
    }
}

fn updated_output(feed_title: &str, entries: Vec<ParsedEntryData>) -> FeedRefreshSourceOutput {
    FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
        metadata: metadata("etag-1", "Wed, 01 Apr 2026 10:00:00 GMT"),
        feed: ParsedFeedUpdate {
            title: Some(feed_title.to_string()),
            site_url: Some(Url::parse("https://example.com").expect("valid site url")),
            description: Some("Example description".to_string()),
            entries,
        },
    })
}

#[tokio::test]
async fn refresh_contract_persists_updated_feed_and_entries() {
    let fixture = build_sqlite_fixture(
        &["https://example.com/feed.xml"],
        vec![updated_output("Example Feed", vec![sample_entry(1)])],
    )
    .await
    .expect("build fixture");

    let outcome = fixture.service.refresh_feed(fixture.feed_ids[0]).await.expect("refresh feed");

    assert!(matches!(
        outcome.result,
        rssr_application::RefreshFeedResult::Updated { entry_count: 1, .. }
    ));

    let stored_feed = fixture
        .feed_repository
        .get_feed(fixture.feed_ids[0])
        .await
        .expect("read feed")
        .expect("feed exists");
    assert_eq!(stored_feed.title.as_deref(), Some("Example Feed"));
    assert_eq!(stored_feed.description.as_deref(), Some("Example description"));
    assert_eq!(stored_feed.etag.as_deref(), Some("etag-1"));
    assert!(stored_feed.last_fetched_at.is_some());
    assert!(stored_feed.last_success_at.is_some());
    assert_eq!(stored_feed.fetch_error, None);

    let entries = fixture
        .entry_repository
        .list_entries(&EntryQuery { feed_id: Some(fixture.feed_ids[0]), ..EntryQuery::default() })
        .await
        .expect("list entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Entry 1");
}

#[tokio::test]
async fn refresh_contract_records_not_modified_without_creating_entries() {
    let fixture = build_sqlite_fixture(
        &["https://example.com/feed.xml"],
        vec![FeedRefreshSourceOutput::NotModified(metadata(
            "etag-2",
            "Thu, 02 Apr 2026 10:00:00 GMT",
        ))],
    )
    .await
    .expect("build fixture");

    let outcome = fixture.service.refresh_feed(fixture.feed_ids[0]).await.expect("refresh feed");

    assert!(matches!(outcome.result, rssr_application::RefreshFeedResult::NotModified));

    let stored_feed = fixture
        .feed_repository
        .get_feed(fixture.feed_ids[0])
        .await
        .expect("read feed")
        .expect("feed exists");
    assert_eq!(stored_feed.etag.as_deref(), Some("etag-2"));
    assert_eq!(stored_feed.last_modified.as_deref(), Some("Thu, 02 Apr 2026 10:00:00 GMT"));
    assert!(stored_feed.last_fetched_at.is_some());
    assert!(stored_feed.last_success_at.is_some());
    assert_eq!(stored_feed.fetch_error, None);

    let entries =
        fixture.entry_repository.list_entries(&EntryQuery::default()).await.expect("list entries");
    assert!(entries.is_empty());
}

#[tokio::test]
async fn refresh_contract_records_failure_message_and_fetch_timestamp() {
    let fixture = build_sqlite_fixture(
        &["https://example.com/feed.xml"],
        vec![FeedRefreshSourceOutput::Failed(RefreshFailure {
            message: "抓取订阅失败: network timeout".to_string(),
            metadata: Some(metadata("etag-failed", "Fri, 03 Apr 2026 10:00:00 GMT")),
        })],
    )
    .await
    .expect("build fixture");

    let outcome = fixture.service.refresh_feed(fixture.feed_ids[0]).await.expect("refresh feed");

    assert!(matches!(
        outcome.result,
        rssr_application::RefreshFeedResult::Failed { ref message }
        if message == "抓取订阅失败: network timeout"
    ));

    let stored_feed = fixture
        .feed_repository
        .get_feed(fixture.feed_ids[0])
        .await
        .expect("read feed")
        .expect("feed exists");
    assert_eq!(stored_feed.etag.as_deref(), Some("etag-failed"));
    assert_eq!(stored_feed.last_modified.as_deref(), Some("Fri, 03 Apr 2026 10:00:00 GMT"));
    assert!(stored_feed.last_fetched_at.is_some());
    assert!(stored_feed.last_success_at.is_none());
    assert_eq!(stored_feed.fetch_error.as_deref(), Some("抓取订阅失败: network timeout"));
}

#[tokio::test]
async fn refresh_contract_refresh_all_aggregates_outcomes_in_target_order() {
    let fixture = build_sqlite_fixture(
        &["https://example.com/feed-1.xml", "https://example.com/feed-2.xml"],
        vec![
            updated_output("Feed One", vec![sample_entry(1), sample_entry(2)]),
            FeedRefreshSourceOutput::NotModified(metadata(
                "etag-2",
                "Thu, 02 Apr 2026 10:00:00 GMT",
            )),
        ],
    )
    .await
    .expect("build fixture");

    let outcome = fixture
        .service
        .refresh_all(RefreshAllInput { max_concurrency: 1 })
        .await
        .expect("refresh all");

    assert_eq!(outcome.feeds.len(), 2);
    assert_eq!(outcome.updated_count(), 1);
    assert_eq!(outcome.not_modified_count(), 1);
    assert!(!outcome.has_failures());
    assert_eq!(outcome.feeds[0].feed_id, fixture.feed_ids[0]);
    assert_eq!(outcome.feeds[1].feed_id, fixture.feed_ids[1]);

    let first_entries = fixture
        .entry_repository
        .list_entries(&EntryQuery { feed_id: Some(fixture.feed_ids[0]), ..EntryQuery::default() })
        .await
        .expect("list first feed entries");
    assert_eq!(first_entries.len(), 2);

    let second_entries = fixture
        .entry_repository
        .list_entries(&EntryQuery { feed_id: Some(fixture.feed_ids[1]), ..EntryQuery::default() })
        .await
        .expect("list second feed entries");
    assert!(second_entries.is_empty());
}
