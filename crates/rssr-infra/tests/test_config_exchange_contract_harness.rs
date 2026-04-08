use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use rssr_application::{FeedRemovalCleanupPort, ImportExportService, RemoteConfigStore};
use rssr_domain::{
    ConfigFeed, ConfigPackage, DomainError, Entry, EntryNavigation, EntryQuery, EntryRepository,
    Feed, FeedRepository, FeedSummary, NewFeedSubscription, Result as DomainResult,
    SettingsRepository, UserSettings, normalize_feed_url,
};
use rssr_infra::{
    application_adapters::{InfraOpmlCodec, SqliteAppStateAdapter},
    db::{
        app_state_repository::SqliteAppStateRepository, entry_repository::SqliteEntryRepository,
        feed_repository::SqliteFeedRepository, migrate,
        settings_repository::SqliteSettingsRepository, sqlite_native::NativeSqliteBackend,
        storage_backend::StorageBackend,
    },
    opml::OpmlCodec,
    parser::ParsedEntry,
};
use time::OffsetDateTime;
use url::Url;

#[derive(Clone)]
struct MemoryRemoteConfigStore {
    payload: Arc<Mutex<Option<String>>>,
}

impl Default for MemoryRemoteConfigStore {
    fn default() -> Self {
        Self { payload: Arc::new(Mutex::new(None)) }
    }
}

impl MemoryRemoteConfigStore {
    fn set_payload(&self, payload: Option<String>) {
        *self.payload.lock().expect("lock payload") = payload;
    }
}

#[async_trait::async_trait]
impl RemoteConfigStore for MemoryRemoteConfigStore {
    async fn upload_config(&self, raw: &str) -> Result<()> {
        self.set_payload(Some(raw.to_string()));
        Ok(())
    }

    async fn download_config(&self) -> Result<Option<String>> {
        Ok(self.payload.lock().expect("lock payload").clone())
    }
}

#[async_trait::async_trait]
trait FixtureProbe: Send + Sync {
    async fn active_feed_urls(&self) -> Result<Vec<String>>;
    async fn feed_id_by_url(&self, url: &str) -> Result<Option<i64>>;
    async fn entry_count(&self, feed_id: i64) -> Result<usize>;
    async fn last_opened_feed_id(&self) -> Result<Option<i64>>;
    async fn load_settings(&self) -> Result<UserSettings>;
}

struct ContractFixture {
    name: &'static str,
    service: ImportExportService,
    probe: Arc<dyn FixtureProbe>,
    remote: MemoryRemoteConfigStore,
}

#[derive(Clone, Copy)]
enum FixtureKind {
    Sqlite,
    BrowserState,
}

async fn build_fixture(
    kind: FixtureKind,
    feed_urls: &[&str],
    entry_feed_url: Option<&str>,
    last_opened_feed_url: Option<&str>,
    settings: UserSettings,
) -> Result<ContractFixture> {
    match kind {
        FixtureKind::Sqlite => {
            build_sqlite_fixture(feed_urls, entry_feed_url, last_opened_feed_url, settings).await
        }
        FixtureKind::BrowserState => {
            build_browser_state_fixture(feed_urls, entry_feed_url, last_opened_feed_url, settings)
                .await
        }
    }
}

