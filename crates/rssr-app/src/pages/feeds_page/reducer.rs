use dioxus::prelude::*;

use super::{
    commands::FeedsPageCommand, effect::FeedsPageEffect, intent::FeedsPageIntent,
    state::FeedsPageState,
};

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
            vec![FeedsPageEffect::ExecuteCommand(FeedsPageCommand::AddFeed {
                raw_url: state.feed_url.clone(),
            })]
        }
        FeedsPageIntent::RefreshAllRequested => {
            vec![FeedsPageEffect::ExecuteCommand(FeedsPageCommand::RefreshAll)]
        }
        FeedsPageIntent::RefreshFeedRequested { feed_id, feed_title } => {
            vec![FeedsPageEffect::ExecuteCommand(FeedsPageCommand::RefreshFeed {
                feed_id,
                feed_title,
            })]
        }
        FeedsPageIntent::RemoveFeedRequested { feed_id, feed_title } => {
            vec![FeedsPageEffect::ExecuteCommand(FeedsPageCommand::RemoveFeed {
                feed_id,
                feed_title,
                confirmed: state.pending_delete_feed == Some(feed_id),
            })]
        }
        FeedsPageIntent::ExportConfigRequested => {
            vec![FeedsPageEffect::ExecuteCommand(FeedsPageCommand::ExportConfig)]
        }
        FeedsPageIntent::ImportConfigRequested => {
            vec![FeedsPageEffect::ExecuteCommand(FeedsPageCommand::ImportConfig {
                raw: state.config_text.clone(),
                confirmed: state.pending_config_import,
            })]
        }
        FeedsPageIntent::ExportOpmlRequested => {
            vec![FeedsPageEffect::ExecuteCommand(FeedsPageCommand::ExportOpml)]
        }
        FeedsPageIntent::ImportOpmlRequested => {
            vec![FeedsPageEffect::ExecuteCommand(FeedsPageCommand::ImportOpml {
                raw: state.opml_text.clone(),
            })]
        }
        FeedsPageIntent::PasteFeedUrlRequested => vec![FeedsPageEffect::ReadFeedUrlFromClipboard],
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
        FeedsPageIntent::ClipboardReadCompleted(result) => {
            match result {
                Ok(Some(text)) => state.feed_url = text,
                Ok(None) => {}
                Err(err) => {
                    state.status = format!("读取系统剪贴板失败：{err}");
                    state.status_tone = "error".to_string();
                }
            }
            Vec::new()
        }
    }
}
