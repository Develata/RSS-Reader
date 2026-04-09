use rssr_domain::UserSettings;

use crate::{
    pages::settings_page::intent::SettingsPageIntent,
    ui::{UiCommand, UiIntent, execute_ui_command},
};

use super::effect::SettingsPageSaveEffect;

pub(crate) struct SettingsPageSaveRuntimeOutcome {
    pub(crate) status_message: String,
    pub(crate) saved_settings: Option<UserSettings>,
}

pub(crate) async fn execute_settings_page_save_effect(
    effect: SettingsPageSaveEffect,
) -> SettingsPageSaveRuntimeOutcome {
    let command = match effect {
        SettingsPageSaveEffect::SaveAppearance { settings, success_message } => {
            UiCommand::SettingsSaveAppearance { settings, success_message }
        }
    };

    let mut status_message = String::new();
    let mut saved_settings = None;

    for intent in execute_ui_command(command).await.intents {
        match intent {
            UiIntent::SettingsPage(SettingsPageIntent::SettingsLoaded(settings)) => {
                saved_settings = Some(settings);
            }
            UiIntent::SettingsPage(SettingsPageIntent::SetStatus { message, .. })
            | UiIntent::SetStatus { message, .. } => {
                status_message = message;
            }
            UiIntent::AuthenticatedShellLoaded(_)
            | UiIntent::StartupRouteResolved(_)
            | UiIntent::EntriesPage(_)
            | UiIntent::FeedsPage(_)
            | UiIntent::ReaderPage(_) => {}
        }
    }

    SettingsPageSaveRuntimeOutcome { status_message, saved_settings }
}