async fn build_sqlite_fixture(
    feed_urls: &[&str],
    entry_feed_url: Option<&str>,
    last_opened_feed_url: Option<&str>,
    settings: UserSettings,
) -> Result<ContractFixture> {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.context("connect sqlite memory")?;
    migrate(&pool).await.context("run sqlite migrations")?;

    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
    let settings_repository = Arc::new(SqliteSettingsRepository::new(pool.clone()));
    let app_state_repository = Arc::new(SqliteAppStateRepository::new(pool));
    settings_repository.save(&settings).await.context("seed sqlite settings")?;

    let mut feed_id_by_url = HashMap::new();
    for raw_url in feed_urls {
        let feed = feed_repository
            .upsert_subscription(&NewFeedSubscription {
                url: Url::parse(raw_url).with_context(|| format!("parse seeded url: {raw_url}"))?,
                title: None,
                folder: None,
            })
            .await
            .with_context(|| format!("seed sqlite feed: {raw_url}"))?;
        feed_id_by_url.insert((*raw_url).to_string(), feed.id);
    }

    if let Some(entry_feed_url) = entry_feed_url {
        let feed_id = *feed_id_by_url
            .get(entry_feed_url)
            .with_context(|| format!("missing seeded feed: {entry_feed_url}"))?;
        entry_repository
            .upsert_entries(
                feed_id,
                &[ParsedEntry {
                    external_id: "seed-entry".to_string(),
                    dedup_key: "seed-entry".to_string(),
                    url: Some(Url::parse("https://example.com/articles/seed").expect("valid url")),
                    title: "seed entry".to_string(),
                    author: None,
                    summary: Some("seed summary".to_string()),
                    content_html: None,
                    content_text: Some("seed summary".to_string()),
                    published_at: Some(OffsetDateTime::UNIX_EPOCH),
                    updated_at_source: None,
                }],
            )
            .await
            .context("seed sqlite entry")?;
    }

    if let Some(last_opened_feed_url) = last_opened_feed_url {
        let feed_id = *feed_id_by_url
            .get(last_opened_feed_url)
            .with_context(|| format!("missing last-opened feed: {last_opened_feed_url}"))?;
        app_state_repository
            .save_last_opened_feed_id(Some(feed_id))
            .await
            .context("seed sqlite app state")?;
    }

    Ok(ContractFixture {
        name: "sqlite",
        service: ImportExportService::new_with_feed_removal_cleanup(
            feed_repository.clone(),
            entry_repository.clone(),
            settings_repository.clone(),
            Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
            Arc::new(SqliteAppStateAdapter::new(app_state_repository.clone())),
        ),
        probe: Arc::new(SqliteFixtureProbe {
            feed_repository,
            entry_repository,
            app_state_repository,
            settings_repository,
        }),
        remote: MemoryRemoteConfigStore::default(),
    })
}

struct SqliteFixtureProbe {
    feed_repository: Arc<SqliteFeedRepository>,
    entry_repository: Arc<SqliteEntryRepository>,
    app_state_repository: Arc<SqliteAppStateRepository>,
    settings_repository: Arc<SqliteSettingsRepository>,
}

#[async_trait::async_trait]
impl FixtureProbe for SqliteFixtureProbe {
    async fn active_feed_urls(&self) -> Result<Vec<String>> {
        let mut urls = self
            .feed_repository
            .list_feeds()
            .await?
            .into_iter()
            .map(|feed| feed.url.to_string())
            .collect::<Vec<_>>();
        urls.sort();
        Ok(urls)
    }

    async fn feed_id_by_url(&self, url: &str) -> Result<Option<i64>> {
        Ok(self
            .feed_repository
            .list_feeds()
            .await?
            .into_iter()
            .find(|feed| feed.url.as_str() == url)
            .map(|feed| feed.id))
    }

    async fn entry_count(&self, feed_id: i64) -> Result<usize> {
        Ok(self
            .entry_repository
            .list_entries(&EntryQuery { feed_id: Some(feed_id), ..EntryQuery::default() })
            .await?
            .len())
    }

    async fn last_opened_feed_id(&self) -> Result<Option<i64>> {
        self.app_state_repository.load_last_opened_feed_id().await.map_err(Into::into)
    }

