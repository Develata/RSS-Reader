use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshAllInput, RefreshCommit, RefreshFailure, RefreshHttpMetadata,
    RefreshService, RefreshStorePort, RefreshTarget,
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

#[derive(Debug, Clone)]
struct FeedProbe {
    title: Option<String>,
    description: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
    fetch_error: Option<String>,
    has_last_fetched_at: bool,
    has_last_success_at: bool,
}

#[async_trait::async_trait]
trait FixtureProbe: Send + Sync {
    async fn feed(&self, feed_id: i64) -> Result<FeedProbe>;
    async fn entry_count(&self, feed_id: i64) -> Result<usize>;
}

struct ContractFixture {
    name: &'static str,
    service: RefreshService,
    probe: Arc<dyn FixtureProbe>,
    feed_ids: Vec<i64>,
}

#[derive(Clone, Copy)]
enum FixtureKind {
    Sqlite,
    BrowserState,
}

async fn build_fixture(
    kind: FixtureKind,
    feed_urls: &[&str],
    outputs: Vec<FeedRefreshSourceOutput>,
) -> Result<ContractFixture> {
    match kind {
        FixtureKind::Sqlite => build_sqlite_fixture(feed_urls, outputs).await,
        FixtureKind::BrowserState => build_browser_state_fixture(feed_urls, outputs).await,
    }
}

async fn build_sqlite_fixture(
    feed_urls: &[&str],
    outputs: Vec<FeedRefreshSourceOutput>,
) -> Result<ContractFixture> {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.context("connect sqlite memory")?;
    migrate(&pool).await.context("run migrations")?;

    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entry_repository = Arc::new(SqliteEntryRepository::new(pool));
    let mut feed_ids = Vec::with_capacity(feed_urls.len());
    for raw in feed_urls {
        let feed = feed_repository
            .upsert_subscription(&NewFeedSubscription {
                url: Url::parse(raw).context("parse seeded feed url")?,
                title: None,
                folder: None,
            })
            .await
            .context("seed feed")?;
        feed_ids.push(feed.id);
    }

    Ok(ContractFixture {
        name: "sqlite",
        service: RefreshService::new(
            Arc::new(ScriptedSource { outputs: Arc::new(Mutex::new(outputs)) }),
            Arc::new(SqliteRefreshStore::new(feed_repository.clone(), entry_repository.clone())),
        ),
        probe: Arc::new(SqliteProbe { feed_repository, entry_repository }),
        feed_ids,
    })
}

struct SqliteProbe {
    feed_repository: Arc<SqliteFeedRepository>,
    entry_repository: Arc<SqliteEntryRepository>,
}

#[async_trait::async_trait]
impl FixtureProbe for SqliteProbe {
    async fn feed(&self, feed_id: i64) -> Result<FeedProbe> {
        let feed = self
            .feed_repository
            .get_feed(feed_id)
            .await?
            .with_context(|| format!("feed {feed_id} should exist"))?;
        Ok(FeedProbe {
            title: feed.title,
            description: feed.description,
            etag: feed.etag,
            last_modified: feed.last_modified,
            fetch_error: feed.fetch_error,
            has_last_fetched_at: feed.last_fetched_at.is_some(),
            has_last_success_at: feed.last_success_at.is_some(),
        })
    }

    async fn entry_count(&self, feed_id: i64) -> Result<usize> {
        Ok(self
            .entry_repository
            .list_entries(&EntryQuery { feed_id: Some(feed_id), ..EntryQuery::default() })
            .await?
            .len())
    }
}

#[derive(Debug, Clone)]
struct BrowserFeed {
    id: i64,
    url: String,
    title: Option<String>,
    description: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
    last_fetched_at: Option<OffsetDateTime>,
    last_success_at: Option<OffsetDateTime>,
    fetch_error: Option<String>,
    is_deleted: bool,
}

#[derive(Debug, Clone)]
struct BrowserEntry {
    feed_id: i64,
    dedup_key: String,
}

#[derive(Debug, Clone, Default)]
struct BrowserState {
    feeds: Vec<BrowserFeed>,
    entries: Vec<BrowserEntry>,
}

#[derive(Clone)]
struct BrowserStateStore {
    state: Arc<Mutex<BrowserState>>,
}

