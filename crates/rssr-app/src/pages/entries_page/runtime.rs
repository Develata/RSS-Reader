use super::{effect::EntriesPageEffect, intent::EntriesPageIntent};
use crate::ui::{UiCommand, UiIntent, execute_ui_command};

pub(crate) struct EntriesPageRuntimeOutcome {
    pub(crate) intents: Vec<EntriesPageIntent>,
}

pub(crate) async fn execute_entries_page_effect(
    effect: EntriesPageEffect,
) -> EntriesPageRuntimeOutcome {
    let command = match effect {
        EntriesPageEffect::Bootstrap { feed_id, load_preferences, load_feeds } => {
            UiCommand::EntriesBootstrap { feed_id, load_preferences, load_feeds }
        }
        EntriesPageEffect::LoadEntries(query) => UiCommand::EntriesLoadEntries { query },
        EntriesPageEffect::ToggleRead { entry_id, entry_title, currently_read } => {
            UiCommand::EntriesToggleRead { entry_id, entry_title, currently_read }
        }
        EntriesPageEffect::ToggleStarred { entry_id, entry_title, currently_starred } => {
            UiCommand::EntriesToggleStarred { entry_id, entry_title, currently_starred }
        }
        EntriesPageEffect::SaveBrowsingPreferences {
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        } => UiCommand::EntriesSaveBrowsingPreferences {
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        },
    };

    let outcome = execute_ui_command(command).await;
    EntriesPageRuntimeOutcome {
        intents: outcome
            .intents
            .into_iter()
            .filter_map(|intent| match intent {
                UiIntent::EntriesPage(intent) => Some(intent),
                UiIntent::SetStatus { message, tone } => {
                    Some(EntriesPageIntent::SetStatus { message, tone })
                }
                UiIntent::AuthenticatedShellLoaded(_)
                | UiIntent::StartupRouteResolved(_)
                | UiIntent::FeedsPage(_)
                | UiIntent::ReaderPage(_)
                | UiIntent::SettingsPage(_) => None,
            })
            .collect(),
    }
}
