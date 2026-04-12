use dioxus::prelude::*;

use super::{intent::FeedsPageIntent, state::FeedsPageState};
use crate::ui::{FeedsCommand, UiCommand};

pub(crate) fn dispatch_feeds_page_intent(
    mut state: Signal<FeedsPageState>,
    intent: FeedsPageIntent,
) -> Vec<UiCommand> {
    let mut effects = Vec::new();
    state.with_mut(|state| effects = reduce_feeds_page_intent(state, intent));
    effects
}

pub(crate) fn reduce_feeds_page_intent(
    state: &mut FeedsPageState,
    intent: FeedsPageIntent,
) -> Vec<UiCommand> {
    match intent {
        FeedsPageIntent::LoadRequested => vec![UiCommand::Feeds(FeedsCommand::LoadSnapshot)],
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
            vec![UiCommand::Feeds(FeedsCommand::AddFeed { raw_url: state.feed_url.clone() })]
        }
        FeedsPageIntent::RefreshAllRequested => vec![UiCommand::Feeds(FeedsCommand::RefreshAll)],
        FeedsPageIntent::RefreshFeedRequested { feed_id, feed_title } => {
            vec![UiCommand::Feeds(FeedsCommand::RefreshFeed { feed_id, feed_title })]
        }
        FeedsPageIntent::RemoveFeedRequested { feed_id, feed_title } => {
            if state.pending_delete_feed != Some(feed_id) {
                state.pending_delete_feed = Some(feed_id);
                state.status = format!("再次点击即可删除订阅：{feed_title}");
                state.status_tone = "info".to_string();
                return Vec::new();
            }

            vec![UiCommand::Feeds(FeedsCommand::RemoveFeed { feed_id, feed_title })]
        }
        FeedsPageIntent::ExportConfigRequested => {
            vec![UiCommand::Feeds(FeedsCommand::ExportConfig)]
        }
        FeedsPageIntent::ImportConfigRequested => {
            if !state.pending_config_import {
                state.pending_config_import = true;
                state.status = "导入配置会按配置包覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。"
                    .to_string();
                state.status_tone = "info".to_string();
                return Vec::new();
            }

            vec![UiCommand::Feeds(FeedsCommand::ImportConfig { raw: state.config_text.clone() })]
        }
        FeedsPageIntent::ExportOpmlRequested => vec![UiCommand::Feeds(FeedsCommand::ExportOpml)],
        FeedsPageIntent::ImportOpmlRequested => {
            vec![UiCommand::Feeds(FeedsCommand::ImportOpml { raw: state.opml_text.clone() })]
        }
        FeedsPageIntent::PasteFeedUrlRequested => {
            vec![UiCommand::Feeds(FeedsCommand::ReadFeedUrlFromClipboard)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_feed_first_request_sets_pending_without_command() {
        let mut state = FeedsPageState::new();

        let effects = reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::RemoveFeedRequested { feed_id: 42, feed_title: "Example".to_string() },
        );

        assert!(effects.is_empty());
        assert_eq!(state.pending_delete_feed, Some(42));
        assert_eq!(state.status, "再次点击即可删除订阅：Example");
        assert_eq!(state.status_tone, "info");
    }

    #[test]
    fn remove_feed_confirmed_request_emits_execution_command() {
        let mut state = FeedsPageState::new();
        state.pending_delete_feed = Some(42);

        let effects = reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::RemoveFeedRequested { feed_id: 42, feed_title: "Example".to_string() },
        );

        assert_eq!(effects.len(), 1);
        match &effects[0] {
            UiCommand::Feeds(FeedsCommand::RemoveFeed { feed_id, feed_title }) => {
                assert_eq!(*feed_id, 42);
                assert_eq!(feed_title, "Example");
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn import_config_first_request_sets_pending_without_command() {
        let mut state = FeedsPageState::new();

        let effects = reduce_feeds_page_intent(&mut state, FeedsPageIntent::ImportConfigRequested);

        assert!(effects.is_empty());
        assert!(state.pending_config_import);
        assert_eq!(
            state.status,
            "导入配置会按配置包覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。"
        );
        assert_eq!(state.status_tone, "info");
    }

    #[test]
    fn import_config_confirmed_request_emits_execution_command() {
        let mut state = FeedsPageState::new();
        state.pending_config_import = true;
        state.config_text = "{\"feeds\":[]}".to_string();

        let effects = reduce_feeds_page_intent(&mut state, FeedsPageIntent::ImportConfigRequested);

        assert_eq!(effects.len(), 1);
        match &effects[0] {
            UiCommand::Feeds(FeedsCommand::ImportConfig { raw }) => {
                assert_eq!(raw, "{\"feeds\":[]}");
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }
}
