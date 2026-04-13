use std::sync::{Arc, Mutex};

use rssr_domain::{
    ConfigFeed, ConfigPackage, Entry, EntryNavigation, EntryQuery, EntryRepository, Feed,
    FeedRepository, FeedSummary, NewFeedSubscription, SettingsRepository, UserSettings,
};
use time::OffsetDateTime;
use url::Url;

use crate::AppStatePort;

use super::{ClockPort, ImportExportService, OpmlCodecPort, RemoteConfigStore};

struct StubRemoteConfigStore {
    payload: Mutex<Option<String>>,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigStore for StubRemoteConfigStore {
    async fn upload_config(&self, raw: &str) -> anyhow::Result<()> {
        *self.payload.lock().expect("lock payload") = Some(raw.to_string());
        Ok(())
    }

    async fn download_config(&self) -> anyhow::Result<Option<String>> {
        Ok(self.payload.lock().expect("lock payload").clone())
    }
}

#[derive(Default)]
struct RecordingOpmlCodec {
    decoded: Mutex<Vec<ConfigFeed>>,
    encoded_feeds: Mutex<Vec<ConfigFeed>>,
}

impl RecordingOpmlCodec {
    fn with_decoded(feeds: Vec<ConfigFeed>) -> Self {
        Self { decoded: Mutex::new(feeds), encoded_feeds: Mutex::new(Vec::new()) }
    }
}

impl OpmlCodecPort for RecordingOpmlCodec {
    fn encode(&self, feeds: &[ConfigFeed]) -> anyhow::Result<String> {
        *self.encoded_feeds.lock().expect("lock encoded feeds") = feeds.to_vec();
        Ok("<opml />".to_string())
    }

    fn decode(&self, _raw: &str) -> anyhow::Result<Vec<ConfigFeed>> {
        Ok(self.decoded.lock().expect("lock decoded feeds").clone())
    }
}

struct StubFeedRepository {
    feeds: Vec<Feed>,
}

#[async_trait::async_trait]
impl FeedRepository for StubFeedRepository {
    async fn upsert_subscription(
        &self,
        _new_feed: &NewFeedSubscription,
    ) -> rssr_domain::Result<Feed> {
        panic!("upsert_subscription should not be used in this test");
    }

    async fn set_deleted(&self, _feed_id: i64, _is_deleted: bool) -> rssr_domain::Result<()> {
        Ok(())
    }

    async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
        Ok(self.feeds.clone())
    }

    async fn get_feed(&self, feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
        Ok(self.feeds.iter().find(|feed| feed.id == feed_id).cloned())
    }

    async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
        Ok(Vec::new())
    }
}

struct StubEntryRepository;

#[async_trait::async_trait]
impl EntryRepository for StubEntryRepository {
    async fn list_entries(
        &self,
        _query: &EntryQuery,
    ) -> rssr_domain::Result<Vec<rssr_domain::EntrySummary>> {
        Ok(Vec::new())
    }

    async fn get_entry(&self, _entry_id: i64) -> rssr_domain::Result<Option<Entry>> {
        Ok(None)
    }

    async fn reader_navigation(
        &self,
        _current_entry_id: i64,
    ) -> rssr_domain::Result<EntryNavigation> {
        Ok(EntryNavigation::default())
    }

    async fn set_read(&self, _entry_id: i64, _is_read: bool) -> rssr_domain::Result<()> {
        Ok(())
    }

    async fn set_starred(&self, _entry_id: i64, _is_starred: bool) -> rssr_domain::Result<()> {
        Ok(())
    }

    async fn delete_for_feed(&self, _feed_id: i64) -> rssr_domain::Result<()> {
        Ok(())
    }
}

struct StubSettingsRepository {
    settings: UserSettings,
}

#[async_trait::async_trait]
impl SettingsRepository for StubSettingsRepository {
    async fn load(&self) -> rssr_domain::Result<UserSettings> {
        Ok(self.settings.clone())
    }

