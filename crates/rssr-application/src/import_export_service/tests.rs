use std::sync::{Arc, Mutex};

use rssr_domain::{
    ConfigFeed, ConfigFingerprint, ConfigPackage, Feed, FeedRepository, FeedSummary,
    NewFeedSubscription, OpmlFeed, SettingsRepository, UserSettings,
};
use time::OffsetDateTime;
use url::Url;

use crate::library_queries::test_support::RepositoryQueries;
use crate::persistence_mutation::{
    ConfigReplacementOutcome, ConfigReplacementPlan, ConfigReplacementPort,
};

use super::{ClockPort, ImportExportService, OpmlCodecPort, RemoteConfigStore};

/// Records the plans it is handed; reports one removed subscription and changed settings.
#[derive(Default)]
struct RecordingConfigReplacement {
    plans: Mutex<Vec<ConfigReplacementPlan>>,
}

#[async_trait::async_trait]
impl ConfigReplacementPort for RecordingConfigReplacement {
    async fn replace_config(
        &self,
        plan: ConfigReplacementPlan,
    ) -> anyhow::Result<ConfigReplacementOutcome> {
        let settings = plan.settings.clone();
        self.plans.lock().expect("lock plans").push(plan);
        Ok(ConfigReplacementOutcome { removed_feed_count: 1, settings_updated: true, settings })
    }
}

/// The service over these ports, a fixed clock and a recording replacement port.
fn service(
    feeds: Arc<dyn FeedRepository>,
    settings: Arc<dyn SettingsRepository>,
    codec: Arc<dyn OpmlCodecPort>,
) -> (ImportExportService, Arc<RecordingConfigReplacement>) {
    let replacement = Arc::new(RecordingConfigReplacement::default());
    let queries = RepositoryQueries {
        feeds: Some(feeds.clone()),
        settings: Some(settings),
        ..RepositoryQueries::default()
    };
    let service = ImportExportService::new(
        feeds,
        Arc::new(queries),
        codec,
        Arc::new(FixedClock { now: OffsetDateTime::UNIX_EPOCH }),
        replacement.clone(),
    );
    (service, replacement)
}

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
    encoded_feeds: Mutex<Vec<OpmlFeed>>,
}

impl RecordingOpmlCodec {
    fn with_decoded(feeds: Vec<ConfigFeed>) -> Self {
        Self { decoded: Mutex::new(feeds), encoded_feeds: Mutex::new(Vec::new()) }
    }
}

