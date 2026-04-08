use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use rssr_application::{
    AddSubscriptionInput, FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedService,
    RefreshCommit, RefreshService, RefreshStorePort, RefreshTarget, RemoveSubscriptionInput,
    SubscriptionWorkflow,
};
use rssr_domain::{
    DomainError, Entry, EntryNavigation, EntryQuery, EntryRepository, Feed, FeedRepository,
    FeedSummary, NewFeedSubscription, Result as DomainResult, normalize_feed_url,
};
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

#[derive(Clone)]
struct NoopRefreshSource;

#[async_trait::async_trait]
impl FeedRefreshSourcePort for NoopRefreshSource {
    async fn refresh(&self, _target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
        anyhow::bail!("add/remove contract should not invoke refresh source")
    }
}

#[derive(Clone)]
struct NoopRefreshStore;

#[async_trait::async_trait]
impl RefreshStorePort for NoopRefreshStore {
    async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
        Ok(Vec::new())
    }

    async fn get_target(&self, _feed_id: i64) -> Result<Option<RefreshTarget>> {
        Ok(None)
    }

    async fn commit(&self, _feed_id: i64, _commit: RefreshCommit) -> Result<()> {
        Ok(())
    }
}

fn build_noop_refresh_service() -> RefreshService {
    RefreshService::new(Arc::new(NoopRefreshSource), Arc::new(NoopRefreshStore))
}

#[async_trait::async_trait]
trait FixtureProbe: Send + Sync {
    async fn feed_id_by_url(&self, normalized_url: &str) -> Result<Option<i64>>;
    async fn feed_row_count_by_url(&self, normalized_url: &str) -> Result<usize>;
    async fn is_deleted(&self, feed_id: i64) -> Result<bool>;
    async fn entry_count(&self, feed_id: i64) -> Result<usize>;
    async fn last_opened_feed_id(&self) -> Result<Option<i64>>;
}

struct ContractFixture {
    name: &'static str,
    workflow: SubscriptionWorkflow,
    probe: Arc<dyn FixtureProbe>,
}

#[derive(Clone, Copy)]
enum FixtureKind {
    Sqlite,
    BrowserState,
}

async fn build_fixture(
    kind: FixtureKind,
    seeded_feeds: &[&str],
    seed_entry_for: Option<&str>,
    last_opened_for: Option<&str>,
) -> Result<ContractFixture> {
    match kind {
        FixtureKind::Sqlite => {
            build_sqlite_fixture(seeded_feeds, seed_entry_for, last_opened_for).await
        }
        FixtureKind::BrowserState => {
            build_browser_state_fixture(seeded_feeds, seed_entry_for, last_opened_for).await
        }
    }
}

