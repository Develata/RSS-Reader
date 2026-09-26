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
    bootstrap_generation: Signal<u64>,
}

impl EntriesPageSession {
    pub(crate) fn preview_mark_read(self, query: EntryQuery) {
        self.spawn_bulk(EntriesCommand::PreviewMarkRead { query }, false);
    }
    pub(crate) fn confirm_mark_read(self) {
        let preview = self.state.peek().bulk_preview.clone();
        if let Some(preview) = preview
            && !preview.unread_entry_ids.is_empty()
        {
            self.spawn_bulk(EntriesCommand::ConfirmMarkRead { preview }, true);
        }
    }
    fn spawn_bulk(self, command: EntriesCommand, writing: bool) {
        if self.state.peek().bulk_busy {
            return;
        }
        self.dispatch(EntriesPageIntent::BeginBulk { writing });
        let generation = self.state.peek().bulk_generation;
        spawn(async move {
            let intents = execute_ui_command(UiCommand::Entries(command)).await;
            if self.state.peek().bulk_generation != generation {
                return;
            }
            for intent in intents.into_iter().filter_map(UiIntent::into_entries_page_intent) {
                let applied = matches!(&intent, EntriesPageIntent::BulkApplied(_));
                self.dispatch(intent);
                if applied {
                    self.bootstrap();
                }
            }
        });
    }

