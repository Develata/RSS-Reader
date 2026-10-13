#![cfg(not(target_arch = "wasm32"))]

//! The subscription contract run against the v0.2 library store (ports of
//! `test_subscription_contract_harness.rs`; the custom-title case lives in
//! `test_library_contracts.rs`).

#[path = "support/library.rs"]
mod library;

use std::sync::Arc;

use anyhow::{Result, bail};
use library::Library;
use rssr_application::{
    AddSubscriptionInput, FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedService,
    ParsedEntryData, RefreshCommit, RefreshService, RefreshStorePort, RemoveSubscriptionInput,
    SubscriptionWorkflow,
};
use rssr_domain::{AppStateRepository, EntryIndexRepository, EntryQuery, FeedRepository};
use time::OffsetDateTime;
use url::Url;

struct UnusedRefreshSource;

#[async_trait::async_trait]
impl rssr_application::SubscriptionProbePort for UnusedRefreshSource {
    async fn probe(&self, url: &Url) -> Result<rssr_application::SubscriptionProbeOutcome> {
        Ok(rssr_application::SubscriptionProbeOutcome::Feed {
            url: url.clone(),
            update: rssr_application::FeedRefreshUpdate {
                metadata: Default::default(),
                feed: rssr_application::ParsedFeedUpdate {
                    title: None,
                    site_url: None,
                    description: None,
                    entries: Vec::new(),
                },
            },
        })
    }
}

#[async_trait::async_trait]
impl FeedRefreshSourcePort for UnusedRefreshSource {
    async fn refresh(
        &self,
        _target: &rssr_application::RefreshTarget,
    ) -> Result<FeedRefreshSourceOutput> {
        bail!("refresh source should not be used in subscription harness")
    }
}

struct UnusedRefreshStore;

#[async_trait::async_trait]
impl RefreshStorePort for UnusedRefreshStore {
    async fn list_targets(&self) -> Result<Vec<rssr_application::RefreshTarget>> {
        bail!("refresh store should not be used in subscription harness")
    }

    async fn get_target(&self, _feed_id: i64) -> Result<Option<rssr_application::RefreshTarget>> {
        bail!("refresh store should not be used in subscription harness")
    }

    async fn commit(
        &self,
        _target: &rssr_application::RefreshTarget,
        _commit: RefreshCommit,
    ) -> Result<rssr_application::RefreshCommitOutcome> {
        bail!("refresh store should not be used in subscription harness")
    }
}

struct Fixture {
    workflow: SubscriptionWorkflow,
    library: Library,
}

async fn build_fixture() -> Fixture {
    let library = Library::memory().await;
    let feed_service = FeedService::new(
        Arc::new(library.feeds.clone()),
        Arc::new(library.mutations.clone()),
        Arc::new(library.mutations.clone()),
    );
    let refresh_service =
        RefreshService::new(Arc::new(UnusedRefreshSource), Arc::new(UnusedRefreshStore));
    let workflow =
        SubscriptionWorkflow::new(feed_service, refresh_service, Arc::new(UnusedRefreshSource));
    Fixture { workflow, library }
}

async fn set_last_opened(library: &Library, feed_id: i64) {
    library
        .app_state
        .update(Box::new(move |state| state.last_opened_feed_id = Some(feed_id)))
        .await
        .expect("save last opened feed");
}

async fn last_opened(library: &Library) -> Option<i64> {
    library.app_state.load().await.expect("load app state").last_opened_feed_id
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

#[tokio::test]
async fn subscription_contract_add_normalizes_and_deduplicates_urls() {
    let fixture = build_fixture().await;

    let first = fixture
        .workflow
        .add_subscription(&AddSubscriptionInput {
            url: "https://example.com:443/feed.xml#fragment".to_string(),
            title: Some("Example".to_string()),
            folder: Some("Inbox".to_string()),
        })
        .await
        .expect("add first subscription");
    let second = fixture
        .workflow
        .add_subscription(&AddSubscriptionInput {
            url: "https://example.com/feed.xml".to_string(),
            title: Some("Updated Title".to_string()),
            folder: Some("Reading".to_string()),
        })
        .await
        .expect_err("duplicate must not modify existing subscription");

    assert!(second.to_string().contains("已订阅"));
    assert_eq!(first.url.as_str(), "https://example.com/feed.xml");

    let feeds = fixture.library.feeds.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].id, first.id);
    assert_eq!(feeds[0].url.as_str(), "https://example.com/feed.xml");
    assert_eq!(feeds[0].title.as_deref(), Some("Example"));
    assert_eq!(feeds[0].folder.as_deref(), Some("Inbox"));
}

#[tokio::test]
async fn subscription_contract_remove_purges_entries_soft_deletes_feed_and_clears_matching_state() {
    let fixture = build_fixture().await;
    let library = &fixture.library;

    let feed = library
        .feeds
        .upsert_subscription(&rssr_domain::NewFeedSubscription {
            title: Some("Example".to_string()),
            ..library::subscription("https://example.com/feed.xml")
        })
        .await
        .expect("seed feed");
    library.seed(feed.id, vec![sample_entry(1), sample_entry(2)]).await;
    set_last_opened(library, feed.id).await;

    fixture
        .workflow
        .remove_subscription(RemoveSubscriptionInput { feed_id: feed.id, purge_entries: true })
        .await
        .expect("remove subscription");

    assert!(library.feeds.get_feed(feed.id).await.expect("get feed").is_none());
    assert_eq!(
        library
            .entries
            .list_entries(&EntryQuery { feed_id: Some(feed.id), ..EntryQuery::default() })
            .await
            .expect("list entries")
            .len(),
        0
    );
    assert_eq!(last_opened(library).await, None);

    let deleted_flag =
        library.scalar(format!("SELECT is_deleted FROM feeds WHERE id = {}", feed.id));
    assert_eq!(deleted_flag.await, 1);
    // Bodies leave with their entries (intentional difference 1: one file, foreign-key cascade).
    assert_eq!(library.scalar("SELECT COUNT(*) FROM entry_contents").await, 0);
}

#[tokio::test]
async fn subscription_contract_remove_preserves_other_last_opened_feed() {
    let fixture = build_fixture().await;
    let library = &fixture.library;

    let retained_feed = library
        .feeds
        .upsert_subscription(&rssr_domain::NewFeedSubscription {
            title: Some("Retained".to_string()),
            ..library::subscription("https://example.com/retained.xml")
        })
        .await
        .expect("seed retained feed");
    let removed_feed = library
        .feeds
        .upsert_subscription(&rssr_domain::NewFeedSubscription {
            title: Some("Removed".to_string()),
            ..library::subscription("https://example.com/removed.xml")
        })
        .await
        .expect("seed removed feed");
    set_last_opened(library, retained_feed.id).await;

    fixture
        .workflow
        .remove_subscription(RemoveSubscriptionInput {
            feed_id: removed_feed.id,
            purge_entries: false,
        })
        .await
        .expect("remove non-current feed");

    assert_eq!(last_opened(library).await, Some(retained_feed.id));
}
