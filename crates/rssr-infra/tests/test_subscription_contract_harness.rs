use std::sync::Arc;

use anyhow::{Result, bail};
use rssr_application::{
    AddSubscriptionInput, FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedService,
    RefreshCommit, RefreshService, RefreshStorePort, RemoveSubscriptionInput, SubscriptionWorkflow,
};
use rssr_domain::{EntryQuery, EntryRepository, FeedRepository, NewFeedSubscription};
use rssr_infra::{
    application_adapters::SqliteAppStateAdapter,
    db::{
        app_state_repository::SqliteAppStateRepository, entry_repository::SqliteEntryRepository,
        feed_repository::SqliteFeedRepository, migrate, sqlite_native::NativeSqliteBackend,
        storage_backend::StorageBackend,
    },
    parser::ParsedEntry,
};
use sqlx::Row;
use time::OffsetDateTime;
use url::Url;

struct UnusedRefreshSource;

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

    async fn commit(&self, _feed_id: i64, _commit: RefreshCommit) -> Result<()> {
        bail!("refresh store should not be used in subscription harness")
    }
}

struct SqliteFixture {
    workflow: SubscriptionWorkflow,
    feed_repository: Arc<SqliteFeedRepository>,
    entry_repository: Arc<SqliteEntryRepository>,
    app_state_repository: Arc<SqliteAppStateRepository>,
    pool: rssr_infra::db::SqlitePool,
}

async fn build_sqlite_fixture() -> Result<SqliteFixture> {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await?;
    migrate(&pool).await?;

    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
    let app_state_repository = Arc::new(SqliteAppStateRepository::new(pool.clone()));
    let app_state_adapter = Arc::new(SqliteAppStateAdapter::new(app_state_repository.clone()));
    let feed_service = FeedService::new(feed_repository.clone(), entry_repository.clone());
    let refresh_service =
        RefreshService::new(Arc::new(UnusedRefreshSource), Arc::new(UnusedRefreshStore));
    let workflow = SubscriptionWorkflow::new(feed_service, refresh_service, app_state_adapter);

    Ok(SqliteFixture { workflow, feed_repository, entry_repository, app_state_repository, pool })
}

fn sample_entry(index: usize) -> ParsedEntry {
    ParsedEntry {
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
    let fixture = build_sqlite_fixture().await.expect("build fixture");

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
        .expect("add normalized duplicate");

    assert_eq!(first.id, second.id);
    assert_eq!(second.url.as_str(), "https://example.com/feed.xml");

    let feeds = fixture.feed_repository.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].id, first.id);
    assert_eq!(feeds[0].url.as_str(), "https://example.com/feed.xml");
    assert_eq!(feeds[0].title.as_deref(), Some("Updated Title"));
    assert_eq!(feeds[0].folder.as_deref(), Some("Reading"));
}

#[tokio::test]
async fn subscription_contract_remove_purges_entries_soft_deletes_feed_and_clears_matching_state() {
    let fixture = build_sqlite_fixture().await.expect("build fixture");

    let feed = fixture
        .feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example".to_string()),
            folder: None,
        })
        .await
        .expect("seed feed");
    fixture
        .entry_repository
        .upsert_entries(feed.id, &[sample_entry(1), sample_entry(2)])
        .await
        .expect("seed entries");
    fixture
        .app_state_repository
        .save_last_opened_feed_id(Some(feed.id))
        .await
        .expect("save matching last opened feed");

    fixture
        .workflow
        .remove_subscription(RemoveSubscriptionInput { feed_id: feed.id, purge_entries: true })
        .await
        .expect("remove subscription");

    assert!(fixture.feed_repository.get_feed(feed.id).await.expect("get feed").is_none());
    assert_eq!(
        fixture
            .entry_repository
            .list_entries(&EntryQuery { feed_id: Some(feed.id), ..EntryQuery::default() })
            .await
            .expect("list entries")
            .len(),
        0
    );
    assert_eq!(
        fixture.app_state_repository.load_last_opened_feed_id().await.expect("load app state"),
        None
    );

    let deleted_flag = sqlx::query("SELECT is_deleted FROM feeds WHERE id = ?1")
        .bind(feed.id)
        .fetch_one(&fixture.pool)
        .await
        .expect("load deleted feed row")
        .get::<i64, _>("is_deleted");
    assert_eq!(deleted_flag, 1);
}

#[tokio::test]
async fn subscription_contract_remove_preserves_other_last_opened_feed() {
    let fixture = build_sqlite_fixture().await.expect("build fixture");

    let retained_feed = fixture
        .feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/retained.xml").expect("valid url"),
            title: Some("Retained".to_string()),
            folder: None,
        })
        .await
        .expect("seed retained feed");
    let removed_feed = fixture
        .feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/removed.xml").expect("valid url"),
            title: Some("Removed".to_string()),
            folder: None,
        })
        .await
        .expect("seed removed feed");
    fixture
        .app_state_repository
        .save_last_opened_feed_id(Some(retained_feed.id))
        .await
        .expect("save non-matching last opened feed");

    fixture
        .workflow
        .remove_subscription(RemoveSubscriptionInput {
            feed_id: removed_feed.id,
            purge_entries: false,
        })
        .await
        .expect("remove non-current feed");

    assert_eq!(
        fixture.app_state_repository.load_last_opened_feed_id().await.expect("load app state"),
        Some(retained_feed.id)
    );
}
