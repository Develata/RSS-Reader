use std::sync::Arc;

use anyhow::Result;
use rssr_application::{AppStatePort, OpmlCodecPort, RemoteConfigStore};
use rssr_domain::{AppStateMutation, AppStateRepository, AppStateSnapshot};

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
        self.repository
            .update_snapshot(|state| {
                if state.last_opened_feed_id == Some(feed_id) {
                    state.last_opened_feed_id = None;
                }
            })
            .await
            .map_err(Into::into)
    }

    pub async fn load_snapshot(&self) -> Result<AppStateSnapshot> {
        self.repository.load_snapshot().await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl AppStateRepository for SqliteAppStateAdapter {
    async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
        self.repository.load_snapshot().await
    }

    async fn update(&self, mutate: AppStateMutation) -> rssr_domain::Result<()> {
        self.repository.update_snapshot(mutate).await
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
    fn encode(&self, feeds: &[rssr_domain::OpmlFeed]) -> Result<String> {
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
