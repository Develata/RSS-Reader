#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshAllInput, RefreshCommit, RefreshFeedResult, RefreshHttpMetadata,
    RefreshService, RefreshStorePort, RefreshTarget,
};
use rssr_domain::{EntryQuery, FeedRepository, NewFeedSubscription};
use rssr_infra::{
    application_adapters::SqliteRefreshStore,
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        migrate_content, sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
};
use time::OffsetDateTime;
use url::Url;

#[tokio::test]
async fn sqlite_refresh_store_persists_updated_feed_metadata_entries_and_fetch_state() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite memory");
    migrate(&pool).await.expect("run migrations");

    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
    let store = SqliteRefreshStore::new(feed_repository.clone(), entry_repository.clone());

    let feed = feed_repository
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example".to_string()),
            folder: Some("Tech".to_string()),
        })
        .await
        .expect("create feed");

    store
        .commit(
            feed.id,
            RefreshCommit::Updated {
                update: FeedRefreshUpdate {
                    metadata: RefreshHttpMetadata {
                        etag: Some("etag-1".to_string()),
                        last_modified: Some("Wed, 01 Apr 2026 10:00:00 GMT".to_string()),
                    },
                    feed: ParsedFeedUpdate {
                        title: Some("Example Feed".to_string()),
                        site_url: Some(Url::parse("https://example.com").expect("valid url")),
                        description: Some("Example description".to_string()),
                        entries: vec![ParsedEntryData {
                            external_id: "entry-1".to_string(),
                            dedup_key: "entry-1".to_string(),
                            url: Some(
                                Url::parse("https://example.com/articles/entry-1")
                                    .expect("valid entry url"),
                            ),
                            title: "中文 Entry 1".to_string(),
                            author: Some("作者 Author".to_string()),
                            summary: Some("摘要 Summary".to_string()),
                            content_html: Some("<p>正文 Content</p>".to_string()),
                            content_text: Some("正文 Content".to_string()),
                            published_at: Some(OffsetDateTime::UNIX_EPOCH),
                            updated_at_source: Some(OffsetDateTime::UNIX_EPOCH),
                        }],
                    },
                },
            },
        )
        .await
        .expect("commit refresh update");

    let stored_feed =
        feed_repository.get_feed(feed.id).await.expect("read feed").expect("feed exists");
    assert_eq!(stored_feed.title.as_deref(), Some("Example Feed"));
    assert_eq!(stored_feed.description.as_deref(), Some("Example description"));
    assert_eq!(stored_feed.etag.as_deref(), Some("etag-1"));
    assert!(stored_feed.last_fetched_at.is_some());
    assert!(stored_feed.last_success_at.is_some());
    assert_eq!(stored_feed.fetch_error, None);

    let entries =
        entry_repository.list_entries(&EntryQuery::default()).await.expect("list entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "中文 Entry 1");

    let entry = entry_repository
        .get_entry(entries[0].id)
        .await
        .expect("read committed entry")
        .expect("committed entry exists");
    assert_eq!(entry.external_id, "entry-1");
    assert_eq!(entry.dedup_key, "entry-1");
    assert_eq!(entry.url.as_ref().map(Url::as_str), Some("https://example.com/articles/entry-1"));
    assert_eq!(entry.title, "中文 Entry 1");
    assert_eq!(entry.author.as_deref(), Some("作者 Author"));
    assert_eq!(entry.summary.as_deref(), Some("摘要 Summary"));
    assert_eq!(entry.content_html.as_deref(), Some("<p>正文 Content</p>"));
    assert_eq!(entry.content_text.as_deref(), Some("正文 Content"));
    assert_eq!(entry.published_at, Some(OffsetDateTime::UNIX_EPOCH));
    assert_eq!(entry.updated_at_source, Some(OffsetDateTime::UNIX_EPOCH));
}