impl OpmlCodecPort for RecordingOpmlCodec {
    fn encode(&self, feeds: &[OpmlFeed]) -> anyhow::Result<String> {
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
    async fn upsert_subscriptions(
        &self,
        new_feeds: &[rssr_domain::NewFeedSubscription],
    ) -> rssr_domain::Result<Vec<rssr_domain::Feed>> {
        let mut feeds = Vec::with_capacity(new_feeds.len());
        for new_feed in new_feeds {
            feeds.push(self.upsert_subscription(new_feed).await?);
        }
        Ok(feeds)
    }

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
            settings: UserSettings::default(),
        }
        .summary_line(),
        "导入 2 个订阅，清理 1 个缺失订阅，设置已更新"
    );
    assert_eq!(
        super::ConfigImportOutcome {
            imported_feed_count: 0,
            removed_feed_count: 0,
            settings_updated: false,
            settings: UserSettings::default(),
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
    async fn upsert_subscriptions(
        &self,
        new_feeds: &[rssr_domain::NewFeedSubscription],
    ) -> rssr_domain::Result<Vec<rssr_domain::Feed>> {
        let mut feeds = Vec::with_capacity(new_feeds.len());
        for new_feed in new_feeds {
            feeds.push(self.upsert_subscription(new_feed).await?);
        }
        Ok(feeds)
    }

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
            source_title: new_feed.source_title.clone(),
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
    let (service, _) = service(
        Arc::new(StubFeedRepository {
            feeds: vec![
                Feed {
                    id: 1,
                    url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                    title: Some("Example".to_string()),
                    source_title: None,
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
                    source_title: None,
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
        Arc::new(StubSettingsRepository { settings: UserSettings::default() }),
        Arc::new(RecordingOpmlCodec::default()),
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
    let (service, replacement) = service(
        Arc::new(MemoryFeedRepository {
            feeds: Mutex::new(vec![Feed {
                id: 1,
                url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                title: Some("Example".to_string()),
                source_title: None,
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
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        Arc::new(RecordingOpmlCodec::default()),
    );
    let remote = StubRemoteConfigStore { payload: Mutex::new(None) };

    let pushed = service.push_remote_config(&remote).await.expect("push config");
    let fingerprint = service.config_fingerprint().await.expect("fingerprint");
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
    let plans = replacement.plans.lock().expect("lock plans");
    assert_eq!(plans.len(), 1, "the pulled configuration replaces through the atomic port");
    assert_eq!(plans[0].feeds[0].url.as_str(), "https://example.com/feed.xml");
    assert_eq!(plans[0].expected, fingerprint, "expects the configuration read before download");
    assert_eq!(pulled.import().map(|import| &import.settings), Some(&plans[0].settings));
}

#[tokio::test]
async fn pull_remote_config_reports_missing_remote_payload() {
    let (service, replacement) = service(
        Arc::new(MemoryFeedRepository { feeds: Mutex::new(Vec::new()) }),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        Arc::new(RecordingOpmlCodec::default()),
    );
    let remote = StubRemoteConfigStore { payload: Mutex::new(None) };

    let pulled = service.pull_remote_config(&remote).await.expect("pull missing config");
    assert!(replacement.plans.lock().expect("lock plans").is_empty());

    assert!(!pulled.found());
    assert_eq!(pulled.import(), None);
}

/// The whole package goes to the atomic port as one plan: parsed and normalized addresses, the
/// names and groups as given, the settings; the outcome is the port's.
#[tokio::test]
async fn import_config_hands_the_whole_package_to_the_atomic_port() {
    let (service, replacement) = service(
        Arc::new(MemoryFeedRepository { feeds: Mutex::new(Vec::new()) }),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        Arc::new(RecordingOpmlCodec::default()),
    );
    let mut imported_settings = UserSettings::default();
    imported_settings.refresh_interval_minutes += 1;
    let expected = ConfigFingerprint::compute(Vec::new(), &UserSettings::default());

    let outcome = service
        .import_config_package(
            &ConfigPackage {
                version: 2,
                exported_at: OffsetDateTime::UNIX_EPOCH,
                feeds: vec![
                    ConfigFeed {
                        url: "https://example.com:443/feed.xml#top".to_string(),
                        title: Some("Example".to_string()),
                        folder: Some("Tech".to_string()),
                    },
                    ConfigFeed {
                        url: "https://other.example.com/rss".to_string(),
                        title: None,
                        folder: None,
                    },
                ],
                settings: imported_settings.clone(),
            },
            &expected,
        )
        .await
        .expect("import package");

    assert_eq!(
        (outcome.imported_feed_count, outcome.removed_feed_count, outcome.settings_updated),
        (2, 1, true)
    );
    let plans = replacement.plans.lock().expect("lock plans");
    assert_eq!(plans.len(), 1);
    let feeds: Vec<_> = plans[0]
        .feeds
        .iter()
        .map(|feed| (feed.url.as_str(), feed.title.as_deref(), feed.folder.as_deref()))
        .collect();
    assert_eq!(
        feeds,
        [
            ("https://example.com/feed.xml", Some("Example"), Some("Tech")),
            ("https://other.example.com/rss", None, None),
        ]
    );
    assert_eq!(plans[0].settings, imported_settings);
    assert_eq!(plans[0].expected, expected);
    assert_eq!(outcome.settings, imported_settings, "the committed settings come back");
}

/// An invalid package is refused before the port is asked to change anything.
#[tokio::test]
async fn an_invalid_package_never_reaches_the_atomic_port() {
    let (service, replacement) = service(
        Arc::new(MemoryFeedRepository { feeds: Mutex::new(Vec::new()) }),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        Arc::new(RecordingOpmlCodec::default()),
    );
    let package = ConfigPackage {
        version: 2,
        exported_at: OffsetDateTime::UNIX_EPOCH,
        feeds: vec![ConfigFeed { url: "not a url".to_string(), title: None, folder: None }],
        settings: UserSettings::default(),
    };
    let expected = ConfigFingerprint::compute(Vec::new(), &UserSettings::default());
    assert!(service.import_config_package(&package, &expected).await.is_err());
    assert!(replacement.plans.lock().expect("lock plans").is_empty());
}

/// OPML 导出与配置包导出同一份订阅列表，另外带上源标题，分别写进 `text` 与 `title`（审计 B6）。
#[tokio::test]
async fn export_opml_passes_the_display_name_and_the_source_title() {
    let codec = Arc::new(RecordingOpmlCodec::default());
    let now = OffsetDateTime::UNIX_EPOCH;
    let (service, _) = service(
        Arc::new(StubFeedRepository {
            feeds: vec![Feed {
                id: 1,
                url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                title: Some("工作资讯".to_string()),
                source_title: Some("Example News".to_string()),
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
        Arc::new(StubSettingsRepository { settings: UserSettings::default() }),
        codec.clone(),
    );

    let raw = service.export_opml().await.expect("export opml");

    assert_eq!(raw, "<opml />");
    assert_eq!(
        codec.encoded_feeds.lock().expect("lock encoded feeds").as_slice(),
        [OpmlFeed {
            url: "https://example.com/feed.xml".to_string(),
            name: Some("工作资讯".to_string()),
            source_title: Some("Example News".to_string()),
            folder: Some("Tech".to_string()),
        }]
    );
}

#[tokio::test]
async fn import_opml_upserts_normalized_feeds() {
    let feed_repository = Arc::new(MemoryFeedRepository { feeds: Mutex::new(Vec::new()) });
    let (service, _) = service(
        feed_repository.clone(),
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

/// 只记录收到的订阅；`list_feeds` 像真实仓储一样不列出已删除的订阅。
#[derive(Default)]
struct RecordingFeedRepository {
    upserted: Mutex<Vec<NewFeedSubscription>>,
}

#[async_trait::async_trait]
impl FeedRepository for RecordingFeedRepository {
    async fn upsert_subscriptions(
        &self,
        new_feeds: &[rssr_domain::NewFeedSubscription],
    ) -> rssr_domain::Result<Vec<rssr_domain::Feed>> {
        let mut feeds = Vec::with_capacity(new_feeds.len());
        for new_feed in new_feeds {
            feeds.push(self.upsert_subscription(new_feed).await?);
        }
        Ok(feeds)
    }

    async fn upsert_subscription(
        &self,
        new_feed: &NewFeedSubscription,
    ) -> rssr_domain::Result<Feed> {
        self.upserted.lock().expect("lock upserted").push(new_feed.clone());
        Ok(Feed {
            id: 1,
            url: new_feed.url.clone(),
            title: None,
            source_title: None,
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
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        })
    }

    async fn set_deleted(&self, _feed_id: i64, _is_deleted: bool) -> rssr_domain::Result<()> {
        Ok(())
    }

    async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
        Ok(Vec::new())
    }

    async fn get_feed(&self, _feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
        Ok(None)
    }

    async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
        Ok(Vec::new())
    }
}

/// OPML 里没有名称和分组就清除，不看导入前的订阅快照：已删除的订阅不在快照里，以前会被当成
/// 新订阅而保留删除前的名称和分组（审计 B6）。
#[tokio::test]
async fn import_opml_clears_missing_names_and_folders_without_consulting_a_snapshot() {
    let feed_repository = Arc::new(RecordingFeedRepository::default());
    let (service, _) = service(
        feed_repository.clone(),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        Arc::new(RecordingOpmlCodec::with_decoded(vec![
            ConfigFeed {
                url: "https://example.com/unnamed.xml".to_string(),
                title: None,
                folder: None,
            },
            ConfigFeed {
                url: "https://example.com/named.xml".to_string(),
                title: Some("工作资讯".to_string()),
                folder: Some("Tech".to_string()),
            },
        ])),
    );

    service.import_opml("<opml />").await.expect("import opml");

    let upserted = feed_repository.upserted.lock().expect("lock upserted").clone();
    let fields: Vec<_> = upserted
        .iter()
        .map(|feed| (feed.title.as_deref(), feed.source_title.as_deref(), feed.folder.as_deref()))
        .collect();
    assert_eq!(fields, [(Some(""), None, Some("")), (Some("工作资讯"), None, Some("Tech"))]);
}
