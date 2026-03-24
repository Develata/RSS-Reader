use std::sync::Arc;

use anyhow::Result;
use rssr_domain::{SettingsRepository, UserSettings};

pub struct SettingsService {
    repository: Arc<dyn SettingsRepository>,
}

impl SettingsService {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }

    pub async fn load(&self) -> Result<UserSettings> {
        Ok(self.repository.load().await?)
    }

    pub async fn save(&self, settings: &UserSettings) -> Result<()> {
        Ok(self.repository.save(settings).await?)
    }
}
