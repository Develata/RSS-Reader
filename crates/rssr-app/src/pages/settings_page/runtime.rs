use crate::ui::{UiCommand, UiIntent, execute_ui_command};

use super::{effect::SettingsPageEffect, intent::SettingsPageIntent};

pub(crate) struct SettingsPageRuntimeOutcome {
    pub(crate) intents: Vec<SettingsPageIntent>,
}

pub(crate) async fn execute_settings_page_effect(
    effect: SettingsPageEffect,
) -> SettingsPageRuntimeOutcome {
    SettingsPageRuntimeOutcome {
        intents: execute_ui_command(match effect {
            SettingsPageEffect::LoadSettings => UiCommand::SettingsLoad,
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