    async fn save(&self, _settings: &UserSettings) -> rssr_domain::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct RecordingAppStateCleanup {
    removed_feed_ids: Mutex<Vec<i64>>,
}

#[async_trait::async_trait]
impl AppStatePort for RecordingAppStateCleanup {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> anyhow::Result<()> {
        self.removed_feed_ids.lock().expect("lock removed ids").push(feed_id);
        Ok(())
    }
}

struct FixedClock {
    now: OffsetDateTime,
}

impl ClockPort for FixedClock {
    fn now_utc(&self) -> OffsetDateTime {
        self.now
    }
}

#[test]
fn config_import_outcome_summary_line_formats_settings_state() {
    assert_eq!(
        super::ConfigImportOutcome {
            imported_feed_count: 2,
            removed_feed_count: 1,
            settings_updated: true,
        }
        .summary_line(),
        "导入 2 个订阅，清理 1 个缺失订阅，设置已更新"
    );
    assert_eq!(
        super::ConfigImportOutcome {
            imported_feed_count: 0,
            removed_feed_count: 0,
            settings_updated: false,
        }
        .summary_line(),
        "导入 0 个订阅，清理 0 个缺失订阅，设置未变化"
    );
}

struct MemoryFeedRepository {
    feeds: Mutex<Vec<Feed>>,
}

#[async_trait::async_trait]
impl FeedRepository for MemoryFeedRepository {
    async fn upsert_subscription(
        &self,
        new_feed: &NewFeedSubscription,
    ) -> rssr_domain::Result<Feed> {
        let mut feeds = self.feeds.lock().expect("lock feeds");
        if let Some(feed) = feeds.iter_mut().find(|feed| feed.url == new_feed.url) {
            if let Some(title) = &new_feed.title {
                feed.title = (!title.is_empty()).then_some(title.clone());
            }
            if let Some(folder) = &new_feed.folder {
                feed.folder = (!folder.is_empty()).then_some(folder.clone());
            }
            feed.is_deleted = false;
            return Ok(feed.clone());
        }

        let now = OffsetDateTime::UNIX_EPOCH;
        let feed = Feed {
            id: feeds.len() as i64 + 1,
            url: new_feed.url.clone(),
            title: new_feed.title.clone(),
            site_url: None,
            description: None,
            icon_url: None,
            folder: new_feed.folder.clone(),
            etag: None,
            last_modified: None,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
            is_deleted: false,
            created_at: now,
            updated_at: now,
        };
        feeds.push(feed.clone());
        Ok(feed)
    }

    async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> rssr_domain::Result<()> {
        let mut feeds = self.feeds.lock().expect("lock feeds");
        let feed = feeds.iter_mut().find(|feed| feed.id == feed_id).expect("feed exists");
        feed.is_deleted = is_deleted;
        Ok(())
    }

    async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
        Ok(self.feeds.lock().expect("lock feeds").clone())
    }

    async fn get_feed(&self, feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
        Ok(self.feeds.lock().expect("lock feeds").iter().find(|feed| feed.id == feed_id).cloned())
    }

    async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
        Ok(Vec::new())
    }
}

struct MemoryEntryRepository {
    deleted_feed_ids: Mutex<Vec<i64>>,
}

#[async_trait::async_trait]
impl EntryRepository for MemoryEntryRepository {
    async fn list_entries(
        &self,
        _query: &EntryQuery,
    ) -> rssr_domain::Result<Vec<rssr_domain::EntrySummary>> {
        Ok(Vec::new())
    }

    async fn get_entry(&self, _entry_id: i64) -> rssr_domain::Result<Option<Entry>> {
        Ok(None)
    }

    async fn reader_navigation(
        &self,
        _current_entry_id: i64,
    ) -> rssr_domain::Result<EntryNavigation> {
        Ok(EntryNavigation::default())
    }

    async fn set_read(&self, _entry_id: i64, _is_read: bool) -> rssr_domain::Result<()> {
        Ok(())
    }

    async fn set_starred(&self, _entry_id: i64, _is_starred: bool) -> rssr_domain::Result<()> {
        Ok(())
    }

    async fn delete_for_feed(&self, feed_id: i64) -> rssr_domain::Result<()> {
        self.deleted_feed_ids.lock().expect("lock deleted feed ids").push(feed_id);
        Ok(())
    }
}

struct MemorySettingsRepository {
    settings: Mutex<UserSettings>,
}

#[async_trait::async_trait]
impl SettingsRepository for MemorySettingsRepository {
    async fn load(&self) -> rssr_domain::Result<UserSettings> {
        Ok(self.settings.lock().expect("lock settings").clone())
    }

    async fn save(&self, settings: &UserSettings) -> rssr_domain::Result<()> {
        *self.settings.lock().expect("lock settings") = settings.clone();
        Ok(())
    }
}

