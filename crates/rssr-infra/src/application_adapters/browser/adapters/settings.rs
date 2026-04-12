use std::sync::{Arc, Mutex};

use rssr_domain::{SettingsRepository, UserSettings};

use crate::application_adapters::browser::state::{BrowserState, save_state_snapshot};

use super::shared::map_persistence_error;

#[derive(Clone)]
pub struct BrowserSettingsRepository {
    state: Arc<Mutex<BrowserState>>,
}

impl BrowserSettingsRepository {
    pub fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl SettingsRepository for BrowserSettingsRepository {
    async fn load(&self) -> rssr_domain::Result<UserSettings> {
        Ok(self.state.lock().expect("lock state").core.settings.clone())
    }

    async fn save(&self, settings: &UserSettings) -> rssr_domain::Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            state.core.settings = settings.clone();
            state.clone()
        };

        save_state_snapshot(snapshot).map_err(map_persistence_error)
    }
}