#[tokio::test]
async fn sqlite_refresh_store_forces_full_fetch_when_feed_has_no_entries() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite memory");
    migrate(&pool).await.expect("run migrations");

    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
    let store = SqliteRefreshStore::new(feed_repository.clone(), entry_repository.clone());

    let feed = feed_repository
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example".to_string()),
            folder: None,
        })
        .await
        .expect("create feed");

    feed_repository
        .update_fetch_state(
            feed.id,
            Some("etag-empty"),
            Some("Wed, 01 Apr 2026 10:00:00 GMT"),
            None,
            true,
        )
        .await
        .expect("persist validators");

    let target =
        store.get_target(feed.id).await.expect("load refresh target").expect("target exists");
    assert_eq!(
        target,
        RefreshTarget {
            feed_id: feed.id,
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            etag: None,
            last_modified: None,
        }
    );
}

struct ConditionalSource {
    update: FeedRefreshUpdate,
}

#[async_trait::async_trait]
impl FeedRefreshSourcePort for ConditionalSource {
    async fn refresh(&self, target: &RefreshTarget) -> anyhow::Result<FeedRefreshSourceOutput> {
        // Model a server that correctly returns 304 for either matching validator. Recovery
        // requires the store to stop sending validators for a response it failed to persist.
        if target.etag == self.update.metadata.etag
            || target.last_modified == self.update.metadata.last_modified
        {
            Ok(FeedRefreshSourceOutput::NotModified(self.update.metadata.clone()))
        } else {
            Ok(FeedRefreshSourceOutput::Updated(self.update.clone()))
        }
    }
}

fn recovery_update(version: &str) -> FeedRefreshUpdate {
    FeedRefreshUpdate {
        metadata: RefreshHttpMetadata {
            etag: Some(format!("etag-{version}")),
            last_modified: Some(format!("modified-{version}")),
        },
        feed: ParsedFeedUpdate {
            title: Some(format!("Feed {version}")),
            site_url: None,
            description: None,
            entries: vec![ParsedEntryData {
                external_id: "entry".to_string(),
                dedup_key: "entry".to_string(),
                url: None,
                title: format!("Entry {version}"),
                author: None,
                summary: None,
                content_html: Some(format!("<p>正文 {version}</p>")),
                content_text: None,
                published_at: None,
                updated_at_source: None,
            }],
        },
    }
}

async fn check_retry_after_failed_write(fail_content: bool) {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let index_pool = backend.connect().await.expect("connect index db");
    migrate(&index_pool).await.expect("migrate index db");
    let content_pool = backend.connect_content().await.expect("connect content db");
    migrate_content(&content_pool).await.expect("migrate content db");
    let feeds = Arc::new(SqliteFeedRepository::new(index_pool.clone()));
    let entries = Arc::new(SqliteEntryRepository::new_with_content_pool(
        index_pool.clone(),
        content_pool.clone(),
    ));
    let store = Arc::new(SqliteRefreshStore::new(feeds.clone(), entries.clone()));
    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/recovery.xml").expect("fixture URL"),
            title: None,
            folder: None,
        })
        .await
        .expect("seed feed");
    store
        .commit(feed.id, RefreshCommit::Updated { update: recovery_update("old") })
        .await
        .expect("seed complete old response");
    let last_success = feeds.get_feed(feed.id).await.unwrap().unwrap().last_success_at;
    let entry_id = entries.list_entries(&EntryQuery::default()).await.unwrap()[0].id;

    let (failure_pool, table) =
        if fail_content { (&content_pool, "entry_contents") } else { (&index_pool, "entries") };
    sqlx::query(&format!(
        "CREATE TRIGGER fail_refresh_write BEFORE INSERT ON {table} \
         BEGIN SELECT RAISE(FAIL, 'injected write failure'); END"
    ))
    .execute(failure_pool)
    .await
    .expect("inject actual SQLite write failure");
    let service = RefreshService::new(
        Arc::new(ConditionalSource { update: recovery_update("new") }),
        store.clone(),
    );
    let error = service.refresh_feed(feed.id).await.expect_err("write must fail");
    assert!(format!("{error:#}").contains("injected write failure"));
    let failed_feed = feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(failed_feed.last_success_at, last_success);
    assert!(failed_feed.fetch_error.is_some());
    // Retain observed response metadata for the existing persistence contract.
    assert_eq!(failed_feed.etag.as_deref(), Some("etag-new"));
    assert_eq!(
        entries.get_content(entry_id).await.unwrap().unwrap().content_html.as_deref(),
        Some("<p>正文 old</p>")
    );
    let retry_target = store.get_target(feed.id).await.unwrap().unwrap();
    assert_eq!(retry_target.etag, None);
    assert_eq!(retry_target.last_modified, None);

    sqlx::query("DROP TRIGGER fail_refresh_write")
        .execute(failure_pool)
        .await
        .expect("restore writable database");
    // Exercise list_targets (refresh all) after get_target (refresh one) failed.
    let recovered = service
        .refresh_all(RefreshAllInput::default())
        .await
        .expect("retry after restoring writes");
    assert!(
        matches!(recovered.feeds[0].result, RefreshFeedResult::Updated { .. }),
        "failed response must be fetched again, got {:?}",
        recovered.feeds[0].result
    );
    assert_eq!(
        entries.get_content(entry_id).await.unwrap().unwrap().content_html.as_deref(),
        Some("<p>正文 new</p>")
    );
    let recovered_feed = feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert!(recovered_feed.fetch_error.is_none());
    assert!(recovered_feed.last_success_at >= last_success);
    let target = store.get_target(feed.id).await.unwrap().unwrap();
    assert_eq!(target.etag.as_deref(), Some("etag-new"));
    assert_eq!(target.last_modified.as_deref(), Some("modified-new"));
    assert!(matches!(
        service.refresh_feed(feed.id).await.expect("next conditional refresh").result,
        RefreshFeedResult::NotModified
    ));
}

