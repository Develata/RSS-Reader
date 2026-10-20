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
}