    async fn load_settings(&self) -> Result<UserSettings> {
        self.settings_repository.load().await.map_err(Into::into)
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

#[derive(Debug, Clone)]
struct BrowserState {
    next_feed_id: i64,
    feeds: Vec<BrowserFeed>,
    entries: Vec<BrowserEntry>,
    settings: UserSettings,
    last_opened_feed_id: Option<i64>,
}

impl BrowserState {
    fn snapshot(&self) -> Self {
        self.clone()
    }
}

trait SnapshotWriter: Send + Sync {
    fn write(&self, state: &BrowserState) -> Result<()>;
}

#[derive(Default)]
struct MemorySnapshotWriter {
    snapshots: Mutex<Vec<BrowserState>>,
}

impl SnapshotWriter for MemorySnapshotWriter {
    fn write(&self, state: &BrowserState) -> Result<()> {
        self.snapshots.lock().expect("lock snapshots").push(state.snapshot());
        Ok(())
    }
}

#[derive(Clone)]
struct BrowserFixtureStore {
    state: Arc<Mutex<BrowserState>>,
    writer: Arc<dyn SnapshotWriter>,
}

impl BrowserFixtureStore {
    fn new(state: Arc<Mutex<BrowserState>>, writer: Arc<dyn SnapshotWriter>) -> Self {
        Self { state, writer }
    }

    fn persist(&self, state: &BrowserState) -> DomainResult<()> {
        self.writer.write(state).map_err(|error| {
            DomainError::Persistence(format!("写入 browser fixture 快照失败：{error}"))
        })
    }

    fn map_feed(feed: &BrowserFeed) -> DomainResult<Feed> {
        Ok(Feed {
            id: feed.id,
            url: Url::parse(&feed.url).map_err(|error| {
                DomainError::Persistence(format!(
                    "browser fixture feed url 无效：{} ({error})",
                    feed.url
                ))
            })?,
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
impl FeedRepository for BrowserFixtureStore {
    async fn upsert_subscription(&self, new_feed: &NewFeedSubscription) -> DomainResult<Feed> {
        let normalized_url = normalize_feed_url(&new_feed.url).to_string();
        let now = OffsetDateTime::now_utc();
        let snapshot = {
            let mut state = self.state.lock().expect("lock browser state");
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
            state.snapshot()
        };
        self.persist(&snapshot)?;
        let feed = snapshot
            .feeds
            .iter()
            .find(|feed| feed.url == normalized_url && !feed.is_deleted)
            .ok_or_else(|| DomainError::Persistence("upserted browser feed missing".to_string()))?;
        Self::map_feed(feed)
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
            state.snapshot()
        };
        self.persist(&snapshot)
    }

    async fn list_feeds(&self) -> DomainResult<Vec<Feed>> {
        let state = self.state.lock().expect("lock browser state");
        let mut feeds = state
            .feeds
            .iter()
            .filter(|feed| !feed.is_deleted)
            .map(Self::map_feed)
            .collect::<DomainResult<Vec<_>>>()?;
        feeds.sort_by(|left, right| left.url.as_str().cmp(right.url.as_str()));
        Ok(feeds)
    }

    async fn get_feed(&self, feed_id: i64) -> DomainResult<Option<Feed>> {
        let state = self.state.lock().expect("lock browser state");
        match state.feeds.iter().find(|feed| feed.id == feed_id && !feed.is_deleted) {
            Some(feed) => Ok(Some(Self::map_feed(feed)?)),
            None => Ok(None),
        }
    }

    async fn list_summaries(&self) -> DomainResult<Vec<FeedSummary>> {
        Ok(Vec::new())
    }
}

#[async_trait::async_trait]
impl EntryRepository for BrowserFixtureStore {
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
            state.snapshot()
        };
        self.persist(&snapshot)
    }
}

#[async_trait::async_trait]
impl SettingsRepository for BrowserFixtureStore {
    async fn load(&self) -> DomainResult<UserSettings> {
        Ok(self.state.lock().expect("lock browser state").settings.clone())
    }

    async fn save(&self, settings: &UserSettings) -> DomainResult<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock browser state");
            state.settings = settings.clone();
            state.snapshot()
        };
        self.persist(&snapshot)
    }
}

#[async_trait::async_trait]
impl FeedRemovalCleanupPort for BrowserFixtureStore {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock browser state");
            if state.last_opened_feed_id == Some(feed_id) {
                state.last_opened_feed_id = None;
                Some(state.snapshot())
            } else {
                None
            }
        };

        if let Some(snapshot) = snapshot {
            self.writer.write(&snapshot)?;
        }
        Ok(())
    }
}