    pub(crate) fn new(
        feed_id: Option<i64>,
        state: Signal<EntriesPageState>,
        query_generation: Signal<u64>,
        bootstrap_generation: Signal<u64>,
    ) -> Self {
        Self { feed_id, state, query_generation, bootstrap_generation }
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

    pub(crate) fn bootstrap(self) {
        self.spawn_bootstrap(execute_ui_command(self.bootstrap_command()));
    }

    fn spawn_bootstrap(mut self, bootstrap: impl Future<Output = Vec<UiIntent>> + 'static) {
        // Refresh may start a new bootstrap while an older initialization is
        // still waiting. Its stale success or failure must not replace the newer
        // preferences/source map. Queries use a separate generation because
        // both operations are allowed to run concurrently after initialization.
        let generation = self.bootstrap_generation.with_mut(|generation| {
            *generation = generation.wrapping_add(1);
            *generation
        });
        spawn(async move {
            let intents = bootstrap.await;
            if *self.bootstrap_generation.peek() != generation {
                return;
            }
            for intent in intents.into_iter().filter_map(UiIntent::into_entries_page_intent) {
                self.dispatch(intent);
            }
        });
    }

    fn bootstrap_command(self) -> UiCommand {
        // Reading readiness must not subscribe the bootstrap effect to its own
        // completion and start a second feed-summary query.
        let load_preferences = !self.state.peek().preferences_load.is_loaded();
        UiCommand::Entries(EntriesCommand::Bootstrap {
            feed_id: self.feed_id,
            load_preferences,
            load_feeds: true,
        })
    }

    pub(crate) fn load_entries_query(self, query: EntryQuery) {
        let key = self.state.peek().position_key(self.feed_id, query.search_title.as_deref());
        let return_page = (!self.state.peek().entries_loaded)
            .then(|| crate::ui::reading_position::remembered_page(&key))
            .flatten();
        self.spawn_entries_query_with_return(
            execute_ui_command(UiCommand::Entries(EntriesCommand::LoadEntries { query })),
            return_page,
        );
    }

    #[cfg(test)]
    fn spawn_entries_query(self, query: impl Future<Output = Vec<UiIntent>> + 'static) {
        self.spawn_entries_query_with_return(query, None);
    }

    fn spawn_entries_query_with_return(
        mut self,
        query: impl Future<Output = Vec<UiIntent>> + 'static,
        return_page: Option<u32>,
    ) {
        if !self.state.peek().preferences_load.can_load_entries() {
            return;
        }
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
                let loaded = matches!(&intent, EntriesPageIntent::SetEntries { .. });
                self.dispatch(intent);
                if loaded && let Some(page) = return_page {
                    self.dispatch(EntriesPageIntent::SetCurrentPage(page));
                }
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
        if !self.can_save_browsing_preferences(preferences_loaded) {
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

    fn can_save_browsing_preferences(self, preferences_loaded: bool) -> bool {
        // The captured flag also matters: an effect queued before bootstrap
        // finished still contains default values, even if the live state is ready.
        preferences_loaded && self.state.peek().preferences_load.is_loaded()
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
            let read_changed =
                matches!(&intent, EntriesPageIntent::PatchEntryFlags { is_read: Some(_), .. });
            self.dispatch(intent);
            if read_changed {
                self.bootstrap();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rssr_domain::{EntriesWorkspaceState, EntrySummary, FeedSummary, ReadFilter, UserSettings};
    use std::{cell::Cell, rc::Rc};
    use time::OffsetDateTime;
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
                Signal::new(0),
            );
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
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
    fn returning_page_is_applied_after_entries_arrive_and_clamped() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let session = dom.in_scope(ScopeId::APP, || {
            let mut state = EntriesPageState::new(true);
            state.entries_page_size = 1;
            let session =
                EntriesPageSession::new(None, Signal::new(state), Signal::new(0), Signal::new(0));
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            session.spawn_entries_query_with_return(
                async {
                    let mut intents = loaded(1);
                    if let UiIntent::EntriesPage(EntriesPageIntent::SetEntries {
                        entries, ..
                    }) = &mut intents[0]
                    {
                        let mut second = entries[0].clone();
                        second.id = 2;
                        entries.push(second);
                    }
                    intents
                },
                Some(9),
            );
            session
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert!(session.snapshot().entries_loaded);
            assert_eq!(session.snapshot().current_page, 2);
        });
    }

    #[test]
    fn older_query_failure_cannot_replace_latest_status() {
        let state = complete_older_query_after_latest(failure());
        assert_eq!(state.entries[0].id, 2);
        assert_eq!(state.status, "共 1 篇文章。");
        assert_eq!(state.status_tone, "info");
    }

    #[test]
    fn first_query_waits_for_all_preferences_and_source_mapping() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let queries = Rc::new(Cell::new(0));
        let session = dom.in_scope(ScopeId::APP, || {
            EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
                Signal::new(0),
            )
        });
        let prepare = [
            EntriesPageIntent::ApplyLoadedSettings(UserSettings {
                entries_page_size: 50,
                ..UserSettings::default()
            }),
            EntriesPageIntent::ApplyLoadedWorkspaceState(EntriesWorkspaceState {
                read_filter: ReadFilter::UnreadOnly,
                selected_feed_urls: vec!["https://example.com/selected".into()],
                ..EntriesWorkspaceState::default()
            }),
            EntriesPageIntent::SetFeeds(vec![FeedSummary {
                id: 7,
                title: "Selected source".into(),
                url: "https://example.com/selected".into(),
                unread_count: 1,
                entry_count: 1,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
            }]),
            EntriesPageIntent::PreferencesLoaded,
        ];
        for intent in std::iter::once(None).chain(prepare.into_iter().map(Some)) {
            dom.in_scope(ScopeId::APP, || {
                if let Some(intent) = intent {
                    session.dispatch(intent);
                }
                let queries = Rc::clone(&queries);
                session.spawn_entries_query(async move {
                    let state = session.snapshot();
                    let query = state.entry_query(None, None, OffsetDateTime::now_utc());
                    assert_eq!(state.page_size(), 50);
                    assert_eq!(query.read_filter, ReadFilter::UnreadOnly);
                    assert_eq!(query.feed_ids, vec![7]);
                    queries.set(queries.get() + 1);
                    loaded(1)
                });
            });
            dom.render_immediate_to_vec();
        }
        assert_eq!(queries.get(), 1, "no default/partial query may execute");
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().entries.len(), 1);
            // A pre-bootstrap effect must not save the defaults it captured.
            assert!(!session.can_save_browsing_preferences(false));
            assert!(session.can_save_browsing_preferences(true));
        });
    }

    #[test]
    fn unavailable_preferences_allow_reading_without_saving_and_retry_on_refresh() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
                Signal::new(0),
            );
            session
                .dispatch(EntriesPageIntent::PreferencesUnavailable("broken preferences".into()));
            assert!(!session.can_save_browsing_preferences(true));
            assert!(matches!(
                session.bootstrap_command(),
                UiCommand::Entries(EntriesCommand::Bootstrap { load_preferences: true, .. })
            ));
            session.spawn_entries_query(async { loaded(1) });
            session
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let state = session.snapshot();
            assert_eq!(state.entries.len(), 1);
            assert!(state.status.contains("broken preferences"));
            assert_eq!(state.status_tone, "error");
            assert!(!session.can_save_browsing_preferences(true));
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            assert!(session.can_save_browsing_preferences(true));
            // A successful retry can keep the same query. Clear the fallback
            // warning even when no further SetEntries intent is necessary.
            assert_eq!(session.snapshot().status_tone, "info");
            assert_eq!(session.snapshot().status, "共 1 篇文章。");
            assert!(matches!(
                session.bootstrap_command(),
                UiCommand::Entries(EntriesCommand::Bootstrap {
                    load_preferences: false,
                    load_feeds: true,
                    ..
                })
            ));
            session.spawn_entries_query(async { loaded(2) });
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().status_tone, "info");
            assert_eq!(session.snapshot().entries[0].id, 2);
        });
    }

    fn bootstrap_loaded(feed_id: i64, page_size: u32) -> Vec<UiIntent> {
        let url = format!("https://example.com/source/{feed_id}");
        vec![
            EntriesPageIntent::ApplyLoadedSettings(UserSettings {
                entries_page_size: page_size,
                ..UserSettings::default()
            }),
            EntriesPageIntent::ApplyLoadedWorkspaceState(EntriesWorkspaceState {
                read_filter: ReadFilter::UnreadOnly,
                selected_feed_urls: vec![url.clone()],
                ..EntriesWorkspaceState::default()
            }),
            EntriesPageIntent::SetFeeds(vec![FeedSummary {
                id: feed_id,
                title: format!("Source {feed_id}"),
                url,
                unread_count: 1,
                entry_count: 1,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
            }]),
            EntriesPageIntent::PreferencesLoaded,
        ]
        .into_iter()
        .map(UiIntent::EntriesPage)
        .collect()
    }

    fn complete_older_bootstrap_after_latest(older_result: Vec<UiIntent>) {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (older_sender, older_receiver) = oneshot::channel();
        let (latest_sender, latest_receiver) = oneshot::channel();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
                Signal::new(0),
            );
            session.spawn_bootstrap(async { older_receiver.await.expect("older bootstrap") });
            session.spawn_bootstrap(async { latest_receiver.await.expect("latest bootstrap") });
            session
        });
        dom.render_immediate_to_vec();
        latest_sender.send(bootstrap_loaded(7, 50)).expect("deliver latest bootstrap");
        dom.render_immediate_to_vec();
        let latest = dom.in_scope(ScopeId::APP, || {
            let state = session.snapshot();
            assert!(state.preferences_load.is_loaded());
            assert_eq!(state.page_size(), 50);
            assert_eq!(state.feeds[0].id, 7);
            state
        });
        older_sender.send(older_result).expect("deliver older bootstrap");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || assert_eq!(session.snapshot(), latest));
    }

    #[test]
    fn older_bootstrap_success_cannot_replace_latest_preferences_or_source_mapping() {
        complete_older_bootstrap_after_latest(bootstrap_loaded(1, 100));
    }

    #[test]
    fn older_bootstrap_failure_cannot_downgrade_loaded_preferences() {
        complete_older_bootstrap_after_latest(vec![UiIntent::EntriesPage(
            EntriesPageIntent::PreferencesUnavailable("old bootstrap failed".into()),
        )]);
    }

    #[test]
    fn bootstrap_and_entry_query_generations_do_not_cancel_each_other() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (bootstrap_sender, bootstrap_receiver) = oneshot::channel();
        let (query_sender, query_receiver) = oneshot::channel();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
                Signal::new(0),
            );
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            session.spawn_bootstrap(async { bootstrap_loaded(1, 100) });
            session.spawn_bootstrap(async { bootstrap_receiver.await.expect("bootstrap") });
            session.spawn_entries_query(async { query_receiver.await.expect("query") });
            session
        });
        dom.render_immediate_to_vec();
        bootstrap_sender.send(bootstrap_loaded(7, 50)).expect("deliver bootstrap");
        dom.render_immediate_to_vec();
        query_sender.send(loaded(2)).expect("deliver query");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let state = session.snapshot();
            assert_eq!(state.page_size(), 50);
            assert_eq!(state.feeds[0].id, 7);
            assert_eq!(state.entries[0].id, 2);
        });
    }
}
