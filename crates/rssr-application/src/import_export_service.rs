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

pub struct ImportExportService {
    feed_repository: Arc<dyn FeedRepository>,
    entry_repository: Arc<dyn EntryRepository>,
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
        entry_repository: Arc<dyn EntryRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
    ) -> Self {
        Self { feed_repository, entry_repository, settings_repository }
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

        for feed in current_feeds {
            if !imported_urls.iter().any(|url| *url == normalize_feed_url(&feed.url)) {
                self.entry_repository.delete_for_feed(feed.id).await?;
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
