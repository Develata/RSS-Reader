mod rules;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use anyhow::{Context, Result};
use rssr_domain::{
    ConfigFeed, ConfigPackage, EntryRepository, FeedRepository, NewFeedSubscription,
    SettingsRepository, normalize_feed_url,
};
use time::OffsetDateTime;
use url::Url;

use self::rules::{import_field, validate_config_package};

#[derive(Clone)]
pub struct ImportExportService {
    feed_repository: Arc<dyn FeedRepository>,
    entry_repository: Arc<dyn EntryRepository>,
    settings_repository: Arc<dyn SettingsRepository>,
    opml_codec: Arc<dyn OpmlCodecPort>,
    feed_removal_cleanup: Arc<dyn FeedRemovalCleanupPort>,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait RemoteConfigStore: Send + Sync {
    async fn upload_config(&self, raw: &str) -> Result<()>;
    async fn download_config(&self) -> Result<Option<String>>;
}

pub trait OpmlCodecPort: Send + Sync {
    fn encode(&self, feeds: &[ConfigFeed]) -> Result<String>;
    fn decode(&self, raw: &str) -> Result<Vec<ConfigFeed>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigImportOutcome {
    pub imported_feed_count: usize,
    pub removed_feed_count: usize,
    pub settings_updated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpmlImportOutcome {
    pub imported_feed_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteConfigPushOutcome {
    pub exported_feed_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteConfigPullOutcome {
    pub import: Option<ConfigImportOutcome>,
}

impl RemoteConfigPullOutcome {
    pub fn not_found() -> Self {
        Self { import: None }
    }

    pub fn imported(outcome: ConfigImportOutcome) -> Self {
        Self { import: Some(outcome) }
    }

    pub fn found(&self) -> bool {
        self.import.is_some()
    }

    pub fn import(&self) -> Option<&ConfigImportOutcome> {
        self.import.as_ref()
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait FeedRemovalCleanupPort: Send + Sync {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()>;
}

#[derive(Default)]
struct NoopFeedRemovalCleanup;

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl FeedRemovalCleanupPort for NoopFeedRemovalCleanup {
    async fn clear_last_opened_feed_if_matches(&self, _feed_id: i64) -> Result<()> {
        Ok(())
    }
}

impl ImportExportService {
    pub fn new(
        feed_repository: Arc<dyn FeedRepository>,
        entry_repository: Arc<dyn EntryRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
        opml_codec: Arc<dyn OpmlCodecPort>,
    ) -> Self {
        Self::new_with_feed_removal_cleanup(
            feed_repository,
            entry_repository,
            settings_repository,
            opml_codec,
            Arc::new(NoopFeedRemovalCleanup),
        )
    }

    pub fn new_with_feed_removal_cleanup(
        feed_repository: Arc<dyn FeedRepository>,
        entry_repository: Arc<dyn EntryRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
        opml_codec: Arc<dyn OpmlCodecPort>,
        feed_removal_cleanup: Arc<dyn FeedRemovalCleanupPort>,
    ) -> Self {
        Self {
            feed_repository,
            entry_repository,
            settings_repository,
            opml_codec,
            feed_removal_cleanup,
        }
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

        Ok(ConfigPackage { version: 2, exported_at: export_time_utc(), feeds, settings })
    }

    pub async fn import_config_package(
        &self,
        package: &ConfigPackage,
    ) -> Result<ConfigImportOutcome> {
        validate_config_package(package)?;

        let current_feeds = self.feed_repository.list_feeds().await?;
        let current_settings = self.settings_repository.load().await?;
        let mut imported_urls = Vec::with_capacity(package.feeds.len());

        for feed in &package.feeds {
            let url = normalize_feed_url(
                &Url::parse(&feed.url).with_context(|| format!("无效的订阅 URL：{}", feed.url))?,
            );
            let existed =
                current_feeds.iter().any(|current| normalize_feed_url(&current.url) == url);
            imported_urls.push(url.clone());

            self.feed_repository
                .upsert_subscription(&NewFeedSubscription {
                    url,
                    title: import_field(feed.title.clone(), existed),
                    folder: import_field(feed.folder.clone(), existed),
                })
                .await?;
        }

        let mut removed_feed_count = 0;
        for feed in current_feeds {
            if !imported_urls.iter().any(|url| *url == normalize_feed_url(&feed.url)) {
                self.remove_feed_with_cleanup(feed.id).await?;
                removed_feed_count += 1;
            }
        }

        let settings_updated = current_settings != package.settings;
        self.settings_repository.save(&package.settings).await?;

        Ok(ConfigImportOutcome {
            imported_feed_count: package.feeds.len(),
            removed_feed_count,
            settings_updated,
        })
    }

    pub async fn export_config_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.export_config().await?)?)
    }

    pub async fn import_config_json(&self, raw: &str) -> Result<ConfigImportOutcome> {
        let package: ConfigPackage = serde_json::from_str(raw)?;
        self.import_config_package(&package).await
    }

    pub async fn export_opml(&self) -> Result<String> {
        self.opml_codec.encode(&self.export_config().await?.feeds)
    }

    pub async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome> {
        let feeds = self.opml_codec.decode(raw)?;
        let current_feeds = self.feed_repository.list_feeds().await?;
        let imported_feed_count = feeds.len();

        for feed in feeds {
            let url = normalize_feed_url(
                &Url::parse(&feed.url)
                    .with_context(|| format!("OPML 中存在无效订阅 URL：{}", feed.url))?,
            );
            let existed =
                current_feeds.iter().any(|current| normalize_feed_url(&current.url) == url);
            self.feed_repository
                .upsert_subscription(&NewFeedSubscription {
                    url,
                    title: import_field(feed.title, existed),
                    folder: import_field(feed.folder, existed),
                })
                .await?;
        }

        Ok(OpmlImportOutcome { imported_feed_count })
    }

    pub async fn push_remote_config(
        &self,
        remote: &dyn RemoteConfigStore,
    ) -> Result<RemoteConfigPushOutcome> {
        let package = self.export_config().await?;
        let exported_feed_count = package.feeds.len();
        let raw = serde_json::to_string_pretty(&package)?;
        remote.upload_config(&raw).await?;
        Ok(RemoteConfigPushOutcome { exported_feed_count })
    }

    pub async fn pull_remote_config(
        &self,
        remote: &dyn RemoteConfigStore,
    ) -> Result<RemoteConfigPullOutcome> {
        match remote.download_config().await? {
            Some(raw) => {
                Ok(RemoteConfigPullOutcome::imported(self.import_config_json(&raw).await?))
            }
            None => Ok(RemoteConfigPullOutcome::not_found()),
        }
    }

    async fn remove_feed_with_cleanup(&self, feed_id: i64) -> Result<()> {
        self.entry_repository.delete_for_feed(feed_id).await?;
        self.feed_repository.set_deleted(feed_id, true).await?;
        self.feed_removal_cleanup.clear_last_opened_feed_if_matches(feed_id).await
    }
}

fn export_time_utc() -> OffsetDateTime {
    #[cfg(target_arch = "wasm32")]
    {
        let millis = js_sys::Date::now() as i128;
        return OffsetDateTime::from_unix_timestamp_nanos(millis * 1_000_000)
            .expect("browser timestamp should fit in OffsetDateTime");
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        OffsetDateTime::now_utc()
    }
}