impl BrowserStateStore {
    fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl RefreshStorePort for BrowserStateStore {
    async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
        let state = self.state.lock().expect("lock state");
        state
            .feeds
            .iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| {
                Ok(RefreshTarget {
                    feed_id: feed.id,
                    url: Url::parse(&feed.url)
                        .with_context(|| format!("订阅 URL 不合法：{}", feed.url))?,
                    etag: feed.etag.clone(),
                    last_modified: feed.last_modified.clone(),
                })
            })
            .collect()
    }

    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
        let state = self.state.lock().expect("lock state");
        state
            .feeds
            .iter()
            .find(|feed| feed.id == feed_id && !feed.is_deleted)
            .map(|feed| {
                Ok(RefreshTarget {
                    feed_id: feed.id,
                    url: Url::parse(&feed.url)
                        .with_context(|| format!("订阅 URL 不合法：{}", feed.url))?,
                    etag: feed.etag.clone(),
                    last_modified: feed.last_modified.clone(),
                })
            })
            .transpose()
    }

    async fn commit(&self, feed_id: i64, commit: RefreshCommit) -> Result<()> {
        let mut state = self.state.lock().expect("lock state");
        let feed_index = state
            .feeds
            .iter()
            .position(|feed| feed.id == feed_id && !feed.is_deleted)
            .context("订阅不存在")?;
        let now = OffsetDateTime::now_utc();

        match commit {
            RefreshCommit::NotModified { metadata } => {
                let feed = &mut state.feeds[feed_index];
                feed.etag = metadata.etag;
                feed.last_modified = metadata.last_modified;
                feed.last_fetched_at = Some(now);
                feed.last_success_at = Some(now);
                feed.fetch_error = None;
            }
            RefreshCommit::Updated { update } => {
                {
                    let feed = &mut state.feeds[feed_index];
                    if let Some(title) = update.feed.title {
                        feed.title = Some(title);
                    }
                    if let Some(description) = update.feed.description {
                        feed.description = Some(description);
                    }
                    feed.etag = update.metadata.etag;
                    feed.last_modified = update.metadata.last_modified;
                    feed.last_fetched_at = Some(now);
                    feed.last_success_at = Some(now);
                    feed.fetch_error = None;
                }

                for entry in update.feed.entries {
                    if let Some(existing) = state.entries.iter_mut().find(|current| {
                        current.feed_id == feed_id && current.dedup_key == entry.dedup_key
                    }) {
                        existing.dedup_key = entry.dedup_key;
                    } else {
                        state.entries.push(BrowserEntry { feed_id, dedup_key: entry.dedup_key });
                    }
                }
            }
            RefreshCommit::Failed { failure } => {
                let feed = &mut state.feeds[feed_index];
                if let Some(metadata) = failure.metadata {
                    feed.etag = metadata.etag;
                    feed.last_modified = metadata.last_modified;
                }
                feed.last_fetched_at = Some(now);
                feed.fetch_error = Some(failure.message);
            }
        }

        Ok(())
    }
}

struct BrowserStateProbe {
    state: Arc<Mutex<BrowserState>>,
}

#[async_trait::async_trait]
impl FixtureProbe for BrowserStateProbe {
    async fn feed(&self, feed_id: i64) -> Result<FeedProbe> {
        let state = self.state.lock().expect("lock state");
        let feed = state
            .feeds
            .iter()
            .find(|feed| feed.id == feed_id)
            .with_context(|| format!("feed {feed_id} should exist"))?;
        Ok(FeedProbe {
            title: feed.title.clone(),
            description: feed.description.clone(),
            etag: feed.etag.clone(),
            last_modified: feed.last_modified.clone(),
            fetch_error: feed.fetch_error.clone(),
            has_last_fetched_at: feed.last_fetched_at.is_some(),
            has_last_success_at: feed.last_success_at.is_some(),
        })
    }

    async fn entry_count(&self, feed_id: i64) -> Result<usize> {
        let state = self.state.lock().expect("lock state");
        Ok(state.entries.iter().filter(|entry| entry.feed_id == feed_id).count())
    }
}

async fn build_browser_state_fixture(
    feed_urls: &[&str],
    outputs: Vec<FeedRefreshSourceOutput>,
) -> Result<ContractFixture> {
    let feed_ids = (0..feed_urls.len()).map(|idx| idx as i64 + 1).collect::<Vec<_>>();
    let state = Arc::new(Mutex::new(BrowserState {
        feeds: feed_urls
            .iter()
            .enumerate()
            .map(|(idx, url)| BrowserFeed {
                id: idx as i64 + 1,
                url: (*url).to_string(),
                title: None,
                description: None,
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
            })
            .collect(),
        entries: Vec::new(),
    }));

    Ok(ContractFixture {
        name: "browser_state",
        service: RefreshService::new(
            Arc::new(ScriptedSource { outputs: Arc::new(Mutex::new(outputs)) }),
            Arc::new(BrowserStateStore::new(state.clone())),
        ),
        probe: Arc::new(BrowserStateProbe { state }),
        feed_ids,
    })
}

fn sample_entry(dedup_key: &str) -> ParsedEntryData {
    ParsedEntryData {
        external_id: dedup_key.to_string(),
        dedup_key: dedup_key.to_string(),
        url: Some(Url::parse("https://example.com/articles/entry").expect("valid url")),
        title: format!("title-{dedup_key}"),
        author: Some("author".to_string()),
        summary: Some("summary".to_string()),
        content_html: Some("<p>summary</p>".to_string()),
        content_text: Some("summary".to_string()),
        published_at: None,
        updated_at_source: None,
    }
}

