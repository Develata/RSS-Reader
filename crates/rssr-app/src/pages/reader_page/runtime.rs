use crate::ui::{UiCommand, UiIntent, execute_ui_command};

use super::{effect::ReaderPageEffect, intent::ReaderPageIntent};

pub(crate) struct ReaderPageRuntimeOutcome {
    pub(crate) intents: Vec<ReaderPageIntent>,
}

pub(crate) async fn execute_reader_page_effect(
    effect: ReaderPageEffect,
) -> ReaderPageRuntimeOutcome {
    let command = match effect {
        ReaderPageEffect::LoadEntry(entry_id) => UiCommand::ReaderLoadEntry { entry_id },
        ReaderPageEffect::ToggleRead { entry_id, currently_read, via_shortcut } => {
            UiCommand::ReaderToggleRead { entry_id, currently_read, via_shortcut }
        }
        ReaderPageEffect::ToggleStarred { entry_id, currently_starred, via_shortcut } => {
            UiCommand::ReaderToggleStarred { entry_id, currently_starred, via_shortcut }
        }
    };

    let outcome = execute_ui_command(command).await;
    ReaderPageRuntimeOutcome {
        intents: outcome
            .intents
            .into_iter()
            .filter_map(|intent| match intent {
                UiIntent::ReaderPage(intent) => Some(intent),
                UiIntent::SetStatus { message, tone } => {
                    Some(ReaderPageIntent::SetStatus { message, tone })
                }
                UiIntent::AuthenticatedShellLoaded(_)
                | UiIntent::StartupRouteResolved(_)
                | UiIntent::EntriesPage(_)
                | UiIntent::FeedsPage(_)
                | UiIntent::SettingsPage(_) => None,
            })
            .collect(),
    }
}
