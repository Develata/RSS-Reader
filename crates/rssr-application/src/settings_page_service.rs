use rssr_domain::UserSettings;

use crate::{
    AppliedRemoteConfigOutcome, RemoteConfigPullOutcome, SettingsService, SettingsSyncService,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsPageSnapshot {
    pub settings: UserSettings,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SaveSettingsAppearanceOutcome {
    pub settings: UserSettings,
}

#[derive(Clone)]
pub struct SettingsPageService {
    settings_service: SettingsService,
    settings_sync_service: SettingsSyncService,
}

impl SettingsPageService {
    pub fn new(
        settings_service: SettingsService,
        settings_sync_service: SettingsSyncService,
    ) -> Self {
        Self { settings_service, settings_sync_service }
    }

    pub async fn load(&self) -> anyhow::Result<SettingsPageSnapshot> {
        Ok(SettingsPageSnapshot { settings: self.settings_service.load().await? })
    }

    pub async fn save_appearance(
        &self,
        settings: &UserSettings,
    ) -> anyhow::Result<SaveSettingsAppearanceOutcome> {
        self.settings_service.save(settings).await?;
        Ok(SaveSettingsAppearanceOutcome { settings: settings.clone() })
    }

    pub async fn apply_remote_pull(
        &self,
        outcome: RemoteConfigPullOutcome,
    ) -> anyhow::Result<AppliedRemoteConfigOutcome> {
        self.settings_sync_service.apply_remote_pull(outcome).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{SettingsRepository, UserSettings};

    use super::{SaveSettingsAppearanceOutcome, SettingsPageService, SettingsPageSnapshot};

    #[derive(Default)]
    struct SettingsRepositoryStub {
        settings: Mutex<UserSettings>,
    }

    #[async_trait::async_trait]
    impl SettingsRepository for SettingsRepositoryStub {
        async fn load(&self) -> rssr_domain::Result<UserSettings> {
            Ok(self.settings.lock().expect("lock settings").clone())
        }

        async fn save(&self, settings: &UserSettings) -> rssr_domain::Result<()> {
            *self.settings.lock().expect("lock settings") = settings.clone();
            Ok(())
        }
    }

    fn service(settings: UserSettings) -> SettingsPageService {
        let settings_service = crate::SettingsService::new(Arc::new(SettingsRepositoryStub {
            settings: Mutex::new(settings),
        }));
        let settings_sync_service = crate::SettingsSyncService::new(settings_service.clone());
        SettingsPageService::new(settings_service, settings_sync_service)
    }

    #[tokio::test]
    async fn load_returns_settings_snapshot() {
        let settings = UserSettings {
            refresh_interval_minutes: 15,
            custom_css: ".settings { gap: 8px; }".to_string(),
            ..UserSettings::default()
        };

        let snapshot = service(settings.clone()).load().await.expect("load settings page");

        assert_eq!(snapshot, SettingsPageSnapshot { settings });
    }

    #[tokio::test]
    async fn save_appearance_persists_and_returns_saved_settings() {
        let next = UserSettings {
            refresh_interval_minutes: 20,
            custom_css: ".reader { line-height: 1.7; }".to_string(),
            ..UserSettings::default()
        };

        let outcome = service(UserSettings::default())
            .save_appearance(&next)
            .await
            .expect("save settings appearance");

        assert_eq!(outcome, SaveSettingsAppearanceOutcome { settings: next });
    }
}
