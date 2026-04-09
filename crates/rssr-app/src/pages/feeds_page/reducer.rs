use dioxus::prelude::*;

use super::{effect::FeedsPageEffect, intent::FeedsPageIntent, state::FeedsPageState};
use crate::ui::UiCommand;

pub(crate) fn dispatch_feeds_page_intent(
    mut state: Signal<FeedsPageState>,
    intent: FeedsPageIntent,
) -> Vec<FeedsPageEffect> {
    let mut effects = Vec::new();
    state.with_mut(|state| effects = reduce_feeds_page_intent(state, intent));
    effects
}

pub(crate) fn reduce_feeds_page_intent(
    state: &mut FeedsPageState,
    intent: FeedsPageIntent,
) -> Vec<FeedsPageEffect> {
    match intent {
        FeedsPageIntent::LoadRequested => vec![FeedsPageEffect::LoadSnapshot],
        FeedsPageIntent::FeedUrlChanged(value) => {
            state.feed_url = value;
            Vec::new()
        }
        FeedsPageIntent::ConfigTextChanged(value) => {
            state.pending_config_import = false;
            state.config_text = value;
            Vec::new()
        }
        FeedsPageIntent::OpmlTextChanged(value) => {
            state.opml_text = value;
            Vec::new()
        }
        FeedsPageIntent::AddFeedRequested => {
            vec![FeedsPageEffect::Dispatch(UiCommand::FeedsAddFeed {
                raw_url: state.feed_url.clone(),
            })]
        }
        FeedsPageIntent::RefreshAllRequested => {
            vec![FeedsPageEffect::Dispatch(UiCommand::FeedsRefreshAll)]
        }
        FeedsPageIntent::RefreshFeedRequested { feed_id, feed_title } => {
            vec![FeedsPageEffect::Dispatch(UiCommand::FeedsRefreshFeed { feed_id, feed_title })]
        }
        FeedsPageIntent::RemoveFeedRequested { feed_id, feed_title } => {
            vec![FeedsPageEffect::Dispatch(UiCommand::FeedsRemoveFeed {
                feed_id,
                feed_title,
                confirmed: state.pending_delete_feed == Some(feed_id),
            })]
        }
        FeedsPageIntent::ExportConfigRequested => {
            vec![FeedsPageEffect::Dispatch(UiCommand::FeedsExportConfig)]
        }
        FeedsPageIntent::ImportConfigRequested => {
            vec![FeedsPageEffect::Dispatch(UiCommand::FeedsImportConfig {
                raw: state.config_text.clone(),
                confirmed: state.pending_config_import,
            })]
        }
        FeedsPageIntent::ExportOpmlRequested => {
            vec![FeedsPageEffect::Dispatch(UiCommand::FeedsExportOpml)]
        }
        FeedsPageIntent::ImportOpmlRequested => {
            vec![FeedsPageEffect::Dispatch(UiCommand::FeedsImportOpml {
                raw: state.opml_text.clone(),
            })]
        }
        FeedsPageIntent::PasteFeedUrlRequested => {
            vec![FeedsPageEffect::Dispatch(UiCommand::FeedsReadFeedUrlFromClipboard)]
        }
        FeedsPageIntent::SnapshotLoaded(result) => {
            match result {
                Ok(snapshot) => {
                    state.feed_count = snapshot.feed_count;
                    state.entry_count = snapshot.entry_count;
                    state.feeds = snapshot.feeds;
                }
                Err(err) => {
                    state.status = err;
                    state.status_tone = "error".to_string();
                }
            }
            Vec::new()
        }
        FeedsPageIntent::ConfigTextExported(raw) => {
            state.config_text = raw;
            Vec::new()
        }
        FeedsPageIntent::OpmlTextExported(raw) => {
            state.opml_text = raw;
            Vec::new()
        }
        FeedsPageIntent::PendingConfigImportSet(pending) => {
            state.pending_config_import = pending;
            Vec::new()
        }
        FeedsPageIntent::PendingDeleteFeedSet(pending) => {
            state.pending_delete_feed = pending;
            Vec::new()
        }
        FeedsPageIntent::SetStatus { message, tone } => {
            state.status = message;
            state.status_tone = tone;
            Vec::new()
        }
        FeedsPageIntent::BumpReload => {
            state.reload_tick += 1;
            Vec::new()
        }
    }
}