#[tokio::test]
async fn export_config_contains_active_feeds_and_settings() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let service = ImportExportService::new_with_app_state_cleanup_and_clock(
        Arc::new(StubFeedRepository {
            feeds: vec![
                Feed {
                    id: 1,
                    url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                    title: Some("Example".to_string()),
                    site_url: None,
                    description: None,
                    icon_url: None,
                    folder: Some("Tech".to_string()),
                    etag: None,
                    last_modified: None,
                    last_fetched_at: None,
                    last_success_at: None,
                    fetch_error: None,
                    is_deleted: false,
                    created_at: now,
                    updated_at: now,
                },
                Feed {
                    id: 2,
                    url: Url::parse("https://example.com/deleted.xml").expect("valid url"),
                    title: Some("Deleted".to_string()),
                    site_url: None,
                    description: None,
                    icon_url: None,
                    folder: Some("Archive".to_string()),
                    etag: None,
                    last_modified: None,
                    last_fetched_at: None,
                    last_success_at: None,
                    fetch_error: None,
                    is_deleted: true,
                    created_at: now,
                    updated_at: now,
                },
            ],
        }),
        Arc::new(StubEntryRepository),
        Arc::new(StubSettingsRepository { settings: UserSettings::default() }),
        Arc::new(RecordingOpmlCodec::default()),
        Arc::new(RecordingAppStateCleanup::default()),
        Arc::new(FixedClock { now }),
    );

    let exported = service.export_config().await.expect("export config");

    assert_eq!(exported.version, 2);
    assert_eq!(exported.exported_at, now);
    assert_eq!(exported.feeds.len(), 1);
    assert_eq!(exported.feeds[0].url, "https://example.com/feed.xml");
    assert_eq!(exported.feeds[0].title.as_deref(), Some("Example"));
    assert_eq!(exported.feeds[0].folder.as_deref(), Some("Tech"));
}

#[tokio::test]
async fn remote_config_roundtrip_uses_json_payload() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let service = ImportExportService::new(
        Arc::new(MemoryFeedRepository {
            feeds: Mutex::new(vec![Feed {
                id: 1,
                url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                title: Some("Example".to_string()),
                site_url: None,
                description: None,
                icon_url: None,
                folder: Some("Tech".to_string()),
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
                created_at: now,
                updated_at: now,
            }]),
        }),
        Arc::new(MemoryEntryRepository { deleted_feed_ids: Mutex::new(Vec::new()) }),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        Arc::new(RecordingOpmlCodec::default()),
    );
    let remote = StubRemoteConfigStore { payload: Mutex::new(None) };

    let pushed = service.push_remote_config(&remote).await.expect("push config");
    let pulled = service.pull_remote_config(&remote).await.expect("pull config");

    assert_eq!(pushed.exported_feed_count, 1);
    assert!(pulled.found());
    assert!(
        remote
            .payload
            .lock()
            .expect("lock payload")
            .as_ref()
            .expect("payload exists")
            .contains("\"feeds\"")
    );
}

#[tokio::test]
async fn pull_remote_config_reports_missing_remote_payload() {
    let service = ImportExportService::new(
        Arc::new(MemoryFeedRepository { feeds: Mutex::new(Vec::new()) }),
        Arc::new(MemoryEntryRepository { deleted_feed_ids: Mutex::new(Vec::new()) }),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        Arc::new(RecordingOpmlCodec::default()),
    );
    let remote = StubRemoteConfigStore { payload: Mutex::new(None) };

    let pulled = service.pull_remote_config(&remote).await.expect("pull missing config");

    assert!(!pulled.found());
    assert_eq!(pulled.import(), None);
}

#[tokio::test]
async fn import_config_clears_removed_feed_entries_and_metadata() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let feed_repository = Arc::new(MemoryFeedRepository {
        feeds: Mutex::new(vec![
            Feed {
                id: 1,
                url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                title: Some("Legacy".to_string()),
                site_url: None,
                description: None,
                icon_url: None,
                folder: Some("Archive".to_string()),
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
                created_at: now,
                updated_at: now,
            },
            Feed {
                id: 2,
                url: Url::parse("https://stale.example.com/rss").expect("valid url"),
                title: Some("Stale".to_string()),
                site_url: None,
                description: None,
                icon_url: None,
                folder: None,
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
                created_at: now,
                updated_at: now,
            },
        ]),
    });
    let entry_repository =
        Arc::new(MemoryEntryRepository { deleted_feed_ids: Mutex::new(Vec::new()) });
    let cleanup = Arc::new(RecordingAppStateCleanup::default());
    let mut imported_settings = UserSettings::default();
    imported_settings.refresh_interval_minutes += 1;
    let service = ImportExportService::new_with_app_state_cleanup(
        feed_repository.clone(),
        entry_repository.clone(),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        Arc::new(RecordingOpmlCodec::default()),
        cleanup.clone(),
    );

    let outcome = service
        .import_config_package(&ConfigPackage {
            version: 2,
            exported_at: OffsetDateTime::UNIX_EPOCH,
            feeds: vec![ConfigFeed {
                url: "https://example.com/feed.xml".to_string(),
                title: None,
                folder: None,
            }],
            settings: imported_settings,
        })
        .await
        .expect("import package");

    assert_eq!(outcome.imported_feed_count, 1);
    assert_eq!(outcome.removed_feed_count, 1);
    assert!(outcome.settings_updated);

    let feeds = feed_repository.list_feeds().await.expect("list feeds");
    let retained = feeds.iter().find(|feed| feed.id == 1).expect("retained feed exists");
    let removed = feeds.iter().find(|feed| feed.id == 2).expect("removed feed exists");
    assert_eq!(retained.title, None);
    assert_eq!(retained.folder, None);
    assert!(removed.is_deleted);
    assert_eq!(
        entry_repository.deleted_feed_ids.lock().expect("lock deleted ids").as_slice(),
        &[2]
    );
    assert_eq!(cleanup.removed_feed_ids.lock().expect("lock removed ids").as_slice(), &[2]);
}

