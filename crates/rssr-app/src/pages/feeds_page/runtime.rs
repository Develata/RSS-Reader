use dioxus::prelude::*;

use super::{commands::FeedsPageCommand, effect::FeedsPageEffect, intent::FeedsPageIntent};
use crate::ui::{UiCommand, UiIntent, execute_ui_command};

pub(crate) struct FeedsPageRuntimeOutcome {
    pub(crate) intents: Vec<FeedsPageIntent>,
}

impl FeedsPageRuntimeOutcome {
    fn single(intent: FeedsPageIntent) -> Self {
        Self { intents: vec![intent] }
    }
}

pub(crate) async fn execute_feeds_page_effect(effect: FeedsPageEffect) -> FeedsPageRuntimeOutcome {
    match effect {
        FeedsPageEffect::LoadSnapshot => {
            execute_feeds_page_ui_command(UiCommand::FeedsLoadSnapshot).await
        }
        FeedsPageEffect::ExecuteCommand(command) => {
            execute_feeds_page_ui_command(map_feeds_page_command(command)).await
        }
        FeedsPageEffect::ReadFeedUrlFromClipboard => FeedsPageRuntimeOutcome::single(
            FeedsPageIntent::ClipboardReadCompleted(read_feed_url_from_clipboard().await),
        ),
    }
}

async fn execute_feeds_page_ui_command(command: UiCommand) -> FeedsPageRuntimeOutcome {
    let outcome = execute_ui_command(command).await;
    FeedsPageRuntimeOutcome {
        intents: outcome
            .intents
            .into_iter()
            .filter_map(|intent| match intent {
                UiIntent::FeedsPage(intent) => Some(intent),
                UiIntent::SetStatus { message, tone } => {
                    Some(FeedsPageIntent::SetStatus { message, tone })
                }
                UiIntent::AuthenticatedShellLoaded(_)
                | UiIntent::StartupRouteResolved(_)
                | UiIntent::EntriesPage(_)
                | UiIntent::ReaderPage(_)
                | UiIntent::SettingsPage(_) => None,
            })
            .collect(),
    }
}

fn map_feeds_page_command(command: FeedsPageCommand) -> UiCommand {
    match command {
        FeedsPageCommand::AddFeed { raw_url } => UiCommand::FeedsAddFeed { raw_url },
        FeedsPageCommand::RefreshAll => UiCommand::FeedsRefreshAll,
        FeedsPageCommand::RefreshFeed { feed_id, feed_title } => {
            UiCommand::FeedsRefreshFeed { feed_id, feed_title }
        }
        FeedsPageCommand::RemoveFeed { feed_id, feed_title, confirmed } => {
            UiCommand::FeedsRemoveFeed { feed_id, feed_title, confirmed }
        }
        FeedsPageCommand::ExportConfig => UiCommand::FeedsExportConfig,
        FeedsPageCommand::ImportConfig { raw, confirmed } => {
            UiCommand::FeedsImportConfig { raw, confirmed }
        }
        FeedsPageCommand::ExportOpml => UiCommand::FeedsExportOpml,
        FeedsPageCommand::ImportOpml { raw } => UiCommand::FeedsImportOpml { raw },
    }
}

async fn read_feed_url_from_clipboard() -> Result<Option<String>, String> {
    document::eval(
        r#"
        if (typeof navigator === "undefined" || !navigator.clipboard || !navigator.clipboard.readText) {
            return null;
        }
        return navigator.clipboard.readText();
        "#,
    )
    .join::<Option<String>>()
    .await
    .map_err(|err| err.to_string())
}
