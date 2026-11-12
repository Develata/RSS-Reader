mod remote_sync;
mod rules;
#[cfg(test)]
mod tests;

pub use remote_sync::{
    RemoteContentHash, RemoteEndpoint, RemoteSyncBase, RemoteSyncOutcome, RemoteSyncProfile,
    RemoteSyncProfileStore, RemoteSyncProfileView, RemoteSyncReport, RemoteSyncStep,
    RemoteSyncTrigger, RemoteSyncView, RemoteTarget, SavedRemote, display_endpoint,
};

use std::sync::Arc;

use anyhow::{Context, Result};
use rssr_domain::{
    ConfigFeed, ConfigFingerprint, ConfigPackage, DomainError, FeedRepository, NewFeedSubscription,
    OpmlFeed, UserSettings, parse_feed_url,
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

    /// The document and the validator (ETag) its server gave it; a store without validators
    /// gives none.
    async fn download_document(&self) -> Result<Option<RemoteConfigDocument>> {
        Ok(self.download_config().await?.map(|raw| RemoteConfigDocument { raw, etag: None }))
    }

    /// Uploads `raw` only while the remote still matches `precondition`. A store that cannot
    /// check a precondition uploads unconditionally.
    async fn upload_config_if(
        &self,
        raw: &str,
        precondition: &UploadPrecondition,
    ) -> Result<ConditionalUpload> {
        let _ = precondition;
        self.upload_config(raw).await?;
        Ok(ConditionalUpload::Stored)
    }
}

/// A configuration document as the remote holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteConfigDocument {
    pub raw: String,
    /// The server's entity tag, when it sent one.
    pub etag: Option<String>,
}

/// What the remote must still hold for a conditional upload to replace it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadPrecondition {
    /// Upload whatever the remote holds.
    None,
    /// Only while there is no document (`If-None-Match: *`).
    Absent,
    /// Only while the document still has this entity tag (`If-Match`).
    Matches(String),
}

/// The result of a conditional upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalUpload {
    Stored,
    /// The remote changed since it was read; nothing was uploaded.
    PreconditionFailed,
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigImportOutcome {
    pub imported_feed_count: u64,
    pub removed_feed_count: u64,
    pub settings_updated: bool,
    /// The settings now stored (committed by the replacement; no second read).
    pub settings: UserSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OpmlImportOutcome {
    pub imported_feed_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RemoteConfigPushOutcome {
    pub exported_feed_count: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
        Ok(self.export_config_with_fingerprint().await?.0)
    }

    /// [`Self::export_config`] with the fingerprint of exactly the exported configuration (the
    /// same read snapshot): what a synchronization keeps as its base after uploading it.
    pub(crate) async fn export_config_with_fingerprint(
        &self,
    ) -> Result<(ConfigPackage, ConfigFingerprint)> {
        let ConfigSnapshot { feeds, settings, fingerprint } =
            self.queries.config_snapshot().await?;
        let feeds = feeds
            .into_iter()
            .map(|feed| ConfigFeed {
                url: feed.url.to_string(),
                title: feed.title,
                folder: feed.folder,
            })
            .collect();

        let package = ConfigPackage {
            version: rssr_domain::CONFIG_PACKAGE_VERSION,
            exported_at: self.clock.now_utc(),
            feeds,
            settings,
        };
        Ok((package, fingerprint))
    }

    /// The fingerprint of the configuration as stored now; a replacement expects it.
    pub async fn config_fingerprint(&self) -> Result<ConfigFingerprint> {
        Ok(self.queries.config_fingerprint().await?)
    }

    /// Replaces the configuration with `package`, unless it changed since the caller read
    /// `expected` (then [`rssr_domain::DomainError::Conflict`], nothing changed).
    pub async fn import_config_package(
        &self,
        package: &ConfigPackage,
        expected: &ConfigFingerprint,
    ) -> Result<ConfigImportOutcome> {
        Ok(self.replace_with_package(package, expected).await?.0)
    }

    /// [`Self::import_config_package`], also giving the configuration's fingerprint after the
    /// replacement (read in its transaction).
    async fn replace_with_package(
        &self,
        package: &ConfigPackage,
        expected: &ConfigFingerprint,
    ) -> Result<(ConfigImportOutcome, ConfigFingerprint)> {
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
                expected: expected.clone(),
            })
            .await?;
        let imported = ConfigImportOutcome {
            imported_feed_count: package.feeds.len() as u64,
            removed_feed_count: outcome.removed_feed_count as u64,
            settings_updated: outcome.settings_updated,
            settings: outcome.settings,
        };
        Ok((imported, outcome.fingerprint))
    }

    pub async fn export_config_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.export_config().await?)?)
    }

    pub async fn import_config_json(
        &self,
        raw: &str,
        expected: &ConfigFingerprint,
    ) -> Result<ConfigImportOutcome> {
        let package: ConfigPackage = serde_json::from_str(raw).map_err(|error| {
            DomainError::InvalidInput(format!("配置包不是有效的 JSON：{error}"))
        })?;
        self.import_config_package(&package, expected).await
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
        let feeds = self
            .opml_codec
            .decode(raw)
            .map_err(|error| DomainError::InvalidInput(format!("OPML 无法解析：{error}")))?;
        let imported_feed_count = feeds.len() as u64;
        let mut subscriptions = Vec::with_capacity(feeds.len());

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
        let exported_feed_count = package.feeds.len() as u64;
        let raw = serde_json::to_string_pretty(&package)?;
        remote.upload_config(&raw).await?;
        Ok(RemoteConfigPushOutcome { exported_feed_count })
    }

    /// Downloads the configuration and replaces the local one with it. The expected
    /// fingerprint is read before the download, so a local change made while downloading is
    /// not overwritten.
    pub async fn pull_remote_config(
        &self,
        remote: &dyn RemoteConfigStore,
    ) -> Result<RemoteConfigPullOutcome> {
        let expected = self.config_fingerprint().await?;
        match remote.download_config().await? {
            Some(raw) => Ok(RemoteConfigPullOutcome::imported(
                self.import_config_json(&raw, &expected).await?,
            )),
            None => Ok(RemoteConfigPullOutcome::not_found()),
        }
    }
}
