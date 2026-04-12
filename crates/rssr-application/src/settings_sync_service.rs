use rssr_domain::UserSettings;

use crate::{ConfigImportOutcome, RemoteConfigPullOutcome, SettingsService};

#[derive(Debug, Clone, PartialEq)]
pub enum AppliedRemoteConfigOutcome {
    NotFound,
    Imported { import: ConfigImportOutcome, settings: UserSettings },
}

#[derive(Clone)]
pub struct SettingsSyncService {
    settings_service: SettingsService,
}

impl SettingsSyncService {
    pub fn new(settings_service: SettingsService) -> Self {
        Self { settings_service }
    }

    pub async fn apply_remote_pull(
        &self,
        outcome: RemoteConfigPullOutcome,
    ) -> anyhow::Result<AppliedRemoteConfigOutcome> {
        match outcome.import().cloned() {
            Some(import) => Ok(AppliedRemoteConfigOutcome::Imported {
                import,
                settings: self.settings_service.load().await?,
            }),
            None => Ok(AppliedRemoteConfigOutcome::NotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{SettingsRepository, UserSettings};

    use super::{AppliedRemoteConfigOutcome, SettingsSyncService};

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

    fn service(settings: UserSettings) -> SettingsSyncService {
        SettingsSyncService::new(crate::SettingsService::new(Arc::new(SettingsRepositoryStub {
            settings: Mutex::new(settings),
        })))
    }

    #[tokio::test]
    async fn apply_remote_pull_reloads_settings_when_imported() {
        let settings = UserSettings {
            refresh_interval_minutes: 10,
            custom_css: ".reader { max-width: 70ch; }".to_string(),
            ..UserSettings::default()
        };
        let outcome = service(settings.clone())
            .apply_remote_pull(crate::RemoteConfigPullOutcome::imported(
                crate::ConfigImportOutcome {
                    imported_feed_count: 2,
                    removed_feed_count: 1,
                    settings_updated: true,
                },
            ))
            .await
            .expect("apply remote pull");

        assert_eq!(
            outcome,
            AppliedRemoteConfigOutcome::Imported {
                import: crate::ConfigImportOutcome {
                    imported_feed_count: 2,
                    removed_feed_count: 1,
                    settings_updated: true,
                },
                settings,
            }
        );
    }

    #[tokio::test]
    async fn apply_remote_pull_keeps_not_found_without_reloading() {
        let outcome = service(UserSettings::default())
            .apply_remote_pull(crate::RemoteConfigPullOutcome::not_found())
            .await
            .expect("apply missing remote pull");

        assert_eq!(outcome, AppliedRemoteConfigOutcome::NotFound);
    }
}