#[tokio::test]
async fn pull_remote_config_cleans_up_removed_feed_app_state() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let feed_repository = Arc::new(MemoryFeedRepository {
        feeds: Mutex::new(vec![
            Feed {
                id: 1,
                url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                title: Some("Example".to_string()),
                site_url: None,
                description: None,
                icon_url: None,
                folder: Some("Tech".to_string()),
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
                created_at: now,
                updated_at: now,
            },
            Feed {
                id: 2,
                url: Url::parse("https://stale.example.com/rss").expect("valid url"),
                title: Some("Stale".to_string()),
                site_url: None,
                description: None,
                icon_url: None,
                folder: None,
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
                created_at: now,
                updated_at: now,
            },
        ]),
    });
    let entry_repository =
        Arc::new(MemoryEntryRepository { deleted_feed_ids: Mutex::new(Vec::new()) });
    let cleanup = Arc::new(RecordingAppStateCleanup::default());
    let service = ImportExportService::new_with_app_state_cleanup(
        feed_repository,
        entry_repository.clone(),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        Arc::new(RecordingOpmlCodec::default()),
        cleanup.clone(),
    );
    let remote = StubRemoteConfigStore {
        payload: Mutex::new(Some(
            serde_json::to_string(&ConfigPackage {
                version: 2,
                exported_at: OffsetDateTime::UNIX_EPOCH,
                feeds: vec![ConfigFeed {
                    url: "https://example.com/feed.xml".to_string(),
                    title: None,
                    folder: None,
                }],
                settings: UserSettings::default(),
            })
            .expect("serialize package"),
        )),
    };

    let pulled = service.pull_remote_config(&remote).await.expect("pull remote config");

    assert!(pulled.found());
    assert_eq!(pulled.import.as_ref().expect("import outcome").removed_feed_count, 1);
    assert_eq!(
        entry_repository.deleted_feed_ids.lock().expect("lock deleted ids").as_slice(),
        &[2]
    );
    assert_eq!(cleanup.removed_feed_ids.lock().expect("lock removed ids").as_slice(), &[2]);
}

#[tokio::test]
async fn export_opml_uses_config_export_as_source_of_truth() {
    let codec = Arc::new(RecordingOpmlCodec::default());
    let now = OffsetDateTime::UNIX_EPOCH;
    let service = ImportExportService::new(
        Arc::new(StubFeedRepository {
            feeds: vec![Feed {
                id: 1,
                url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                title: Some("Example".to_string()),
                site_url: None,
                description: None,
                icon_url: None,
                folder: Some("Tech".to_string()),
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
                created_at: now,
                updated_at: now,
            }],
        }),
        Arc::new(StubEntryRepository),
        Arc::new(StubSettingsRepository { settings: UserSettings::default() }),
        codec.clone(),
    );

    let raw = service.export_opml().await.expect("export opml");

    assert_eq!(raw, "<opml />");
    assert_eq!(codec.encoded_feeds.lock().expect("lock encoded feeds").len(), 1);
    assert_eq!(
        codec.encoded_feeds.lock().expect("lock encoded feeds")[0].url,
        "https://example.com/feed.xml"
    );
}

#[tokio::test]
async fn import_opml_upserts_normalized_feeds() {
    let feed_repository = Arc::new(MemoryFeedRepository { feeds: Mutex::new(Vec::new()) });
    let service = ImportExportService::new(
        feed_repository.clone(),
        Arc::new(MemoryEntryRepository { deleted_feed_ids: Mutex::new(Vec::new()) }),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        Arc::new(RecordingOpmlCodec::with_decoded(vec![ConfigFeed {
            url: "https://example.com:443/feed.xml#top".to_string(),
            title: Some("Example".to_string()),
            folder: Some("Tech".to_string()),
        }])),
    );

    let outcome = service.import_opml("<opml />").await.expect("import opml");

    let feeds = feed_repository.list_feeds().await.expect("list feeds");
    assert_eq!(outcome.imported_feed_count, 1);
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].url.as_str(), "https://example.com/feed.xml");
    assert_eq!(feeds[0].folder.as_deref(), Some("Tech"));
}