#[tokio::test]
async fn contract_single_feed_refresh_success() -> Result<()> {
    let outputs = vec![FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
        metadata: RefreshHttpMetadata {
            etag: Some("etag-1".to_string()),
            last_modified: Some("Wed, 01 Apr 2026 10:00:00 GMT".to_string()),
        },
        feed: ParsedFeedUpdate {
            title: Some("Example Feed".to_string()),
            site_url: Some(Url::parse("https://example.com").expect("valid url")),
            description: Some("Example description".to_string()),
            entries: vec![sample_entry("entry-1")],
        },
    })];

    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture =
            build_fixture(kind, &["https://example.com/feed.xml"], outputs.clone()).await?;
        let feed_id = fixture.feed_ids[0];
        let outcome = fixture.service.refresh_feed(feed_id).await?;

        assert!(
            matches!(
                outcome.result,
                rssr_application::RefreshFeedResult::Updated { entry_count: 1, .. }
            ),
            "{}: expected updated outcome",
            fixture.name
        );

        let feed = fixture.probe.feed(feed_id).await?;
        assert_eq!(feed.title.as_deref(), Some("Example Feed"), "{}", fixture.name);
        assert_eq!(feed.description.as_deref(), Some("Example description"), "{}", fixture.name);
        assert_eq!(feed.etag.as_deref(), Some("etag-1"), "{}", fixture.name);
        assert!(feed.has_last_fetched_at, "{}", fixture.name);
        assert!(feed.has_last_success_at, "{}", fixture.name);
        assert_eq!(fixture.probe.entry_count(feed_id).await?, 1, "{}", fixture.name);
    }

    Ok(())
}

#[tokio::test]
async fn contract_single_feed_refresh_failure() -> Result<()> {
    let outputs = vec![FeedRefreshSourceOutput::Failed(RefreshFailure {
        message: "抓取订阅失败: timeout".to_string(),
        metadata: Some(RefreshHttpMetadata {
            etag: Some("etag-fail".to_string()),
            last_modified: None,
        }),
    })];

    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture =
            build_fixture(kind, &["https://example.com/feed.xml"], outputs.clone()).await?;
        let feed_id = fixture.feed_ids[0];
        let outcome = fixture.service.refresh_feed(feed_id).await?;

        assert_eq!(outcome.failure_message(), Some("抓取订阅失败: timeout"), "{}", fixture.name);

        let feed = fixture.probe.feed(feed_id).await?;
        assert_eq!(feed.fetch_error.as_deref(), Some("抓取订阅失败: timeout"), "{}", fixture.name);
        assert_eq!(feed.etag.as_deref(), Some("etag-fail"), "{}", fixture.name);
        assert!(feed.has_last_fetched_at, "{}", fixture.name);
    }

    Ok(())
}

#[tokio::test]
async fn contract_single_feed_not_modified() -> Result<()> {
    let outputs = vec![FeedRefreshSourceOutput::NotModified(RefreshHttpMetadata {
        etag: Some("etag-not-modified".to_string()),
        last_modified: Some("Wed, 01 Apr 2026 10:00:00 GMT".to_string()),
    })];

    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture =
            build_fixture(kind, &["https://example.com/feed.xml"], outputs.clone()).await?;
        let feed_id = fixture.feed_ids[0];
        let outcome = fixture.service.refresh_feed(feed_id).await?;

        assert!(
            matches!(outcome.result, rssr_application::RefreshFeedResult::NotModified),
            "{}: expected not modified",
            fixture.name
        );

        let feed = fixture.probe.feed(feed_id).await?;
        assert_eq!(feed.etag.as_deref(), Some("etag-not-modified"), "{}", fixture.name);
        assert_eq!(
            feed.last_modified.as_deref(),
            Some("Wed, 01 Apr 2026 10:00:00 GMT"),
            "{}",
            fixture.name
        );
        assert!(feed.has_last_fetched_at, "{}", fixture.name);
        assert!(feed.has_last_success_at, "{}", fixture.name);
        assert_eq!(feed.fetch_error, None, "{}", fixture.name);
        assert_eq!(fixture.probe.entry_count(feed_id).await?, 0, "{}", fixture.name);
    }

    Ok(())
}

#[tokio::test]
async fn contract_refresh_all_aggregates_mixed_results() -> Result<()> {
    let outputs = vec![
        FeedRefreshSourceOutput::NotModified(RefreshHttpMetadata::default()),
        FeedRefreshSourceOutput::Failed(RefreshFailure {
            message: "boom".to_string(),
            metadata: None,
        }),
    ];

    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture = build_fixture(
            kind,
            &["https://example.com/one.xml", "https://example.com/two.xml"],
            outputs.clone(),
        )
        .await?;

        let outcome = fixture.service.refresh_all(RefreshAllInput { max_concurrency: 1 }).await?;
        assert_eq!(outcome.not_modified_count(), 1, "{}", fixture.name);
        assert_eq!(outcome.failures().len(), 1, "{}", fixture.name);

        let failed_feed = fixture.probe.feed(fixture.feed_ids[1]).await?;
        assert_eq!(failed_feed.fetch_error.as_deref(), Some("boom"), "{}", fixture.name);
    }

    Ok(())
}
