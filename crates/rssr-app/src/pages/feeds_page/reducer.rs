use dioxus::prelude::*;

use super::{
    intent::FeedsPageIntent, request_policy::refresh_status_matches, state::FeedsPageState,
};
use crate::ui::{FeedsCommand, ShellCommand, UiCommand};

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
        FeedsPageIntent::EditRequested(feed_id) => {
            if state.edit_busy && state.edit_details.is_some() {
                return Vec::new();
            }
            if state.pending_delete_feed.is_some() || state.pending_config_import {
                state.status.clear();
                state.status_revision = state.status_revision.wrapping_add(1);
            }
            clear_pending_confirmations(state);
            state.edit_generation = state.edit_generation.wrapping_add(1);
            state.edit_feed_id = Some(feed_id);
            state.edit_details = None;
            state.edit_error = None;
            state.edit_busy = true;
            vec![UiCommand::Feeds(FeedsCommand::LoadEdit {
                feed_id,
                generation: state.edit_generation,
            })]
        }
        FeedsPageIntent::EditLoaded { feed_id, generation, result } => {
            // A second visit to the same feed must not accept the first visit's
            // delayed read, especially after the new draft has begun saving.
            if state.edit_feed_id == Some(feed_id)
                && state.edit_generation == generation
                && state.edit_busy
                && state.edit_details.is_none()
            {
                state.edit_busy = false;
                match result {
                    Ok(details) => state.edit_details = Some(details),
                    Err(error) => state.edit_error = Some(error),
                }
            }
            Vec::new()
        }
        FeedsPageIntent::EditTitleChanged(title) => {
            if !state.edit_busy
                && let Some(edit) = &mut state.edit_details
            {
                edit.custom_title = (!title.is_empty()).then_some(title);
            }
            Vec::new()
        }
        FeedsPageIntent::EditUrlChanged(url) => {
            if !state.edit_busy
                && let Some(edit) = &mut state.edit_details
            {
                edit.url = url;
            }
            Vec::new()
        }
        FeedsPageIntent::EditClosed => {
            if !state.edit_busy || state.edit_details.is_none() {
                state.edit_generation = state.edit_generation.wrapping_add(1);
                state.edit_feed_id = None;
                state.edit_details = None;
                state.edit_error = None;
                state.edit_busy = false;
            }
            Vec::new()
        }
        FeedsPageIntent::EditSaveRequested => {
            if state.edit_busy {
                return Vec::new();
            }
            let Some(edit) = &state.edit_details else {
                return Vec::new();
            };
            state.edit_busy = true;
            state.edit_error = None;
            vec![UiCommand::Feeds(FeedsCommand::SaveEdit {
                generation: state.edit_generation,
                edit: rssr_domain::FeedEditRequest {
                    feed_id: edit.feed_id,
                    url: edit.url.clone(),
                    custom_title: edit.custom_title.clone(),
                },
            })]
        }
        FeedsPageIntent::EditSaved { feed_id, generation, result } => {
            if state.edit_feed_id == Some(feed_id) && state.edit_generation == generation {
                state.edit_busy = false;
                match result {
                    Ok(()) => {
                        state.edit_feed_id = None;
                        state.edit_details = None;
                        state.edit_error = None;
                        state.reload_tick += 1;
                        state.status = "来源已更新。".into();
                        state.status_tone = "success".into();
                    }
                    Err(error) => state.edit_error = Some(error),
                }
            }
            Vec::new()
        }
        FeedsPageIntent::FeedCandidates { page_url, candidates } => {
            if state.adding_feed_url.as_deref() == Some(state.feed_url.as_str()) {
                state.feed_candidates = Some((page_url, candidates));
                state.status = "发现多个订阅，请选择一个地址。".into();
                state.status_tone = "info".into();
            }
            Vec::new()
        }
        FeedsPageIntent::CancelFeedCandidates => {
            state.feed_candidates = None;
            state.status.clear();
            Vec::new()
        }
        FeedsPageIntent::SelectFeedCandidate(index) => {
            if state.adding_feed_url.is_some() {
                return Vec::new();
            }
            let Some((page_url, candidates)) = &state.feed_candidates else {
                return Vec::new();
            };
            let Some(candidate) = candidates.get(index) else {
                return Vec::new();
            };
            let command = FeedsCommand::AddFeed {
                raw_url: candidate.url.to_string(),
                fallback_site_url: Some(page_url.clone()),
            };
            state.adding_feed_url = Some(state.feed_url.clone());
            state.feed_candidates = None;
            vec![UiCommand::Feeds(command)]
        }

        FeedsPageIntent::ClearRefreshStatus { revision, message } => {
            if refresh_status_matches(revision, &message, state.status_revision, &state.status) {
                state.status.clear();
            }
            Vec::new()
        }
        FeedsPageIntent::LoadRequested => vec![UiCommand::Feeds(FeedsCommand::LoadSnapshot)],
        FeedsPageIntent::FeedUrlChanged(value) => {
            clear_pending_confirmations(state);
            state.feed_candidates = None;
            state.feed_url = value;
            Vec::new()
        }
        FeedsPageIntent::ConfigTextChanged(value) => {
            clear_pending_confirmations(state);
            state.config_text = value;
            Vec::new()
        }
        FeedsPageIntent::OpmlTextChanged(value) => {
            clear_pending_confirmations(state);
            state.opml_text = value;
            Vec::new()
        }
        FeedsPageIntent::AddFeedRequested => {
            if state.adding_feed_url.is_some() {
                return Vec::new();
            }
            clear_pending_confirmations(state);
            state.adding_feed_url = Some(state.feed_url.clone());
            vec![UiCommand::Feeds(FeedsCommand::AddFeed {
                raw_url: state.feed_url.clone(),
                fallback_site_url: None,
            })]
        }
        FeedsPageIntent::AddFeedFinished { saved } => {
            // Adding includes the first network refresh. Keep a new address typed while
            // that request was running; only the successfully submitted draft is cleared.
            if let Some(submitted_url) = state.adding_feed_url.take()
                && saved
                && state.feed_url == submitted_url
            {
                state.feed_url.clear();
            }
            Vec::new()
        }
        FeedsPageIntent::RefreshAllRequested => {
            clear_pending_confirmations(state);
            vec![UiCommand::Shell(ShellCommand::ManualRefresh)]
        }
        FeedsPageIntent::RefreshFeedRequested { feed_id, feed_title } => {
            clear_pending_confirmations(state);
            vec![UiCommand::Feeds(FeedsCommand::RefreshFeed { feed_id, feed_title })]
        }
        FeedsPageIntent::RemoveFeedRequested { feed_id, feed_title } => {
            state.pending_config_import = false;
            if state.pending_delete_feed != Some(feed_id) {
                state.pending_delete_feed = Some(feed_id);
                state.status = format!("再次点击即可删除订阅：{feed_title}");
                state.status_tone = "info".to_string();
                return Vec::new();
            }

            vec![UiCommand::Feeds(FeedsCommand::RemoveFeed { feed_id, feed_title })]
        }
        FeedsPageIntent::ExportConfigRequested => {
            clear_pending_confirmations(state);
            vec![UiCommand::Feeds(FeedsCommand::ExportConfig)]
        }
        FeedsPageIntent::ImportConfigRequested => {
            state.pending_delete_feed = None;
            if !state.pending_config_import {
                state.pending_config_import = true;
                state.status = "导入配置会按配置包覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。"
                    .to_string();
                state.status_tone = "info".to_string();
                return Vec::new();
            }

            vec![UiCommand::Feeds(FeedsCommand::ImportConfig { raw: state.config_text.clone() })]
        }
        FeedsPageIntent::ExportOpmlRequested => {
            clear_pending_confirmations(state);
            vec![UiCommand::Feeds(FeedsCommand::ExportOpml)]
        }
        FeedsPageIntent::ImportOpmlRequested => {
            clear_pending_confirmations(state);
            vec![UiCommand::Feeds(FeedsCommand::ImportOpml { raw: state.opml_text.clone() })]
        }
        FeedsPageIntent::SnapshotLoaded(result) => {
            match result {
                Ok(snapshot) => {
                    state.feed_count = snapshot.feed_count;
                    state.entry_count = snapshot.entry_count;
                    state.unread_count = snapshot.unread_count;
                    state.archived_count = snapshot.archived_count;
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
            state.pending_config_import = false;
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
            state.status_revision = state.status_revision.wrapping_add(1);
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

fn clear_pending_confirmations(state: &mut FeedsPageState) {
    state.pending_config_import = false;
    state.pending_delete_feed = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit_details() -> rssr_domain::FeedEditDetails {
        rssr_domain::FeedEditDetails {
            feed_id: 42,
            url: "https://example.com/feed".into(),
            original_title: Some("Original".into()),
            custom_title: None,
        }
    }

    #[test]
    fn editing_clears_previous_destructive_confirmations() {
        let mut state = FeedsPageState::new();
        for confirmation in [
            FeedsPageIntent::RemoveFeedRequested { feed_id: 42, feed_title: "Original".into() },
            FeedsPageIntent::ImportConfigRequested,
        ] {
            reduce_feeds_page_intent(&mut state, confirmation);
            reduce_feeds_page_intent(&mut state, FeedsPageIntent::EditRequested(42));
            reduce_feeds_page_intent(&mut state, FeedsPageIntent::EditClosed);
            assert_eq!(state.pending_delete_feed, None);
            assert!(!state.pending_config_import);
        }
        assert!(
            reduce_feeds_page_intent(
                &mut state,
                FeedsPageIntent::RemoveFeedRequested { feed_id: 42, feed_title: "Original".into() },
            )
            .is_empty(),
            "deleting after an edit must require a fresh confirmation"
        );
    }

    #[test]
    fn old_edit_load_cannot_replace_reopened_draft_or_release_save_gate() {
        let mut state = FeedsPageState::new();
        reduce_feeds_page_intent(&mut state, FeedsPageIntent::EditRequested(42));
        let older = state.edit_generation;
        reduce_feeds_page_intent(&mut state, FeedsPageIntent::EditClosed);
        reduce_feeds_page_intent(&mut state, FeedsPageIntent::EditRequested(42));
        let latest = state.edit_generation;
        assert_ne!(older, latest);
        // A late failure must not dismiss the new visit's loading state either.
        reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::EditLoaded {
                feed_id: 42,
                generation: older,
                result: Err("old read failed".into()),
            },
        );
        assert!(state.edit_busy);
        assert!(state.edit_error.is_none());
        reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::EditLoaded {
                feed_id: 42,
                generation: latest,
                result: Ok(edit_details()),
            },
        );
        reduce_feeds_page_intent(&mut state, FeedsPageIntent::EditTitleChanged("My draft".into()));
        reduce_feeds_page_intent(&mut state, FeedsPageIntent::EditSaveRequested);
        // The first visit's read finishes after the second visit has started saving.
        reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::EditLoaded {
                feed_id: 42,
                generation: older,
                result: Ok(edit_details()),
            },
        );
        assert_eq!(state.edit_details.as_ref().unwrap().custom_title.as_deref(), Some("My draft"));
        assert!(state.edit_busy, "a stale read must not make an in-flight save dismissible");
        reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::EditSaved { feed_id: 42, generation: older, result: Ok(()) },
        );
        assert_eq!(state.edit_feed_id, Some(42));
        reduce_feeds_page_intent(&mut state, FeedsPageIntent::EditClosed);
        assert_eq!(state.edit_feed_id, Some(42), "saving still blocks dismissal");
        reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::EditSaved {
                feed_id: 42,
                generation: latest,
                result: Err("save failed".into()),
            },
        );
        assert!(!state.edit_busy);
        assert_eq!(state.edit_error.as_deref(), Some("save failed"));
        assert_eq!(state.edit_details.unwrap().custom_title.as_deref(), Some("My draft"));
    }

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

    #[test]
    fn unrelated_user_action_clears_pending_delete_confirmation() {
        let mut state = FeedsPageState::new();
        state.pending_delete_feed = Some(42);

        let effects = reduce_feeds_page_intent(&mut state, FeedsPageIntent::RefreshAllRequested);

        assert_eq!(effects.len(), 1);
        assert_eq!(state.pending_delete_feed, None);
        match &effects[0] {
            UiCommand::Shell(ShellCommand::ManualRefresh) => {}
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn unrelated_user_action_clears_pending_config_import_confirmation() {
        let mut state = FeedsPageState::new();
        state.pending_config_import = true;

        let effects = reduce_feeds_page_intent(&mut state, FeedsPageIntent::ExportOpmlRequested);

        assert_eq!(effects.len(), 1);
        assert!(!state.pending_config_import);
        match &effects[0] {
            UiCommand::Feeds(FeedsCommand::ExportOpml) => {}
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn exported_config_text_clears_pending_config_import_confirmation() {
        let mut state = FeedsPageState::new();
        state.pending_config_import = true;

        let effects = reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::ConfigTextExported("{\"feeds\":[]}".to_string()),
        );

        assert!(effects.is_empty());
        assert!(!state.pending_config_import);
        assert_eq!(state.config_text, "{\"feeds\":[]}");
    }

    #[test]
    fn switching_confirmation_domains_clears_previous_pending_state() {
        let mut state = FeedsPageState::new();
        state.pending_delete_feed = Some(42);

        let effects = reduce_feeds_page_intent(&mut state, FeedsPageIntent::ImportConfigRequested);

        assert!(effects.is_empty());
        assert_eq!(state.pending_delete_feed, None);
        assert!(state.pending_config_import);
    }
}
