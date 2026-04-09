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
        FeedsPageIntent::CommandCompleted(outcome) => {
            if let Some(next) = outcome.patch.feed_url {
                state.feed_url = next;
            }
            if let Some(next) = outcome.patch.config_text {
                state.config_text = next;
            }
            if let Some(next) = outcome.patch.opml_text {
                state.opml_text = next;
            }
            if let Some(next) = outcome.patch.pending_config_import {
                state.pending_config_import = next;
            }
            if let Some(next) = outcome.patch.pending_delete_feed {
                state.pending_delete_feed = next;
            }
            state.status = outcome.status_message;
            state.status_tone = outcome.status_tone.to_string();
            if outcome.reload {
                state.reload_tick += 1;
            }
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
