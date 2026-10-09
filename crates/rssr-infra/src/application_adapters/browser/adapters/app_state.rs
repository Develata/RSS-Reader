use super::shared::map_store_error;
use crate::application_adapters::browser::state::{BrowserStore, Changes};
use anyhow::Result;
use rssr_application::AppStatePort;
use rssr_domain::{AppStateMutation, AppStateRepository, AppStateSnapshot};

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
    /// 读改写在同一次 `store.update` 里完成，Web Lock 让并发更新逐个进行（审计 A2）。
    async fn update(&self, mutate: AppStateMutation) -> rssr_domain::Result<()> {
        self.store
            .update(move |state| {
                let before = state.app_state.clone();
                mutate(&mut state.app_state);
                let changes =
                    if state.app_state == before { Changes::NONE } else { Changes::APP_STATE };
                Ok(((), changes))
            })
            .await
            .map_err(map_store_error)
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