struct BrowserFixtureProbe {
    state: Arc<Mutex<BrowserState>>,
}

#[async_trait::async_trait]
impl FixtureProbe for BrowserFixtureProbe {
    async fn active_feed_urls(&self) -> Result<Vec<String>> {
        let state = self.state.lock().expect("lock browser state");
        let mut urls = state
            .feeds
            .iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| feed.url.clone())
            .collect::<Vec<_>>();
        urls.sort();
        Ok(urls)
    }

    async fn feed_id_by_url(&self, url: &str) -> Result<Option<i64>> {
        let state = self.state.lock().expect("lock browser state");
        Ok(state.feeds.iter().find(|feed| !feed.is_deleted && feed.url == url).map(|feed| feed.id))
    }

    async fn entry_count(&self, feed_id: i64) -> Result<usize> {
        let state = self.state.lock().expect("lock browser state");
        Ok(state.entries.iter().filter(|entry| entry.feed_id == feed_id).count())
    }

    async fn last_opened_feed_id(&self) -> Result<Option<i64>> {
        Ok(self.state.lock().expect("lock browser state").last_opened_feed_id)
    }

    async fn load_settings(&self) -> Result<UserSettings> {
        Ok(self.state.lock().expect("lock browser state").settings.clone())
    }
}

async fn build_browser_state_fixture(
    feed_urls: &[&str],
    entry_feed_url: Option<&str>,
    last_opened_feed_url: Option<&str>,
    settings: UserSettings,
) -> Result<ContractFixture> {
    let mut feeds = Vec::with_capacity(feed_urls.len());
    let mut feed_id_by_url = HashMap::new();

    for (idx, raw_url) in feed_urls.iter().enumerate() {
        let normalized = normalize_feed_url(
            &Url::parse(raw_url).with_context(|| format!("parse seeded url: {raw_url}"))?,
        )
        .to_string();
        let feed_id = idx as i64 + 1;
        feed_id_by_url.insert(normalized.clone(), feed_id);
        feeds.push(BrowserFeed {
            id: feed_id,
            url: normalized,
            title: None,
            folder: None,
            is_deleted: false,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        });
    }

    let mut entries = Vec::new();
    if let Some(entry_feed_url) = entry_feed_url {
        let normalized = normalize_feed_url(
            &Url::parse(entry_feed_url)
                .with_context(|| format!("parse entry feed url: {entry_feed_url}"))?,
        )
        .to_string();
        let feed_id = *feed_id_by_url
            .get(&normalized)
            .with_context(|| format!("missing entry feed: {entry_feed_url}"))?;
        entries.push(BrowserEntry { feed_id });
    }

    let last_opened_feed_id = match last_opened_feed_url {
        Some(url) => {
            let normalized = normalize_feed_url(
                &Url::parse(url).with_context(|| format!("parse last-opened url: {url}"))?,
            )
            .to_string();
            Some(
                *feed_id_by_url
                    .get(&normalized)
                    .with_context(|| format!("missing last-opened feed: {url}"))?,
            )
        }
        None => None,
    };

    let state = Arc::new(Mutex::new(BrowserState {
        next_feed_id: feed_urls.len() as i64,
        feeds,
        entries,
        settings,
        last_opened_feed_id,
    }));
    let store = Arc::new(BrowserFixtureStore::new(
        state.clone(),
        Arc::new(MemorySnapshotWriter::default()),
    ));

    Ok(ContractFixture {
        name: "browser_state",
        service: ImportExportService::new_with_feed_removal_cleanup(
            store.clone(),
            store.clone(),
            store.clone(),
            Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
            store,
        ),
        probe: Arc::new(BrowserFixtureProbe { state }),
        remote: MemoryRemoteConfigStore::default(),
    })
}