async fn build_sqlite_fixture(
    seeded_feeds: &[&str],
    seed_entry_for: Option<&str>,
    last_opened_for: Option<&str>,
) -> Result<ContractFixture> {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.context("connect sqlite memory")?;
    migrate(&pool).await.context("run sqlite migrations")?;

    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
    let app_state_repository = Arc::new(SqliteAppStateRepository::new(pool.clone()));
    let mut seeded_ids_by_url = HashMap::new();

    for raw_url in seeded_feeds {
        let normalized = normalize_url(raw_url)?;
        let feed = feed_repository
            .upsert_subscription(&NewFeedSubscription {
                url: Url::parse(raw_url).with_context(|| format!("parse seed url: {raw_url}"))?,
                title: None,
                folder: None,
            })
            .await
            .with_context(|| format!("seed sqlite feed: {raw_url}"))?;
        seeded_ids_by_url.insert(normalized, feed.id);
    }

    if let Some(raw_url) = seed_entry_for {
        let normalized = normalize_url(raw_url)?;
        let feed_id = *seeded_ids_by_url
            .get(&normalized)
            .with_context(|| format!("missing feed seed: {raw_url}"))?;
        entry_repository
            .upsert_entries(
                feed_id,
                &[ParsedEntry {
                    external_id: "seed-entry".to_string(),
                    dedup_key: "seed-entry".to_string(),
                    url: Some(Url::parse("https://example.com/articles/seed").expect("valid url")),
                    title: "seed".to_string(),
                    author: None,
                    summary: Some("seed".to_string()),
                    content_html: None,
                    content_text: Some("seed".to_string()),
                    published_at: Some(OffsetDateTime::UNIX_EPOCH),
                    updated_at_source: None,
                }],
            )
            .await
            .context("seed sqlite entry")?;
    }

    if let Some(raw_url) = last_opened_for {
        let normalized = normalize_url(raw_url)?;
        let feed_id = *seeded_ids_by_url
            .get(&normalized)
            .with_context(|| format!("missing last-opened seed: {raw_url}"))?;
        app_state_repository
            .save_last_opened_feed_id(Some(feed_id))
            .await
            .context("seed sqlite last_opened_feed_id")?;
    }

    let workflow = SubscriptionWorkflow::new(
        FeedService::new(feed_repository.clone(), entry_repository.clone()),
        build_noop_refresh_service(),
        Arc::new(SqliteAppStateAdapter::new(app_state_repository.clone())),
    );

    Ok(ContractFixture {
        name: "sqlite",
        workflow,
        probe: Arc::new(SqliteProbe { pool, app_state_repository }),
    })
}

struct SqliteProbe {
    pool: sqlx::SqlitePool,
    app_state_repository: Arc<SqliteAppStateRepository>,
}

#[async_trait::async_trait]
impl FixtureProbe for SqliteProbe {
    async fn feed_id_by_url(&self, normalized_url: &str) -> Result<Option<i64>> {
        Ok(sqlx::query("SELECT id FROM feeds WHERE url = ?1 AND is_deleted = 0")
            .bind(normalized_url)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| row.get::<i64, _>("id")))
    }

    async fn feed_row_count_by_url(&self, normalized_url: &str) -> Result<usize> {
        Ok(sqlx::query("SELECT COUNT(*) AS count FROM feeds WHERE url = ?1")
            .bind(normalized_url)
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("count") as usize)
    }

    async fn is_deleted(&self, feed_id: i64) -> Result<bool> {
        Ok(sqlx::query("SELECT is_deleted FROM feeds WHERE id = ?1")
            .bind(feed_id)
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("is_deleted")
            != 0)
    }

    async fn entry_count(&self, feed_id: i64) -> Result<usize> {
        Ok(sqlx::query("SELECT COUNT(*) AS count FROM entries WHERE feed_id = ?1")
            .bind(feed_id)
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("count") as usize)
    }

    async fn last_opened_feed_id(&self) -> Result<Option<i64>> {
        self.app_state_repository.load_last_opened_feed_id().await.map_err(Into::into)
    }
}

#[derive(Debug, Clone)]
struct BrowserFeed {
    id: i64,
    url: String,
    title: Option<String>,
    folder: Option<String>,
    is_deleted: bool,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
struct BrowserEntry {
    feed_id: i64,
}

#[derive(Debug, Clone, Default)]
struct BrowserState {
    next_feed_id: i64,
    feeds: Vec<BrowserFeed>,
    entries: Vec<BrowserEntry>,
    last_opened_feed_id: Option<i64>,
}

trait BrowserSnapshotWriter: Send + Sync {
    fn write(&self, state: BrowserState) -> Result<()>;
}

#[derive(Default)]
struct MemorySnapshotWriter;

impl BrowserSnapshotWriter for MemorySnapshotWriter {
    fn write(&self, _state: BrowserState) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
struct BrowserStore {
    state: Arc<Mutex<BrowserState>>,
    writer: Arc<dyn BrowserSnapshotWriter>,
}

impl BrowserStore {
    fn new(state: Arc<Mutex<BrowserState>>, writer: Arc<dyn BrowserSnapshotWriter>) -> Self {
        Self { state, writer }
    }

