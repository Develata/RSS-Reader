use crate::pages::settings_page::intent::SettingsPageIntent;
use crate::ui::{UiCommand, UiIntent, execute_ui_command};

use super::effect::SettingsPageSyncEffect;

pub(crate) struct SettingsPageSyncRuntimeOutcome {
    pub(crate) page_intents: Vec<SettingsPageIntent>,
}

pub(crate) async fn execute_settings_page_sync_effect(
    effect: SettingsPageSyncEffect,
) -> SettingsPageSyncRuntimeOutcome {
    SettingsPageSyncRuntimeOutcome {
        page_intents: execute_ui_command(match effect {
            SettingsPageSyncEffect::PushConfig { endpoint, remote_path } => {
                UiCommand::SettingsPushConfig { endpoint, remote_path }
            }
            SettingsPageSyncEffect::PullConfig { endpoint, remote_path } => {
                UiCommand::SettingsPullConfig { endpoint, remote_path }
            }
        })
        .await
        .intents
        .into_iter()
        .filter_map(|intent| match intent {
            UiIntent::SettingsPage(intent) => Some(intent),
            UiIntent::SetStatus { message, tone } => {
                Some(SettingsPageIntent::SetStatus { message, tone })
            }
            UiIntent::AuthenticatedShellLoaded(_)
            | UiIntent::StartupRouteResolved(_)
            | UiIntent::EntriesPage(_)
            | UiIntent::FeedsPage(_)
            | UiIntent::ReaderPage(_) => None,
        })
        .collect(),
    }
}
