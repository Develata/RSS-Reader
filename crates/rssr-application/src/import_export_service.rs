mod rules;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use anyhow::{Context, Result};
use rssr_domain::{
    ConfigFeed, ConfigPackage, FeedRepository, NewFeedSubscription, OpmlFeed, parse_feed_url,
};
use time::OffsetDateTime;

use self::rules::{import_field, validate_config_package};
use crate::library_queries::{ConfigSnapshot, LibraryQueryPort};
use crate::persistence_mutation::{
    ConfigReplacementFeed, ConfigReplacementPlan, ConfigReplacementPort,
};

#[derive(Clone)]
pub struct ImportExportService {
    feed_repository: Arc<dyn FeedRepository>,
    queries: Arc<dyn LibraryQueryPort>,
    opml_codec: Arc<dyn OpmlCodecPort>,
    clock: Arc<dyn ClockPort>,
    /// Replaces the subscriptions and settings in one transaction of the store.
    config_replacement: Arc<dyn ConfigReplacementPort>,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait RemoteConfigStore: Send + Sync {
    async fn upload_config(&self, raw: &str) -> Result<()>;
    async fn download_config(&self) -> Result<Option<String>>;
}

pub trait OpmlCodecPort: Send + Sync {
    /// 导出：每条订阅带显示名称与源标题，分别写进 OPML 的 `text` 与 `title`（审计 B6）。
    fn encode(&self, feeds: &[OpmlFeed]) -> Result<String>;
    /// 导入：每条订阅只有一个名称（`text` 优先），按指定名称导入。
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

impl ImportExportService {
    pub fn new(
        feed_repository: Arc<dyn FeedRepository>,
        queries: Arc<dyn LibraryQueryPort>,
        opml_codec: Arc<dyn OpmlCodecPort>,
        clock: Arc<dyn ClockPort>,
        config_replacement: Arc<dyn ConfigReplacementPort>,
    ) -> Self {
        Self { feed_repository, queries, opml_codec, clock, config_replacement }
    }

    /// 订阅与设置来自同一个读快照，导出的配置不会是两个时刻拼起来的。
    pub async fn export_config(&self) -> Result<ConfigPackage> {
        let ConfigSnapshot { feeds, settings } = self.queries.config_snapshot().await?;
        let feeds = feeds
            .into_iter()
            .map(|feed| ConfigFeed {
                url: feed.url.to_string(),
                title: feed.title,
                folder: feed.folder,
            })
            .collect();

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

        let outcome = self
            .config_replacement
            .replace_config(ConfigReplacementPlan {
                feeds: desired_feeds,
                settings: package.settings.clone(),
            })
            .await?;
        Ok(ConfigImportOutcome {
            imported_feed_count: package.feeds.len(),
            removed_feed_count: outcome.removed_feed_count,
            settings_updated: outcome.settings_updated,
        })
    }

    pub async fn export_config_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.export_config().await?)?)
    }

    pub async fn import_config_json(&self, raw: &str) -> Result<ConfigImportOutcome> {
        let package: ConfigPackage = serde_json::from_str(raw)?;
        self.import_config_package(&package).await
    }

    /// 与配置包导出同一份订阅列表，另外带上源标题：配置包格式只有一个名称，带不了它。
    pub async fn export_opml(&self) -> Result<String> {
        let feeds = self
            .feed_repository
            .list_feeds()
            .await?
            .into_iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| OpmlFeed {
                url: feed.url.to_string(),
                name: feed.title,
                source_title: feed.source_title,
                folder: feed.folder,
            })
            .collect::<Vec<_>>();
        self.opml_codec.encode(&feeds)
    }

    pub async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome> {
        let feeds = self.opml_codec.decode(raw)?;
        let imported_feed_count = feeds.len();
        let mut subscriptions = Vec::with_capacity(imported_feed_count);

        // Validate and normalize the complete document before the first write. A bad URL near the
        // end of an OPML file must not leave the valid prefix imported.
        for feed in feeds {
            let url = parse_feed_url(&feed.url)
                .with_context(|| format!("OPML 中存在无效订阅 URL：{}", feed.url))?;
            subscriptions.push(NewFeedSubscription {
                site_url: None,
                url,
                title: Some(import_field(feed.title)),
                source_title: None,
                folder: Some(import_field(feed.folder)),
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
}
