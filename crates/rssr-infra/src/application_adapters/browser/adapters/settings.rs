use super::shared::map_store_error;
use crate::application_adapters::browser::state::{BrowserStore, Changes};
use rssr_application::{SettingsPatch, SettingsUpdatePort};
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

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl SettingsUpdatePort for BrowserSettingsRepository {
    async fn update_settings(&self, patch: SettingsPatch) -> anyhow::Result<UserSettings> {
        self.store
            .update(move |state| {
                // Only a real change touches the cache: an unpublished edit would outlive the
                // write in memory.
                let next = patch.merge(&state.core.settings)?;
                if next == state.core.settings {
                    return Ok((next, Changes::NONE));
                }
                state.core.settings = next.clone();
                Ok((next, Changes::CORE))
            })
            .await
            .map_err(map_store_error)
            .map_err(Into::into)
    }
}
