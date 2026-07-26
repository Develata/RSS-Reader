use std::sync::Arc;

use anyhow::Result;
use rssr_domain::{SettingsRepository, UserSettings};

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

    /// 取值范围校验的唯一实现在 `rssr_domain::validation`，与配置包导入共用同一套规则。
    pub async fn save(&self, settings: &UserSettings) -> Result<()> {
        rssr_domain::validate_user_settings(settings)?;
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
        let settings = UserSettings { entries_page_size: 0, ..UserSettings::default() };

        let err = service().save(&settings).await.expect_err("reject zero page size");

        assert!(err.to_string().contains("文章页每页数量"));
    }

    #[tokio::test]
    async fn rejects_entries_page_size_above_upper_bound() {
        let settings = UserSettings { entries_page_size: 201, ..UserSettings::default() };

        let err = service().save(&settings).await.expect_err("reject large page size");

        assert!(err.to_string().contains("文章页每页数量"));
    }
}
