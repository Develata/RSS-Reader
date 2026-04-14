#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use rssr_application::{
    FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit, RefreshHttpMetadata,
    RefreshStorePort, RefreshTarget,
};
use rssr_domain::{EntryQuery, EntryRepository, FeedRepository, NewFeedSubscription};
use rssr_infra::{
    application_adapters::SqliteRefreshStore,
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
};
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
                            title: "Entry 1".to_string(),
                            author: Some("Author".to_string()),
                            summary: Some("Summary".to_string()),
                            content_html: Some("<p>Summary</p>".to_string()),
                            content_text: Some("Summary".to_string()),
                            published_at: None,
                            updated_at_source: None,
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
    assert_eq!(entries[0].title, "Entry 1");
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
