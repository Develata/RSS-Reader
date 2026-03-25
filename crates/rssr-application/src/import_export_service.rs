use std::sync::Arc;

use anyhow::{Context, Result, ensure};
use rssr_domain::{
    ConfigFeed, ConfigPackage, FeedRepository, NewFeedSubscription, SettingsRepository,
};
use time::OffsetDateTime;
use url::Url;

pub struct ImportExportService {
    feed_repository: Arc<dyn FeedRepository>,
    settings_repository: Arc<dyn SettingsRepository>,
}

#[async_trait::async_trait]
pub trait RemoteConfigStore: Send + Sync {
    async fn upload_config(&self, raw: &str) -> Result<()>;
    async fn download_config(&self) -> Result<Option<String>>;
}

impl ImportExportService {
    pub fn new(
        feed_repository: Arc<dyn FeedRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
    ) -> Self {
        Self { feed_repository, settings_repository }
    }

    pub async fn export_config(&self) -> Result<ConfigPackage> {
        let feeds = self
            .feed_repository
            .list_feeds()
            .await?
            .into_iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| ConfigFeed {
                url: feed.url.to_string(),
                title: feed.title,
                folder: feed.folder,
            })
            .collect();

        let settings = self.settings_repository.load().await?;

        Ok(ConfigPackage { version: 1, exported_at: OffsetDateTime::now_utc(), feeds, settings })
    }

    pub async fn import_config_package(&self, package: &ConfigPackage) -> Result<()> {
        validate_config_package(package)?;

        let current_feeds = self.feed_repository.list_feeds().await?;
        let mut imported_urls = Vec::with_capacity(package.feeds.len());

        for feed in &package.feeds {
            let url =
                Url::parse(&feed.url).with_context(|| format!("无效的订阅 URL：{}", feed.url))?;
            imported_urls.push(url.clone());

            self.feed_repository
                .upsert_subscription(&NewFeedSubscription {
                    url,
                    title: feed.title.clone(),
                    folder: feed.folder.clone(),
                })
                .await?;
        }

        for feed in current_feeds {
            if !imported_urls.iter().any(|url| *url == feed.url) {
                self.feed_repository.set_deleted(feed.id, true).await?;
            }
        }

        self.settings_repository.save(&package.settings).await?;

        Ok(())
    }

    pub async fn export_config_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.export_config().await?)?)
    }

    pub async fn import_config_json(&self, raw: &str) -> Result<()> {
        let package: ConfigPackage = serde_json::from_str(raw)?;
        self.import_config_package(&package).await
    }

    pub async fn push_remote_config(&self, remote: &dyn RemoteConfigStore) -> Result<()> {
        remote.upload_config(&self.export_config_json().await?).await
    }

    pub async fn pull_remote_config(&self, remote: &dyn RemoteConfigStore) -> Result<bool> {
        match remote.download_config().await? {
            Some(raw) => {
                self.import_config_json(&raw).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

fn validate_config_package(package: &ConfigPackage) -> Result<()> {
    ensure!(package.version >= 1, "配置包版本必须大于等于 1");
    validate_settings(&package.settings)?;

    let mut seen_urls = std::collections::HashSet::new();
    for feed in &package.feeds {
        let mut normalized =
            Url::parse(&feed.url).with_context(|| format!("无效的订阅 URL：{}", feed.url))?;
        normalized.set_fragment(None);
        ensure!(
            seen_urls.insert(normalized.to_string()),
            "配置包中包含重复的 feed URL：{}",
            feed.url
        );
    }

    Ok(())
}

fn validate_settings(settings: &rssr_domain::UserSettings) -> Result<()> {
    ensure!(settings.refresh_interval_minutes >= 1, "刷新间隔必须大于等于 1 分钟");
    ensure!(
        (0.8..=1.5).contains(&settings.reader_font_scale),
        "阅读字号缩放必须在 0.8 到 1.5 之间"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{
        DomainError, FeedRepository, SettingsRepository, UserSettings,
        feed::{Feed, FeedSummary, NewFeedSubscription},
    };
    use time::OffsetDateTime;
    use url::Url;

    use super::{ImportExportService, RemoteConfigStore};

    struct StubFeedRepository {
        feeds: Vec<Feed>,
    }

    #[async_trait::async_trait]
    impl FeedRepository for StubFeedRepository {
        async fn upsert_subscription(
            &self,
            _new_feed: &NewFeedSubscription,
        ) -> rssr_domain::Result<Feed> {
            Err(DomainError::Persistence("not used in test".into()))
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

    struct StubRemoteConfigStore {
        payload: Mutex<Option<String>>,
    }

    #[async_trait::async_trait]
    impl RemoteConfigStore for StubRemoteConfigStore {
        async fn upload_config(&self, raw: &str) -> anyhow::Result<()> {
            *self.payload.lock().expect("lock payload") = Some(raw.to_string());
            Ok(())
        }

        async fn download_config(&self) -> anyhow::Result<Option<String>> {
            Ok(self.payload.lock().expect("lock payload").clone())
        }
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
                feed.title = new_feed.title.clone().or_else(|| feed.title.clone());
                feed.folder = new_feed.folder.clone().or_else(|| feed.folder.clone());
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
            let Some(feed) = feeds.iter_mut().find(|feed| feed.id == feed_id) else {
                return Err(DomainError::NotFound);
            };
            feed.is_deleted = is_deleted;
            Ok(())
        }

        async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
            Ok(self.feeds.lock().expect("lock feeds").clone())
        }

        async fn get_feed(&self, feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
            Ok(self
                .feeds
                .lock()
                .expect("lock feeds")
                .iter()
                .find(|feed| feed.id == feed_id)
                .cloned())
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
        let active_feed = Feed {
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
        };
        let deleted_feed = Feed {
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
        };

        let service = ImportExportService::new(
            Arc::new(StubFeedRepository { feeds: vec![active_feed, deleted_feed] }),
            Arc::new(StubSettingsRepository { settings: UserSettings::default() }),
        );

        let exported = service.export_config().await.expect("export config");

        assert_eq!(exported.version, 1);
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
            Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
        );
        let remote = StubRemoteConfigStore { payload: Mutex::new(None) };

        service.push_remote_config(&remote).await.expect("push config");
        let pulled = service.pull_remote_config(&remote).await.expect("pull config");

        assert!(pulled);
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
}
