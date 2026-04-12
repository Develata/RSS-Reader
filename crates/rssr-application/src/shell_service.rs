use rssr_domain::UserSettings;

use crate::SettingsService;

#[derive(Debug, Clone, PartialEq)]
pub struct AuthenticatedShellSnapshot {
    pub settings: UserSettings,
}

#[derive(Clone)]
pub struct ShellService {
    settings_service: SettingsService,
}

impl ShellService {
    pub fn new(settings_service: SettingsService) -> Self {
        Self { settings_service }
    }

    pub async fn load_authenticated_shell(&self) -> anyhow::Result<AuthenticatedShellSnapshot> {
        Ok(AuthenticatedShellSnapshot { settings: self.settings_service.load().await? })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{SettingsRepository, UserSettings};

    use super::ShellService;

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

    #[tokio::test]
    async fn load_authenticated_shell_returns_current_settings() {
        let settings = UserSettings {
            refresh_interval_minutes: 20,
            custom_css: ".shell { gap: 12px; }".to_string(),
            ..UserSettings::default()
        };
        let service =
            ShellService::new(crate::SettingsService::new(Arc::new(SettingsRepositoryStub {
                settings: Mutex::new(settings.clone()),
            })));

        let snapshot = service.load_authenticated_shell().await.expect("load shell snapshot");

        assert_eq!(snapshot.settings, settings);
    }
}