    fn write_snapshot(&self, state: BrowserState) -> DomainResult<()> {
        self.writer.write(state).map_err(|error| {
            DomainError::Persistence(format!("write browser snapshot failed: {error}"))
        })
    }

    fn map_feed(feed: &BrowserFeed) -> DomainResult<Feed> {
        Ok(Feed {
            id: feed.id,
            url: Url::parse(&feed.url)
                .map_err(|error| DomainError::Persistence(format!("invalid feed url: {error}")))?,
            title: feed.title.clone(),
            site_url: None,
            description: None,
            icon_url: None,
            folder: feed.folder.clone(),
            etag: None,
            last_modified: None,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
            is_deleted: feed.is_deleted,
            created_at: feed.created_at,
            updated_at: feed.updated_at,
        })
    }
}

#[async_trait::async_trait]
impl FeedRepository for BrowserStore {
    async fn upsert_subscription(&self, new_feed: &NewFeedSubscription) -> DomainResult<Feed> {
        let normalized_url = normalize_feed_url(&new_feed.url).to_string();
        let snapshot = {
            let mut state = self.state.lock().expect("lock browser state");
            let now = OffsetDateTime::now_utc();
            if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.url == normalized_url) {
                if let Some(title) = new_feed.title.as_ref() {
                    feed.title = (!title.is_empty()).then_some(title.clone());
                }
                if let Some(folder) = new_feed.folder.as_ref() {
                    feed.folder = (!folder.is_empty()).then_some(folder.clone());
                }
                feed.is_deleted = false;
                feed.updated_at = now;
            } else {
                state.next_feed_id += 1;
                let feed_id = state.next_feed_id;
                state.feeds.push(BrowserFeed {
                    id: feed_id,
                    url: normalized_url.clone(),
                    title: new_feed.title.clone(),
                    folder: new_feed.folder.clone(),
                    is_deleted: false,
                    created_at: now,
                    updated_at: now,
                });
            }
            state.clone()
        };
        let feed = snapshot
            .feeds
            .iter()
            .find(|feed| feed.url == normalized_url && !feed.is_deleted)
            .ok_or_else(|| DomainError::Persistence("upserted feed missing".to_string()))
            .and_then(Self::map_feed)?;
        self.write_snapshot(snapshot)?;
        Ok(feed)
    }

    async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> DomainResult<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock browser state");
            let feed = state
                .feeds
                .iter_mut()
                .find(|feed| feed.id == feed_id)
                .ok_or(DomainError::NotFound)?;
            feed.is_deleted = is_deleted;
            feed.updated_at = OffsetDateTime::now_utc();
            state.clone()
        };
        self.write_snapshot(snapshot)
    }

    async fn list_feeds(&self) -> DomainResult<Vec<Feed>> {
        let state = self.state.lock().expect("lock browser state");
        state.feeds.iter().filter(|feed| !feed.is_deleted).map(Self::map_feed).collect()
    }

    async fn get_feed(&self, feed_id: i64) -> DomainResult<Option<Feed>> {
        let state = self.state.lock().expect("lock browser state");
        state
            .feeds
            .iter()
            .find(|feed| feed.id == feed_id && !feed.is_deleted)
            .map(Self::map_feed)
            .transpose()
    }

    async fn list_summaries(&self) -> DomainResult<Vec<FeedSummary>> {
        Ok(Vec::new())
    }
}

#[async_trait::async_trait]
impl EntryRepository for BrowserStore {
    async fn list_entries(
        &self,
        _query: &EntryQuery,
    ) -> DomainResult<Vec<rssr_domain::EntrySummary>> {
        Ok(Vec::new())
    }

    async fn get_entry(&self, _entry_id: i64) -> DomainResult<Option<Entry>> {
        Ok(None)
    }

    async fn reader_navigation(&self, _current_entry_id: i64) -> DomainResult<EntryNavigation> {
        Ok(EntryNavigation::default())
    }

    async fn set_read(&self, _entry_id: i64, _is_read: bool) -> DomainResult<()> {
        Ok(())
    }

