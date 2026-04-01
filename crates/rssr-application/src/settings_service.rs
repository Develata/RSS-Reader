use std::sync::Arc;

use anyhow::{Result, ensure};
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
        ensure!(settings.refresh_interval_minutes >= 1, "刷新间隔必须大于等于 1 分钟");
        ensure!(settings.archive_after_months >= 1, "自动归档阈值必须大于等于 1 个月");
        ensure!(
            (0.8..=1.5).contains(&settings.reader_font_scale),
            "阅读字号缩放必须在 0.8 到 1.5 之间"
        );
        Ok(self.repository.save(settings).await?)
    }
}
