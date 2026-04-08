use std::sync::Arc;

use anyhow::Result;
use rssr_application::{AppStatePort, FeedRemovalCleanupPort, OpmlCodecPort, RemoteConfigStore};

use crate::{
    config_sync::webdav::WebDavConfigSync, db::app_state_repository::SqliteAppStateRepository,
    opml::OpmlCodec,
};

#[derive(Clone)]
pub struct SqliteAppStateAdapter {
    repository: Arc<SqliteAppStateRepository>,
}

impl SqliteAppStateAdapter {
    pub fn new(repository: Arc<SqliteAppStateRepository>) -> Self {
        Self { repository }
    }

    async fn clear_last_opened_feed_if_matches_impl(&self, feed_id: i64) -> Result<()> {
        if self.repository.load_last_opened_feed_id().await? == Some(feed_id) {
            self.repository.save_last_opened_feed_id(None).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AppStatePort for SqliteAppStateAdapter {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
        self.clear_last_opened_feed_if_matches_impl(feed_id).await
    }
}

#[async_trait::async_trait]
impl FeedRemovalCleanupPort for SqliteAppStateAdapter {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
        self.clear_last_opened_feed_if_matches_impl(feed_id).await
    }
}

#[derive(Clone, Default)]
pub struct InfraOpmlCodec {
    codec: OpmlCodec,
}

impl InfraOpmlCodec {
    pub fn new(codec: OpmlCodec) -> Self {
        Self { codec }
    }
}

impl OpmlCodecPort for InfraOpmlCodec {
    fn encode(&self, feeds: &[rssr_domain::ConfigFeed]) -> Result<String> {
        self.codec.encode(feeds)
    }

    fn decode(&self, raw: &str) -> Result<Vec<rssr_domain::ConfigFeed>> {
        self.codec.decode(raw)
    }
}

#[async_trait::async_trait]
impl RemoteConfigStore for WebDavConfigSync {
    async fn upload_config(&self, raw: &str) -> Result<()> {
        self.upload_text(raw).await
    }

    async fn download_config(&self) -> Result<Option<String>> {
        self.download_text().await
    }
}
