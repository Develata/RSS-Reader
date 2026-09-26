use super::shared::map_store_error;
use crate::application_adapters::browser::state::{BrowserStore, Changes};
use rssr_domain::{SettingsRepository, UserSettings};

#[derive(Clone)]
pub struct BrowserSettingsRepository {
    store: BrowserStore,
}
impl BrowserSettingsRepository {
    pub fn new(store: BrowserStore) -> Self {
        Self { store }
    }
}
#[async_trait::async_trait]
impl SettingsRepository for BrowserSettingsRepository {
    async fn load(&self) -> rssr_domain::Result<UserSettings> {
        self.store.read(|state| Ok(state.core.settings.clone())).await.map_err(map_store_error)
    }
    async fn save(&self, settings: &UserSettings) -> rssr_domain::Result<()> {
        let settings = settings.clone();
        self.store
            .update(move |state| {
                state.core.settings = settings;
                Ok(((), Changes::CORE))
            })
            .await
            .map_err(map_store_error)
    }
}
