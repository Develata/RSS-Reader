use std::future::Future;

use dioxus::prelude::*;

use super::{
    intent::EntriesPageIntent, reducer::dispatch_entries_page_intent, state::EntriesPageState,
};
use crate::ui::{
    EntriesCommand, UiCommand, UiIntent, execute_ui_command, remember_entry_controls_hidden,
    spawn_projected_ui_command,
};
use rssr_domain::EntryQuery;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct EntriesPageSession {
    feed_id: Option<i64>,
    state: Signal<EntriesPageState>,
    query_generation: Signal<u64>,
}

impl EntriesPageSession {
    pub(crate) fn new(
        feed_id: Option<i64>,
        state: Signal<EntriesPageState>,
        query_generation: Signal<u64>,
    ) -> Self {
        Self { feed_id, state, query_generation }
    }

    pub(crate) fn snapshot(self) -> EntriesPageState {
        (self.state)()
    }

    /// 借用状态而不是克隆整份。
    ///
    /// `snapshot()` 会克隆 `EntriesPageState`（集合以 Arc 共享，但 status / selected_feed_urls 仍需复制），
    /// 用在「每次状态变化都要跑一遍」的投影里代价白扔。`Readable::with` 同样会建立订阅。
    pub(crate) fn with_state<R>(self, read: impl FnOnce(&EntriesPageState) -> R) -> R {
        self.state.with(read)
    }

    pub(crate) fn feed_id(self) -> Option<i64> {
        self.feed_id
    }

    pub(crate) fn bootstrap(self, load_preferences: bool, load_feeds: bool) {
        self.spawn_ui_command(UiCommand::Entries(EntriesCommand::Bootstrap {
            feed_id: self.feed_id,
            load_preferences,
            load_feeds,
        }));
    }

    pub(crate) fn load_entries_query(self, query: EntryQuery) {
        self.spawn_entries_query(execute_ui_command(UiCommand::Entries(
            EntriesCommand::LoadEntries { query },
        )));
    }

    fn spawn_entries_query(mut self, query: impl Future<Output = Vec<UiIntent>> + 'static) {
        // Query and refresh changes can overlap: a slower earlier query must not replace
        // the latest list or its status. This token belongs to the page, not the global
        // refresh task; other commands (including flag writes) keep their own lifecycle.
        let generation = self.query_generation.with_mut(|generation| {
            *generation = generation.wrapping_add(1);
            *generation
        });
        spawn(async move {
            let intents = query.await;
            if *self.query_generation.peek() != generation {
                return;
            }
            for intent in intents.into_iter().filter_map(UiIntent::into_entries_page_intent) {
                self.dispatch(intent);
            }
        });
    }

    pub(crate) fn save_browsing_preferences_with(
        self,
        preferences_loaded: bool,
        grouping_mode: rssr_domain::EntryGroupingPreference,
        show_archived: bool,
        read_filter: rssr_domain::ReadFilter,
        starred_filter: rssr_domain::StarredFilter,
        selected_feed_urls: Vec<String>,
    ) {
        if !preferences_loaded {
            return;
        }

        self.spawn_ui_command(UiCommand::Entries(EntriesCommand::SaveBrowsingPreferences {
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        }));
    }

    pub(crate) fn toggle_read(self, entry_id: i64, entry_title: String, currently_read: bool) {
        self.spawn_ui_command(UiCommand::Entries(EntriesCommand::ToggleRead {
            entry_id,
            entry_title,
            currently_read,
        }));
    }

    pub(crate) fn toggle_starred(
        self,
        entry_id: i64,
        entry_title: String,
        currently_starred: bool,
    ) {
        self.spawn_ui_command(UiCommand::Entries(EntriesCommand::ToggleStarred {
            entry_id,
            entry_title,
            currently_starred,
        }));
    }

    pub(crate) fn dispatch(self, intent: EntriesPageIntent) {
        if let EntriesPageIntent::SetControlsHidden(hidden) = &intent {
            remember_entry_controls_hidden(*hidden);
        }
        dispatch_entries_page_intent(self.state, intent);
    }

    fn spawn_ui_command(self, command: UiCommand) {
        spawn_projected_ui_command(command, UiIntent::into_entries_page_intent, move |intent| {
            self.dispatch(intent);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rssr_domain::EntrySummary;
    use tokio::sync::oneshot;

    fn loaded(entry_id: i64) -> Vec<UiIntent> {
        vec![UiIntent::EntriesPage(EntriesPageIntent::SetEntries {
            entries: vec![EntrySummary {
                id: entry_id,
                feed_id: 1,
                title: format!("Entry {entry_id}"),
                feed_title: "Feed".into(),
                published_at: None,
                is_read: false,
                is_starred: false,
            }],
            archived_count: 0,
        })]
    }

    fn failure() -> Vec<UiIntent> {
        vec![UiIntent::EntriesPage(EntriesPageIntent::SetStatus {
            message: "old query failed".into(),
            tone: "error".into(),
        })]
    }

    fn complete_older_query_after_latest(older_result: Vec<UiIntent>) -> EntriesPageState {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (older_sender, older_receiver) = oneshot::channel();
        let (latest_sender, latest_receiver) = oneshot::channel();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
            );
            session
                .spawn_entries_query(async { older_receiver.await.expect("older query result") });
            session
                .spawn_entries_query(async { latest_receiver.await.expect("latest query result") });
            session
        });
        dom.render_immediate_to_vec();
        latest_sender.send(loaded(2)).expect("deliver latest result");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().entries[0].id, 2);
        });

        older_sender.send(older_result).expect("deliver older result");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || session.snapshot())
    }

    #[test]
    fn older_query_success_cannot_replace_latest_entries() {
        let state = complete_older_query_after_latest(loaded(1));
        assert_eq!(state.entries[0].id, 2);
        assert_eq!(state.status_tone, "info");
    }

    #[test]
    fn older_query_failure_cannot_replace_latest_status() {
        let state = complete_older_query_after_latest(failure());
        assert_eq!(state.entries[0].id, 2);
        assert_eq!(state.status, "共 1 篇文章。");
        assert_eq!(state.status_tone, "info");
    }
}
