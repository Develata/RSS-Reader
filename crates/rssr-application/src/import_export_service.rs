mod rules;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use anyhow::{Context, Result};
use rssr_domain::{
    ConfigFeed, ConfigPackage, EntryContentRepository, EntryIndexRepository, FeedRepository,
    NewFeedSubscription, SettingsRepository, normalize_feed_url, parse_feed_url,
};
use time::OffsetDateTime;

use self::rules::{import_field, validate_config_package};
use crate::{
    persistence_mutation::{ConfigReplacementFeed, ConfigReplacementPlan, ConfigReplacementPort},
    subscription_workflow::AppStatePort,
};

#[derive(Clone)]
pub struct ImportExportService {
    feed_repository: Arc<dyn FeedRepository>,
    entry_index_repository: Arc<dyn EntryIndexRepository>,
    entry_content_repository: Arc<dyn EntryContentRepository>,
    settings_repository: Arc<dyn SettingsRepository>,
    opml_codec: Arc<dyn OpmlCodecPort>,
    app_state_cleanup: Arc<dyn AppStatePort>,
    clock: Arc<dyn ClockPort>,
    config_replacement_port: Option<Arc<dyn ConfigReplacementPort>>,
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

pub trait ClockPort: Send + Sync {
    fn now_utc(&self) -> OffsetDateTime;
}

#[derive(Default)]
pub struct SystemClock;

impl ClockPort for SystemClock {
    fn now_utc(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigImportOutcome {
    pub imported_feed_count: usize,
    pub removed_feed_count: usize,
    pub settings_updated: bool,
}

impl ConfigImportOutcome {
    pub fn summary_line(&self) -> String {
        let settings = if self.settings_updated { "设置已更新" } else { "设置未变化" };
        format!(
            "导入 {} 个订阅，清理 {} 个缺失订阅，{settings}",
            self.imported_feed_count, self.removed_feed_count
        )
    }
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

#[derive(Default)]
struct NoopAppStateCleanup;

#[async_trait::async_trait]
impl AppStatePort for NoopAppStateCleanup {
    async fn clear_last_opened_feed_if_matches(&self, _feed_id: i64) -> Result<()> {
        Ok(())
    }
}

impl ImportExportService {
    pub fn new(
        feed_repository: Arc<dyn FeedRepository>,
        entry_index_repository: Arc<dyn EntryIndexRepository>,
        entry_content_repository: Arc<dyn EntryContentRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
        opml_codec: Arc<dyn OpmlCodecPort>,
    ) -> Self {
        Self::new_with_app_state_cleanup(
            feed_repository,
            entry_index_repository,
            entry_content_repository,
            settings_repository,
            opml_codec,
            Arc::new(NoopAppStateCleanup),
        )
    }

    pub fn new_with_app_state_cleanup(
        feed_repository: Arc<dyn FeedRepository>,
        entry_index_repository: Arc<dyn EntryIndexRepository>,
        entry_content_repository: Arc<dyn EntryContentRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
        opml_codec: Arc<dyn OpmlCodecPort>,
        app_state_cleanup: Arc<dyn AppStatePort>,
    ) -> Self {
        Self::new_with_app_state_cleanup_and_clock(
            feed_repository,
            entry_index_repository,
            entry_content_repository,
            settings_repository,
            opml_codec,
            app_state_cleanup,
            Arc::new(SystemClock),
        )
    }

    pub fn new_with_app_state_cleanup_and_clock(
        feed_repository: Arc<dyn FeedRepository>,
        entry_index_repository: Arc<dyn EntryIndexRepository>,
        entry_content_repository: Arc<dyn EntryContentRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
        opml_codec: Arc<dyn OpmlCodecPort>,
        app_state_cleanup: Arc<dyn AppStatePort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            feed_repository,
            entry_index_repository,
            entry_content_repository,
            settings_repository,
            opml_codec,
            app_state_cleanup,
            clock,
            config_replacement_port: None,
        }
    }

    pub fn with_config_replacement_port(
        mut self,
        config_replacement_port: Arc<dyn ConfigReplacementPort>,
    ) -> Self {
        self.config_replacement_port = Some(config_replacement_port);
        self
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

        Ok(ConfigPackage {
            version: rssr_domain::CONFIG_PACKAGE_VERSION,
            exported_at: self.clock.now_utc(),
            feeds,
            settings,
        })
    }

    pub async fn import_config_package(
        &self,
        package: &ConfigPackage,
    ) -> Result<ConfigImportOutcome> {
        validate_config_package(package)?;

        // Parse the complete desired state before any write. Production mutation ports derive the
        // current-vs-desired diff only after acquiring their backend transaction/Web Lock.
        let desired_feeds = package
            .feeds
            .iter()
            .map(|feed| {
                Ok(ConfigReplacementFeed {
                    url: parse_feed_url(&feed.url)
                        .with_context(|| format!("无效的订阅 URL：{}", feed.url))?,
                    title: feed.title.clone(),
                    folder: feed.folder.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        if let Some(port) = &self.config_replacement_port {
            let outcome = port
                .replace_config(ConfigReplacementPlan {
                    feeds: desired_feeds,
                    settings: package.settings.clone(),
                })
                .await?;
            return Ok(ConfigImportOutcome {
                imported_feed_count: package.feeds.len(),
                removed_feed_count: outcome.removed_feed_count,
                settings_updated: outcome.settings_updated,
            });
        }

        // Legacy/test fallback for hand-built services that do not inject a backend-native
        // replacement port. Shipping native/Web compositions always use the atomic path above.
        let current_feeds = self.feed_repository.list_feeds().await?;
        let current_settings = self.settings_repository.load().await?;
        let imported_urls = desired_feeds.iter().map(|feed| feed.url.clone()).collect::<Vec<_>>();

        for feed in desired_feeds {
            let existed =
                current_feeds.iter().any(|current| normalize_feed_url(&current.url) == feed.url);
            self.feed_repository
                .upsert_subscription(&NewFeedSubscription {
                    site_url: None,
                    url: feed.url,
                    title: import_field(feed.title, existed),
                    folder: import_field(feed.folder, existed),
                })
                .await?;
        }

        let removed_feed_ids = current_feeds
            .iter()
            .filter(|feed| !imported_urls.iter().any(|url| *url == normalize_feed_url(&feed.url)))
            .map(|feed| feed.id)
            .collect::<Vec<_>>();
        let removed_feed_count = removed_feed_ids.len();
        for feed_id in removed_feed_ids {
            self.remove_feed_with_cleanup(feed_id).await?;
        }
        self.settings_repository.save(&package.settings).await?;

        Ok(ConfigImportOutcome {
            imported_feed_count: package.feeds.len(),
            removed_feed_count,
            settings_updated: current_settings != package.settings,
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
        let mut subscriptions = Vec::with_capacity(imported_feed_count);

        // Validate and normalize the complete document before the first write. A bad URL near the
        // end of an OPML file must not leave the valid prefix imported.
        for feed in feeds {
            let url = parse_feed_url(&feed.url)
                .with_context(|| format!("OPML 中存在无效订阅 URL：{}", feed.url))?;
            let existed =
                current_feeds.iter().any(|current| normalize_feed_url(&current.url) == url);
            subscriptions.push(NewFeedSubscription {
                site_url: None,
                url,
                title: import_field(feed.title, existed),
                folder: import_field(feed.folder, existed),
            });
        }

        self.feed_repository.upsert_subscriptions(&subscriptions).await?;
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
        // Keep the fallback failure mode consistent with direct removal: once cleanup begins,
        // the feed is hidden first, so a later cleanup error cannot expose an empty subscription.
        self.feed_repository.set_deleted(feed_id, true).await?;
        self.entry_index_repository.delete_for_feed(feed_id).await?;
        self.entry_content_repository.delete_for_feed(feed_id).await?;
        self.app_state_cleanup.clear_last_opened_feed_if_matches(feed_id).await
    }
}
