use std::sync::Arc;

use anyhow::{Result, ensure};
use rssr_domain::{MAX_ENTRIES_PAGE_SIZE, SettingsRepository, UserSettings};

#[derive(Clone)]
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
            (1..=MAX_ENTRIES_PAGE_SIZE).contains(&settings.entries_page_size),
            "文章页每页数量必须在 1 到 {MAX_ENTRIES_PAGE_SIZE} 之间"
        );
        ensure!(
            (0.8..=1.5).contains(&settings.reader_font_scale),
            "阅读字号缩放必须在 0.8 到 1.5 之间"
        );
        Ok(self.repository.save(settings).await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{SettingsRepository, UserSettings};

    use super::SettingsService;

    struct SettingsRepositoryStub;

    #[async_trait::async_trait]
    impl SettingsRepository for SettingsRepositoryStub {
        async fn load(&self) -> rssr_domain::Result<UserSettings> {
            Ok(UserSettings::default())
        }

        async fn save(&self, _settings: &UserSettings) -> rssr_domain::Result<()> {
            Ok(())
        }
    }

    fn service() -> SettingsService {
        SettingsService::new(Arc::new(SettingsRepositoryStub))
    }

    #[tokio::test]
    async fn rejects_zero_entries_page_size() {
        let mut settings = UserSettings::default();
        settings.entries_page_size = 0;

        let err = service().save(&settings).await.expect_err("reject zero page size");

        assert!(err.to_string().contains("文章页每页数量"));
    }

    #[tokio::test]
    async fn rejects_entries_page_size_above_upper_bound() {
        let mut settings = UserSettings::default();
        settings.entries_page_size = 201;

        let err = service().save(&settings).await.expect_err("reject large page size");

        assert!(err.to_string().contains("文章页每页数量"));
    }
}