#[tokio::test]
async fn sqlite_refresh_retries_content_after_write_failure_instead_of_accepting_304() {
    check_retry_after_failed_write(true).await;
}

#[tokio::test]
async fn sqlite_refresh_retries_index_after_write_failure_instead_of_accepting_304() {
    check_retry_after_failed_write(false).await;
}

#[path = "support/refresh_count_cases.rs"]
mod refresh_count_cases;

#[path = "support/refresh_content_cases.rs"]
mod refresh_content_cases;

#[tokio::test]
async fn sqlite_refresh_only_updates_changed_content_records() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.unwrap();
    migrate(&pool).await.unwrap();
    let feeds = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entries = Arc::new(SqliteEntryRepository::new(pool.clone()));
    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/content.xml").unwrap(),
            title: None,
            site_url: None,
            folder: None,
        })
        .await
        .unwrap();
    let store = SqliteRefreshStore::new(feeds, entries.clone());
    refresh_content_cases::verify_content_changes(
        &store,
        entries.as_ref(),
        entries.as_ref(),
        feed.id,
    )
    .await;

    // Observe actual database writes rather than relying on a clock tick between refreshes.
    sqlx::query("CREATE TRIGGER reject_redundant_update BEFORE UPDATE ON entry_contents BEGIN SELECT RAISE(FAIL, 'redundant content write'); END")
        .execute(&pool).await.unwrap();
    let content = entries.get_content(1).await.unwrap().unwrap();
    assert_eq!(
        entries
            .upsert_contents(
                feed.id,
                &[rssr_infra::db::entry_repository::ResolvedEntryContent {
                    entry_id: 1,
                    dedup_key: "entry".into(),
                    content_html: content.content_html,
                    content_text: content.content_text,
                    content_hash: content.content_hash,
                }]
            )
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn sqlite_counts_only_real_inserts_including_same_batch_duplicates() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.unwrap();
    migrate(&pool).await.unwrap();
    let feeds = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entries = Arc::new(SqliteEntryRepository::new(pool));
    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/count").unwrap(),
            site_url: None,
            title: None,
            folder: None,
        })
        .await
        .unwrap();
    let store = SqliteRefreshStore::new(feeds.clone(), entries);
    refresh_count_cases::verify_counts(&store, feed.id).await;
    assert_eq!(feeds.list_summaries().await.unwrap()[0].entry_count, 3);
}
