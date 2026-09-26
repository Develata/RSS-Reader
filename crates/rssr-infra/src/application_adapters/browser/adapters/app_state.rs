use super::shared::map_store_error;
use crate::application_adapters::browser::state::{BrowserStore, Changes};
use anyhow::Result;
use rssr_application::AppStatePort;
use rssr_domain::{AppStateRepository, AppStateSnapshot};

#[derive(Clone)]
pub struct BrowserAppStateAdapter {
    store: BrowserStore,
}
impl BrowserAppStateAdapter {
    pub fn new(store: BrowserStore) -> Self {
        Self { store }
    }
    pub async fn load_snapshot(&self) -> Result<AppStateSnapshot> {
        self.store.read(|state| Ok(state.app_state.clone())).await
    }
    pub async fn save_snapshot(&self, app_state: &AppStateSnapshot) -> Result<()> {
        let app_state = app_state.clone();
        self.store
            .update(move |state| {
                state.app_state = app_state;
                Ok(((), Changes::APP_STATE))
            })
            .await
    }
}
#[async_trait::async_trait]
impl AppStateRepository for BrowserAppStateAdapter {
    async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
        self.load_snapshot().await.map_err(map_store_error)
    }
    async fn save(&self, state: &AppStateSnapshot) -> rssr_domain::Result<()> {
        self.save_snapshot(state).await.map_err(map_store_error)
    }
}
#[async_trait::async_trait]
impl AppStatePort for BrowserAppStateAdapter {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
        self.store
            .update(move |state| {
                if state.app_state.last_opened_feed_id != Some(feed_id) {
                    return Ok(((), Changes::NONE));
                }
                state.app_state.last_opened_feed_id = None;
                Ok(((), Changes::APP_STATE))
            })
            .await
    }
}
