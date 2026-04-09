use dioxus::prelude::*;

use super::{
    dispatch::execute_command, effect::FeedsPageEffect, intent::FeedsPageIntent,
    queries::load_feeds_page_snapshot,
};

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
            FeedsPageRuntimeOutcome::single(FeedsPageIntent::SnapshotLoaded(
                load_feeds_page_snapshot().await.map_err(|err| err.to_string()),
            ))
        }
        FeedsPageEffect::ExecuteCommand(command) => FeedsPageRuntimeOutcome::single(
            FeedsPageIntent::CommandCompleted(execute_command(command).await),
        ),
        FeedsPageEffect::ReadFeedUrlFromClipboard => FeedsPageRuntimeOutcome::single(
            FeedsPageIntent::ClipboardReadCompleted(read_feed_url_from_clipboard().await),
        ),
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
