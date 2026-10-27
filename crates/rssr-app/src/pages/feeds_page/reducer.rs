use dioxus::prelude::*;

use super::{intent::FeedsPageIntent, state::FeedsPageState};
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
                    Err(error) => state.edit_error = Some(error.message()),
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
                    Err(error) => state.edit_error = Some(error.message()),
                }
            }
            Vec::new()
        }
        FeedsPageIntent::FeedCandidates { token, page_url, candidates } => {
            if token == state.add_token
                && state.adding_feed_url.as_deref() == Some(state.feed_url.as_str())
            {
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
            state.add_token = state.add_token.wrapping_add(1);
            let command = FeedsCommand::AddFeed {
                raw_url: candidate.url.to_string(),
                fallback_site_url: Some(page_url.clone()),
                token: state.add_token,
            };
            state.adding_feed_url = Some(state.feed_url.clone());
            state.feed_candidates = None;
            vec![UiCommand::Feeds(command)]
        }

        FeedsPageIntent::ClearRefreshStatus { revision, message } => {
            if state.status_revision == revision && state.status == message {
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
            state.add_token = state.add_token.wrapping_add(1);
            vec![UiCommand::Feeds(FeedsCommand::AddFeed {
                raw_url: state.feed_url.clone(),
                fallback_site_url: None,
                token: state.add_token,
            })]
        }
        FeedsPageIntent::AddFeedFinished { token, saved } => {
            if token != state.add_token {
                return Vec::new();
            }
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
            let Some(expected) = state.fingerprint.clone() else {
                state.pending_config_import = false;
                state.status = "订阅列表尚未载入，载入后再导入配置。".to_string();
                state.status_tone = "info".to_string();
                return Vec::new();
            };
            if !state.pending_config_import {
                state.pending_config_import = true;
                state.status = "导入配置会按配置包覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。"
                    .to_string();
                state.status_tone = "info".to_string();
                return Vec::new();
            }

            vec![UiCommand::Feeds(FeedsCommand::ImportConfig {
                raw: state.config_text.clone(),
                expected,
            })]
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
                    // A confirmation was asked against the configuration shown; it changed, so
                    // the destructive step asks again (C4b item 2).
                    let changed = state
                        .fingerprint
                        .as_ref()
                        .is_some_and(|shown| *shown != snapshot.fingerprint);
                    if changed
                        && (state.pending_config_import || state.pending_delete_feed.is_some())
                    {
                        clear_pending_confirmations(state);
                        state.status_revision = state.status_revision.wrapping_add(1);
                        state.status = "订阅或设置已变化，请重新确认。".to_string();
                        state.status_tone = "info".to_string();
                    }
                    state.feed_count = snapshot.feed_count;
                    state.entry_count = snapshot.entry_count;
                    state.unread_count = snapshot.unread_count;
                    state.archived_count = snapshot.archived_count;
                    state.feeds = snapshot.feeds;
                    state.fingerprint = Some(snapshot.fingerprint);
                }
                Err(failure) => {
                    state.status = failure.message();
                    state.status_tone = failure.tone().to_string();
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
        FeedsPageIntent::Failed(failure) => {
            state.status_revision = state.status_revision.wrapping_add(1);
            state.status = failure.message();
            state.status_tone = failure.tone().to_string();
            Vec::new()
        }
        FeedsPageIntent::BumpReload => {
            state.reload_tick += 1;
            Vec::new()
        }
        // 外观属于 shell 而不是订阅页状态，`FeedsPageSession` 在进入 reducer 之前处理。
        FeedsPageIntent::SettingsReplaced(_) => Vec::new(),
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
                result: Err(crate::ui::failure::plain("old read failed")),
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
                result: Err(crate::ui::failure::plain("save failed")),
            },
        );
        assert!(!state.edit_busy);
        assert_eq!(state.edit_error.as_deref(), Some("测试失败：save failed"));
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

    fn loaded_state() -> FeedsPageState {
        let mut state = FeedsPageState::new();
        state.fingerprint = Some(fingerprint());
        state
    }

    fn fingerprint() -> rssr_domain::ConfigFingerprint {
        rssr_domain::ConfigFingerprint::compute(Vec::new(), &rssr_domain::UserSettings::default())
    }

    /// Until a snapshot is shown there is no configuration to replace knowingly.
    #[test]
    fn import_config_waits_for_the_snapshot() {
        let mut state = FeedsPageState::new();
        state.pending_config_import = true;

        let effects = reduce_feeds_page_intent(&mut state, FeedsPageIntent::ImportConfigRequested);

        assert!(effects.is_empty());
        assert!(!state.pending_config_import);
        assert!(state.status.contains("尚未载入"));
    }

    #[test]
    fn import_config_first_request_sets_pending_without_command() {
        let mut state = loaded_state();

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
        let mut state = loaded_state();
        state.pending_config_import = true;
        state.config_text = "{\"feeds\":[]}".to_string();

        let effects = reduce_feeds_page_intent(&mut state, FeedsPageIntent::ImportConfigRequested);

        assert_eq!(effects.len(), 1);
        match &effects[0] {
            UiCommand::Feeds(FeedsCommand::ImportConfig { raw, expected }) => {
                assert_eq!(raw, "{\"feeds\":[]}");
                assert_eq!(expected, &fingerprint());
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
        let mut state = loaded_state();
        state.pending_delete_feed = Some(42);

        let effects = reduce_feeds_page_intent(&mut state, FeedsPageIntent::ImportConfigRequested);

        assert!(effects.is_empty());
        assert_eq!(state.pending_delete_feed, None);
        assert!(state.pending_config_import);
    }

    fn snapshot_of(title: &str) -> FeedsPageIntent {
        let feed = rssr_domain::FingerprintFeed {
            id: 7,
            url: "https://example.com/feed.xml",
            generation: 1,
            custom_title: Some(title),
            folder: None,
        };
        FeedsPageIntent::SnapshotLoaded(Ok(crate::pages::feeds_page::intent::FeedsPageSnapshot {
            unread_count: 0,
            archived_count: 0,
            feeds: Vec::new(),
            feed_count: 1,
            entry_count: 0,
            fingerprint: rssr_domain::ConfigFingerprint::compute(
                vec![feed],
                &rssr_domain::UserSettings::default(),
            ),
        }))
    }

    /// A confirmation asked against the configuration shown is dropped when a read shows it
    /// changed (another browser, a job): the destructive step asks again (C4b item 2). The same
    /// configuration read again keeps it; the first read has nothing to compare.
    #[test]
    fn a_changed_configuration_drops_waiting_confirmations() {
        let mut state = FeedsPageState::new();
        state.pending_config_import = true;
        reduce_feeds_page_intent(&mut state, snapshot_of("A"));
        assert!(state.pending_config_import, "the first read");

        reduce_feeds_page_intent(&mut state, snapshot_of("A"));
        assert!(state.pending_config_import, "unchanged");

        reduce_feeds_page_intent(&mut state, snapshot_of("B"));
        assert!(!state.pending_config_import);
        assert_eq!(state.status, "订阅或设置已变化，请重新确认。");

        state.pending_delete_feed = Some(7);
        reduce_feeds_page_intent(&mut state, snapshot_of("C"));
        assert_eq!(state.pending_delete_feed, None);
    }

    /// An add's results apply to that add only: an older one's end or candidates do not release
    /// the gate of the add now running (C4b item 3, R2-02).
    #[test]
    fn an_add_result_reaches_only_its_operation() {
        let mut state = FeedsPageState::new();
        state.feed_url = "https://example.com".into();
        let commands = reduce_feeds_page_intent(&mut state, FeedsPageIntent::AddFeedRequested);
        let [UiCommand::Feeds(FeedsCommand::AddFeed { token, .. })] = commands.as_slice() else {
            panic!("{commands:?}")
        };
        let token = *token;
        let older = token.wrapping_sub(1);
        reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::FeedCandidates {
                token: older,
                page_url: "https://example.com".parse().unwrap(),
                candidates: Vec::new(),
            },
        );
        reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::AddFeedFinished { token: older, saved: true },
        );
        assert!(state.adding_feed_url.is_some(), "still adding");
        assert!(state.feed_candidates.is_none());
        assert_eq!(state.feed_url, "https://example.com");

        reduce_feeds_page_intent(
            &mut state,
            FeedsPageIntent::AddFeedFinished { token, saved: true },
        );
        assert!(state.adding_feed_url.is_none());
        assert!(state.feed_url.is_empty());
    }
}
