#![cfg(not(target_arch = "wasm32"))]

//! The refresh store contract, run against the v0.2 library store (ported from
//! `test_application_refresh_store_adapter.rs`). The content-change and insert-count cases of
//! that file already run in `test_library_contracts.rs`.

#[path = "support/library.rs"]
mod library;

use std::sync::Arc;

use library::Library;
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshAllInput, RefreshCommit, RefreshFeedResult, RefreshHttpMetadata,
    RefreshService, RefreshStorePort, RefreshTarget, SubscriptionRemovalPort,
};
use rssr_domain::{
    EntryContentRepository, EntryIndexRepository, EntryQuery, FeedRepository, NewFeedSubscription,
};
use time::OffsetDateTime;
use url::Url;

#[tokio::test]
async fn library_refresh_store_persists_updated_feed_metadata_entries_and_fetch_state() {
    let library = Library::memory().await;
    let feed = library
        .feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example".to_string()),
            source_title: None,
            folder: Some("Tech".to_string()),
        })
        .await
        .expect("create feed");

    let target = library.refresh.get_target(feed.id).await.unwrap().unwrap();
    library
        .refresh
        .commit(
            &target,
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
        library.feeds.get_feed(feed.id).await.expect("read feed").expect("feed exists");
    // 添加时指定的名称不被刷新覆盖，源标题单独保存（审计 B6）。
    assert_eq!(stored_feed.title.as_deref(), Some("Example"));
    assert_eq!(stored_feed.source_title.as_deref(), Some("Example Feed"));
    assert_eq!(stored_feed.description.as_deref(), Some("Example description"));
    assert_eq!(stored_feed.etag.as_deref(), Some("etag-1"));
    assert!(stored_feed.last_fetched_at.is_some());
    assert!(stored_feed.last_success_at.is_some());
    assert_eq!(stored_feed.fetch_error, None);

    let entries = library.entries.list_entries(&EntryQuery::default()).await.expect("list entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "中文 Entry 1");

    let entry = library
        .entries
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
async fn library_refresh_store_forces_full_fetch_when_feed_has_no_entries() {
    let library = Library::memory().await;
    let feed = library
        .feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example".to_string()),
            source_title: None,
            folder: None,
        })
        .await
        .expect("create feed");

    // Persist validators the way a successful response does (v0.1.x wrote them with
    // `update_fetch_state`; the library only changes fetch state through a refresh commit).
    let target = library.refresh.get_target(feed.id).await.unwrap().unwrap();
    library
        .refresh
        .commit(
            &target,
            RefreshCommit::NotModified {
                metadata: RefreshHttpMetadata {
                    etag: Some("etag-empty".to_string()),
                    last_modified: Some("Wed, 01 Apr 2026 10:00:00 GMT".to_string()),
                },
            },
        )
        .await
        .expect("persist validators");
    let stored = library.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(stored.etag.as_deref(), Some("etag-empty"), "the validators are stored");

    let target = library
        .refresh
        .get_target(feed.id)
        .await
        .expect("load refresh target")
        .expect("target exists");
    assert_eq!(
        target,
        RefreshTarget {
            feed_id: feed.id,
            generation: 0,
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

/// `table` is the table whose insert is made to fail: the index row (`entries`) or the body
/// (`entry_contents`). In v0.1.x these were two databases and a failure left the other half
/// written; the library commits both in one transaction, so either failure leaves the previous
/// response intact (intentional difference 2) -- asserted below in addition to the original
/// checks.
async fn check_retry_after_failed_write(table: &str) {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/recovery.xml").await;
    let store = Arc::new(library.refresh.clone());
    let target = store.get_target(feed.id).await.unwrap().unwrap();
    store
        .commit(&target, RefreshCommit::Updated { update: recovery_update("old") })
        .await
        .expect("seed complete old response");
    let last_success = library.feeds.get_feed(feed.id).await.unwrap().unwrap().last_success_at;
    let entry_id = library.entries.list_entries(&EntryQuery::default()).await.unwrap()[0].id;

    library
        .exec(format!(
            "CREATE TRIGGER fail_refresh_write BEFORE INSERT ON {table} \
             BEGIN SELECT RAISE(FAIL, 'injected write failure'); END"
        ))
        .await;
    let service = RefreshService::new(
        Arc::new(ConditionalSource { update: recovery_update("new") }),
        store.clone(),
    );
    let error = service.refresh_feed(feed.id).await.expect_err("write must fail");
    assert!(format!("{error:#}").contains("injected write failure"));
    let failed_feed = library.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(failed_feed.last_success_at, last_success);
    assert!(failed_feed.fetch_error.is_some());
    // Retain observed response metadata for the existing persistence contract.
    assert_eq!(failed_feed.etag.as_deref(), Some("etag-new"));
    assert_eq!(
        library.entries.get_content(entry_id).await.unwrap().unwrap().content_html.as_deref(),
        Some("<p>正文 old</p>")
    );
    // All or nothing: the failed commit wrote neither the new feed metadata nor the new entry.
    assert_eq!(failed_feed.source_title.as_deref(), Some("Feed old"));
    let entries = library.entries.list_entries(&EntryQuery::default()).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Entry old");
    let retry_target = store.get_target(feed.id).await.unwrap().unwrap();
    assert_eq!(retry_target.etag, None);
    assert_eq!(retry_target.last_modified, None);

    library.exec("DROP TRIGGER fail_refresh_write").await;
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
        library.entries.get_content(entry_id).await.unwrap().unwrap().content_html.as_deref(),
        Some("<p>正文 new</p>")
    );
    let recovered_feed = library.feeds.get_feed(feed.id).await.unwrap().unwrap();
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
async fn library_refresh_retries_content_after_write_failure_instead_of_accepting_304() {
    check_retry_after_failed_write("entry_contents").await;
}

#[tokio::test]
async fn library_refresh_retries_index_after_write_failure_instead_of_accepting_304() {
    check_retry_after_failed_write("entries").await;
}

#[tokio::test]
async fn old_library_refresh_generation_cannot_commit_after_delete_and_same_url_readd() {
    let library = Library::memory().await;
    let feeds = &library.feeds;
    let store = &library.refresh;

    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/aba.xml").unwrap(),
            title: Some("Original".into()),
            source_title: None,
            folder: None,
        })
        .await
        .unwrap();
    let stale_target = store.get_target(feed.id).await.unwrap().unwrap();
    assert_eq!(stale_target.generation, 0);
    store
        .commit(
            &stale_target,
            RefreshCommit::NotModified {
                metadata: RefreshHttpMetadata {
                    etag: Some("old-generation-etag".into()),
                    last_modified: Some("old-generation-modified".into()),
                },
            },
        )
        .await
        .unwrap();

    library.mutations.remove_subscription(feed.id, true).await.unwrap();
    feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/aba.xml").unwrap(),
            title: Some("Re-added".into()),
            source_title: None,
            folder: None,
        })
        .await
        .unwrap();

    let fresh_target = store.get_target(feed.id).await.unwrap().unwrap();
    assert_eq!(fresh_target.generation, 1);
    assert_eq!(fresh_target.etag, None);
    assert_eq!(fresh_target.last_modified, None);

    let stale = store
        .commit(&stale_target, RefreshCommit::Updated { update: recovery_update("stale") })
        .await;
    assert!(stale.is_err(), "old generation must not commit after re-add");

    let active = feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(active.title.as_deref(), Some("Re-added"));
    // 指定名称会遮住源标题：单独核对迟到的刷新没有写入源标题（独立评审）。
    assert_eq!(active.source_title, None, "the late refresh wrote no metadata");
    assert!(library.entries.list_entries(&EntryQuery::default()).await.unwrap().is_empty());

    store
        .commit(&fresh_target, RefreshCommit::Updated { update: recovery_update("fresh") })
        .await
        .unwrap();
    let refreshed = feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(refreshed.source_title.as_deref(), Some("Feed fresh"));
    assert_eq!(refreshed.title.as_deref(), Some("Re-added"), "the chosen name stays (B6)");
}