    async fn set_starred(&self, _entry_id: i64, _is_starred: bool) -> DomainResult<()> {
        Ok(())
    }

    async fn delete_for_feed(&self, feed_id: i64) -> DomainResult<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock browser state");
            state.entries.retain(|entry| entry.feed_id != feed_id);
            state.clone()
        };
        self.write_snapshot(snapshot)
    }
}

#[async_trait::async_trait]
impl rssr_application::AppStatePort for BrowserStore {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock browser state");
            if state.last_opened_feed_id == Some(feed_id) {
                state.last_opened_feed_id = None;
                Some(state.clone())
            } else {
                None
            }
        };
        if let Some(snapshot) = snapshot {
            self.writer.write(snapshot)?;
        }
        Ok(())
    }
}

struct BrowserProbe {
    state: Arc<Mutex<BrowserState>>,
}

#[async_trait::async_trait]
impl FixtureProbe for BrowserProbe {
    async fn feed_id_by_url(&self, normalized_url: &str) -> Result<Option<i64>> {
        let state = self.state.lock().expect("lock browser state");
        Ok(state
            .feeds
            .iter()
            .find(|feed| feed.url == normalized_url && !feed.is_deleted)
            .map(|feed| feed.id))
    }

    async fn feed_row_count_by_url(&self, normalized_url: &str) -> Result<usize> {
        let state = self.state.lock().expect("lock browser state");
        Ok(state.feeds.iter().filter(|feed| feed.url == normalized_url).count())
    }

    async fn is_deleted(&self, feed_id: i64) -> Result<bool> {
        let state = self.state.lock().expect("lock browser state");
        let feed = state
            .feeds
            .iter()
            .find(|feed| feed.id == feed_id)
            .with_context(|| format!("feed {feed_id} should exist"))?;
        Ok(feed.is_deleted)
    }

    async fn entry_count(&self, feed_id: i64) -> Result<usize> {
        let state = self.state.lock().expect("lock browser state");
        Ok(state.entries.iter().filter(|entry| entry.feed_id == feed_id).count())
    }

    async fn last_opened_feed_id(&self) -> Result<Option<i64>> {
        Ok(self.state.lock().expect("lock browser state").last_opened_feed_id)
    }
}

async fn build_browser_state_fixture(
    seeded_feeds: &[&str],
    seed_entry_for: Option<&str>,
    last_opened_for: Option<&str>,
) -> Result<ContractFixture> {
    let mut feeds = Vec::with_capacity(seeded_feeds.len());
    let mut seeded_ids_by_url = HashMap::new();
    for (idx, raw_url) in seeded_feeds.iter().enumerate() {
        let normalized = normalize_url(raw_url)?;
        let feed_id = idx as i64 + 1;
        feeds.push(BrowserFeed {
            id: feed_id,
            url: normalized.clone(),
            title: None,
            folder: None,
            is_deleted: false,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        });
        seeded_ids_by_url.insert(normalized, feed_id);
    }

    let mut entries = Vec::new();
    if let Some(raw_url) = seed_entry_for {
        let normalized = normalize_url(raw_url)?;
        let feed_id = *seeded_ids_by_url
            .get(&normalized)
            .with_context(|| format!("missing seed feed: {raw_url}"))?;
        entries.push(BrowserEntry { feed_id });
    }

    let last_opened_feed_id = match last_opened_for {
        Some(raw_url) => {
            let normalized = normalize_url(raw_url)?;
            Some(
                *seeded_ids_by_url
                    .get(&normalized)
                    .with_context(|| format!("missing last-opened seed: {raw_url}"))?,
            )
        }
        None => None,
    };

    let state = Arc::new(Mutex::new(BrowserState {
        next_feed_id: seeded_feeds.len() as i64,
        feeds,
        entries,
        last_opened_feed_id,
    }));
    let store = Arc::new(BrowserStore::new(state.clone(), Arc::new(MemorySnapshotWriter)));
    let workflow = SubscriptionWorkflow::new(
        FeedService::new(store.clone(), store.clone()),
        build_noop_refresh_service(),
        store,
    );

    Ok(ContractFixture { name: "browser_state", workflow, probe: Arc::new(BrowserProbe { state }) })
}