#[tokio::test]
async fn contract_json_import_export_roundtrip() -> Result<()> {
    let expected_settings = UserSettings {
        refresh_interval_minutes: 15,
        archive_after_months: 6,
        ..UserSettings::default()
    };

    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture = build_fixture(
            kind,
            &["https://example.com/a.xml", "https://example.com/b.xml"],
            None,
            None,
            expected_settings.clone(),
        )
        .await?;
        let before_urls = fixture.probe.active_feed_urls().await?;

        let raw = fixture.service.export_config_json().await?;
        fixture.service.import_config_json(&raw).await?;

        assert_eq!(fixture.probe.active_feed_urls().await?, before_urls, "{}", fixture.name);
        assert_eq!(fixture.probe.load_settings().await?, expected_settings, "{}", fixture.name);
    }

    Ok(())
}

#[tokio::test]
async fn contract_opml_import_export() -> Result<()> {
    let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Added" title="Added" type="rss" xmlUrl="https://example.com/new.xml" />
    </outline>
  </body>
</opml>"#;

    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture = build_fixture(
            kind,
            &["https://example.com/original.xml"],
            None,
            None,
            UserSettings::default(),
        )
        .await?;

        let exported = fixture.service.export_opml().await?;
        assert!(exported.contains("<opml"), "{}: export should produce OPML", fixture.name);

        fixture.service.import_opml(opml).await?;
        assert_eq!(
            fixture.probe.active_feed_urls().await?,
            vec![
                "https://example.com/new.xml".to_string(),
                "https://example.com/original.xml".to_string(),
            ],
            "{}",
            fixture.name
        );
    }

    Ok(())
}

#[tokio::test]
async fn contract_remote_pull_removes_feed_and_cleans_last_opened_state() -> Result<()> {
    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture = build_fixture(
            kind,
            &["https://example.com/keep.xml", "https://example.com/drop.xml"],
            Some("https://example.com/drop.xml"),
            Some("https://example.com/drop.xml"),
            UserSettings::default(),
        )
        .await?;
        let dropped_feed_id = fixture
            .probe
            .feed_id_by_url("https://example.com/drop.xml")
            .await?
            .context("missing dropped feed")?;

        fixture.remote.set_payload(Some(
            serde_json::to_string(&ConfigPackage {
                version: 1,
                exported_at: OffsetDateTime::UNIX_EPOCH,
                feeds: vec![ConfigFeed {
                    url: "https://example.com/keep.xml".to_string(),
                    title: None,
                    folder: None,
                }],
                settings: UserSettings::default(),
            })
            .context("serialize remote package")?,
        ));

        let pulled = fixture.service.pull_remote_config(&fixture.remote).await?;
        assert!(pulled, "{}: expected remote payload to be applied", fixture.name);
        assert_eq!(
            fixture.probe.active_feed_urls().await?,
            vec!["https://example.com/keep.xml".to_string()],
            "{}",
            fixture.name
        );
        assert_eq!(fixture.probe.entry_count(dropped_feed_id).await?, 0, "{}", fixture.name);
        assert_eq!(fixture.probe.last_opened_feed_id().await?, None, "{}", fixture.name);
    }

    Ok(())
}

#[tokio::test]
async fn contract_import_rejects_invalid_settings_boundary() -> Result<()> {
    for kind in [FixtureKind::Sqlite, FixtureKind::BrowserState] {
        let fixture = build_fixture(
            kind,
            &["https://example.com/only.xml"],
            None,
            None,
            UserSettings::default(),
        )
        .await?;
        let invalid_settings =
            UserSettings { refresh_interval_minutes: 0, ..UserSettings::default() };

        let error = fixture
            .service
            .import_config_package(&ConfigPackage {
                version: 1,
                exported_at: OffsetDateTime::UNIX_EPOCH,
                feeds: vec![ConfigFeed {
                    url: "https://example.com/only.xml".to_string(),
                    title: None,
                    folder: None,
                }],
                settings: invalid_settings,
            })
            .await
            .expect_err("invalid settings should be rejected");

        assert!(
            error.to_string().contains("刷新间隔必须大于等于 1 分钟"),
            "{}: unexpected error message: {error}",
            fixture.name
        );
    }

    Ok(())
}
