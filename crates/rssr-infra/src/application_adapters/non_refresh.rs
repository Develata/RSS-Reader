use std::sync::Arc;

use anyhow::Result;
use rssr_application::{AppStatePort, OpmlCodecPort, RemoteConfigStore};
use rssr_domain::{AppStateRepository, AppStateSnapshot};

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
        let mut state = self.repository.load_snapshot().await?;
        if state.last_opened_feed_id == Some(feed_id) {
            state.last_opened_feed_id = None;
            self.repository.save_snapshot(&state).await?;
        }
        Ok(())
    }

    pub async fn load_snapshot(&self) -> Result<AppStateSnapshot> {
        self.repository.load_snapshot().await.map_err(Into::into)
    }

    pub async fn save_snapshot(&self, state: &AppStateSnapshot) -> Result<()> {
        self.repository.save_snapshot(state).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl AppStateRepository for SqliteAppStateAdapter {
    async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
        self.repository.load_snapshot().await
    }

    async fn save(&self, state: &AppStateSnapshot) -> rssr_domain::Result<()> {
        self.repository.save_snapshot(state).await
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl AppStatePort for SqliteAppStateAdapter {
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

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigStore for WebDavConfigSync {
    async fn upload_config(&self, raw: &str) -> Result<()> {
        self.upload_text(raw).await
    }

    async fn download_config(&self) -> Result<Option<String>> {
        self.download_text().await
    }
}