fn normalize_url(raw: &str) -> Result<String> {
    Ok(normalize_feed_url(&Url::parse(raw).with_context(|| format!("parse url: {raw}"))?)
        .to_string())
}

#[tokio::test]
async fn contract_add_subscription_normalizes_url() -> Result<()> {
    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture = build_fixture(kind, &[], None, None).await?;
        let feed = fixture
            .workflow
            .add_subscription(&AddSubscriptionInput {
                url: "https://example.com:443/feed.xml#top".to_string(),
                title: None,
                folder: None,
            })
            .await?;

        let normalized = "https://example.com/feed.xml";
        assert_eq!(feed.url.as_str(), normalized, "{}", fixture.name);
        assert_eq!(
            fixture.probe.feed_id_by_url(normalized).await?,
            Some(feed.id),
            "{}",
            fixture.name
        );
        assert_eq!(fixture.probe.feed_row_count_by_url(normalized).await?, 1, "{}", fixture.name);
    }

    Ok(())
}

#[tokio::test]
async fn contract_add_subscription_reactivates_soft_deleted_feed() -> Result<()> {
    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture = build_fixture(kind, &[], None, None).await?;
        let first = fixture
            .workflow
            .add_subscription(&AddSubscriptionInput {
                url: "https://example.com/feed.xml".to_string(),
                title: None,
                folder: None,
            })
            .await?;

        fixture
            .workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id: first.id, purge_entries: true })
            .await?;
        let second = fixture
            .workflow
            .add_subscription(&AddSubscriptionInput {
                url: "https://example.com:443/feed.xml#new".to_string(),
                title: None,
                folder: None,
            })
            .await?;

        let normalized = "https://example.com/feed.xml";
        assert_eq!(second.id, first.id, "{}", fixture.name);
        assert_eq!(
            fixture.probe.feed_id_by_url(normalized).await?,
            Some(first.id),
            "{}",
            fixture.name
        );
        assert_eq!(fixture.probe.feed_row_count_by_url(normalized).await?, 1, "{}", fixture.name);
        assert!(!fixture.probe.is_deleted(first.id).await?, "{}", fixture.name);
    }

    Ok(())
}

#[tokio::test]
async fn contract_remove_subscription_purges_entries_and_soft_deletes_feed() -> Result<()> {
    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture = build_fixture(
            kind,
            &["https://example.com/feed.xml"],
            Some("https://example.com/feed.xml"),
            None,
        )
        .await?;
        let feed_id = fixture
            .probe
            .feed_id_by_url("https://example.com/feed.xml")
            .await?
            .context("seeded feed should exist")?;

        fixture
            .workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id, purge_entries: true })
            .await?;

        assert!(fixture.probe.is_deleted(feed_id).await?, "{}", fixture.name);
        assert_eq!(fixture.probe.entry_count(feed_id).await?, 0, "{}", fixture.name);
        assert_eq!(
            fixture.probe.feed_id_by_url("https://example.com/feed.xml").await?,
            None,
            "{}",
            fixture.name
        );
    }

    Ok(())
}

#[tokio::test]
async fn contract_remove_subscription_clears_last_opened_feed() -> Result<()> {
    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture = build_fixture(
            kind,
            &["https://example.com/feed.xml"],
            None,
            Some("https://example.com/feed.xml"),
        )
        .await?;
        let feed_id = fixture
            .probe
            .feed_id_by_url("https://example.com/feed.xml")
            .await?
            .context("seeded feed should exist")?;

        fixture
            .workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id, purge_entries: true })
            .await?;

        assert_eq!(fixture.probe.last_opened_feed_id().await?, None, "{}", fixture.name);
    }

    Ok(())
}
