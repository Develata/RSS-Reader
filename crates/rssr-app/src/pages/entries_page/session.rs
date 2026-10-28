use std::future::Future;

use dioxus::prelude::*;

use super::{
    intent::EntriesPageIntent,
    preferences_policy,
    reducer::dispatch_entries_page_intent,
    request_lifecycle::EntriesRequestLifecycle,
    state::EntriesPageState,
    write_lifecycle::{BulkWrite, EntryWrite, WriteEffect},
};
use crate::ui::{EntriesCommand, UiCommand, UiIntent, execute_ui_command};
use rssr_domain::{EntriesWorkspaceState, EntryQuery};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct EntriesPageSession {
    feed_id: Option<i64>,
    state: Signal<EntriesPageState>,
    requests: Signal<EntriesRequestLifecycle>,
}

impl EntriesPageSession {
    pub(crate) fn apply_selection(self, action: rssr_domain::EntryBatchAction) {
        let request = self.state.with_peek(|state| BulkWrite::selection(state, action));
        if let Some(request) = request {
            self.spawn_bulk(true, move || {
                execute_ui_command(UiCommand::Entries(EntriesCommand::ApplySelection { request }))
            });
        }
    }
    fn spawn_bulk<F: Future<Output = Vec<UiIntent>> + 'static>(
        self,
        writing: bool,
        write: impl FnOnce() -> F + 'static,
    ) {
        if !self.state.with_peek(BulkWrite::can_start) {
            return;
        }
        self.dispatch(EntriesPageIntent::BeginBulk { writing });
        let request = self.state.with_peek(BulkWrite::capture_started);
        spawn(async move {
            let intents = write().await.into_iter().filter_map(UiIntent::into_entries_page_intent);
            let effects = self.state.with_peek(|state| request.complete(state, intents));
            self.apply_write_effects(effects);
        });
    }

    pub(crate) fn new(
        feed_id: Option<i64>,
        state: Signal<EntriesPageState>,
        requests: Signal<EntriesRequestLifecycle>,
    ) -> Self {
        Self { feed_id, state, requests }
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
        self.spawn_bootstrap(|load_preferences| {
            execute_ui_command(UiCommand::Entries(EntriesCommand::Bootstrap {
                feed_id: self.feed_id,
                load_preferences,
                load_feeds: true,
            }))
        });
    }

    fn spawn_bootstrap<F: Future<Output = Vec<UiIntent>> + 'static>(
        mut self,
        bootstrap: impl FnOnce(bool) -> F,
    ) {
        // Peek: initialization must not subscribe to its own readiness changes.
        let request = self
            .state
            .with_peek(|state| self.requests.with_mut(|requests| requests.begin_bootstrap(state)));
        let bootstrap = bootstrap(request.load_preferences);
        spawn(async move {
            let intents = bootstrap.await;
            if !self.requests.peek().accepts_bootstrap(request) {
                return;
            }
            for intent in intents.into_iter().filter_map(UiIntent::into_entries_page_intent) {
                self.dispatch(intent);
            }
        });
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
        let request = self.state.with_peek(|state| {
            self.requests.with_mut(|requests| requests.begin_query(state, return_page))
        });
        let Some(request) = request else {
            return;
        };
        spawn(async move {
            let intents = query.await;
            let completion =
                self.state.with_peek(|state| self.requests.peek().complete_query(request, state));
            for intent in completion
                .intents(intents.into_iter().filter_map(UiIntent::into_entries_page_intent))
            {
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
        if !self.can_save_browsing_preferences(preferences_loaded) {
            return;
        }

        self.spawn_preferences_write(
            EntriesWorkspaceState {
                grouping_mode,
                show_archived,
                read_filter,
                starred_filter,
                selected_feed_urls,
            },
            |next| {
                execute_ui_command(UiCommand::Entries(EntriesCommand::SaveBrowsingPreferences {
                    grouping_mode: next.grouping_mode,
                    show_archived: next.show_archived,
                    read_filter: next.read_filter,
                    starred_filter: next.starred_filter,
                    selected_feed_urls: next.selected_feed_urls,
                }))
            },
        );
    }

    fn can_save_browsing_preferences(self, preferences_loaded: bool) -> bool {
        // The captured flag also matters: an effect queued before bootstrap
        // finished still contains default values, even if the live state is ready.
        self.state
            .with_peek(|state| preferences_policy::can_schedule_save(state, preferences_loaded))
    }

    pub(crate) fn toggle_read(self, entry_id: i64, entry_title: String, currently_read: bool) {
        self.spawn_entry_write(entry_id, move || {
            execute_ui_command(UiCommand::Entries(EntriesCommand::ToggleRead {
                entry_id,
                entry_title,
                currently_read,
            }))
        });
    }

    pub(crate) fn toggle_starred(
        self,
        entry_id: i64,
        entry_title: String,
        currently_starred: bool,
    ) {
        self.spawn_entry_write(entry_id, move || {
            execute_ui_command(UiCommand::Entries(EntriesCommand::ToggleStarred {
                entry_id,
                entry_title,
                currently_starred,
            }))
        });
    }

    fn spawn_entry_write<F: Future<Output = Vec<UiIntent>> + 'static>(
        self,
        entry_id: i64,
        write: impl FnOnce() -> F + 'static,
    ) {
        // Check live Rust state as well as disabling buttons: a second event
        // can already be queued before the DOM has rendered the pending state.
        let Some(request) = self.state.with_peek(|state| EntryWrite::prepare(state, entry_id))
        else {
            return;
        };
        self.dispatch(request.started());
        spawn(async move {
            let intents = write().await.into_iter().filter_map(UiIntent::into_entries_page_intent);
            self.apply_write_effects(request.complete(intents));
        });
    }

    fn apply_write_effects(self, effects: impl Iterator<Item = WriteEffect>) {
        for effect in effects {
            match effect {
                WriteEffect::Publish(intent) => self.dispatch(intent),
                WriteEffect::RefreshSources => self.bootstrap(),
            }
        }
    }

    pub(crate) fn dispatch(self, intent: EntriesPageIntent) {
        dispatch_entries_page_intent(self.state, intent);
    }

    fn spawn_preferences_write<F: Future<Output = Vec<UiIntent>> + 'static>(
        self,
        next: EntriesWorkspaceState,
        write: impl FnOnce(EntriesWorkspaceState) -> F + 'static,
    ) {
        spawn(async move {
            // An effect's captured choice can be obsolete by the time its task
            // starts. Do not enqueue that older choice behind the current save.
            let current =
                self.state.with_peek(|state| preferences_policy::is_current_save(state, &next));
            if !current {
                return;
            }
            for intent in
                write(next).await.into_iter().filter_map(UiIntent::into_entries_page_intent)
            {
                self.dispatch(intent);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rssr_domain::{EntriesWorkspaceState, EntrySummary, FeedSummary, ReadFilter, UserSettings};
    use std::{cell::Cell, rc::Rc};
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

    #[test]
    fn queued_preferences_skip_choices_replaced_before_the_task_starts() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let saved = Rc::new(std::cell::RefCell::new(Vec::new()));
        dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new()),
                Signal::new(EntriesRequestLifecycle::default()),
            );
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            for read_filter in [ReadFilter::UnreadOnly, ReadFilter::All] {
                session.dispatch(EntriesPageIntent::SetReadFilter(read_filter));
                let saved = Rc::clone(&saved);
                session.spawn_preferences_write(
                    EntriesWorkspaceState { read_filter, ..Default::default() },
                    move |next| {
                        saved.borrow_mut().push(next.read_filter);
                        async { Vec::new() }
                    },
                );
            }
        });
        dom.render_immediate_to_vec();
        assert_eq!(*saved.borrow(), [ReadFilter::All]);
    }

    #[test]
    fn entry_writes_reject_overlap_and_unlock_after_success_or_failure() {
        for failed in [false, true] {
            let mut dom = VirtualDom::new(|| rsx! {});
            dom.rebuild_in_place();
            let calls = Rc::new(Cell::new(0));
            let (sender, receiver) = oneshot::channel();
            let session = dom.in_scope(ScopeId::APP, || {
                let session = EntriesPageSession::new(
                    None,
                    Signal::new(EntriesPageState::new()),
                    Signal::new(EntriesRequestLifecycle::default()),
                );
                for intent in loaded(1).into_iter().filter_map(UiIntent::into_entries_page_intent) {
                    session.dispatch(intent);
                }
                session.dispatch(EntriesPageIntent::ToggleSelected(1));
                let first = Rc::clone(&calls);
                session.spawn_entry_write(1, move || {
                    first.set(first.get() + 1);
                    async { receiver.await.unwrap() }
                });
                let duplicate = Rc::clone(&calls);
                session.spawn_entry_write(1, move || {
                    duplicate.set(duplicate.get() + 1);
                    async { Vec::new() }
                });
                // This must be rejected before executing a real batch command.
                session.apply_selection(rssr_domain::EntryBatchAction::Archive);
                assert!(!session.snapshot().bulk_busy);
                session
            });
            dom.render_immediate_to_vec();
            assert_eq!(calls.get(), 1, "a repeated event cannot start a second write");
            dom.in_scope(ScopeId::APP, || {
                assert!(session.snapshot().pending_entry_writes.contains(&1))
            });
            sender
                .send(if failed {
                    failure()
                } else {
                    vec![UiIntent::EntriesPage(EntriesPageIntent::PatchEntryFlags {
                        entry_id: 1,
                        is_read: None,
                        is_starred: Some(true),
                    })]
                })
                .unwrap();
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                assert!(session.snapshot().pending_entry_writes.is_empty());
                let retry = Rc::clone(&calls);
                session.spawn_entry_write(1, move || {
                    retry.set(retry.get() + 1);
                    async { Vec::new() }
                });
            });
            dom.render_immediate_to_vec();
            assert_eq!(calls.get(), 2, "both success and failure allow the next attempt");
            dom.in_scope(ScopeId::APP, || {
                session.dispatch(EntriesPageIntent::BeginBulk { writing: true });
                let overlap = Rc::clone(&calls);
                session.spawn_entry_write(1, move || {
                    overlap.set(overlap.get() + 1);
                    async { Vec::new() }
                });
            });
            dom.render_immediate_to_vec();
            assert_eq!(calls.get(), 2, "a bulk write also blocks new per-entry writes");
        }
    }

    #[test]
    fn query_cannot_restore_flags_or_rows_from_before_a_completed_write() {
        for (unread_only, failed) in [(false, false), (true, false), (false, true)] {
            let mut dom = VirtualDom::new(|| rsx! {});
            dom.rebuild_in_place();
            let (sender, receiver) = oneshot::channel();
            let session = dom.in_scope(ScopeId::APP, || {
                let session = EntriesPageSession::new(
                    None,
                    Signal::new(EntriesPageState::new()),
                    Signal::new(EntriesRequestLifecycle::default()),
                );
                session.dispatch(EntriesPageIntent::PreferencesLoaded);
                if unread_only {
                    session.dispatch(EntriesPageIntent::SetReadFilter(ReadFilter::UnreadOnly));
                }
                for intent in loaded(1).into_iter().filter_map(UiIntent::into_entries_page_intent) {
                    session.dispatch(intent);
                }
                session.dispatch(EntriesPageIntent::ToggleSelected(1));
                session.spawn_entries_query(async { receiver.await.unwrap() });
                session
            });
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                session.dispatch(EntriesPageIntent::PatchEntryFlags {
                    entry_id: 1,
                    is_read: Some(true),
                    is_starred: Some(true),
                })
            });
            sender.send(if failed { failure() } else { loaded(1) }).unwrap();
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                let state = session.snapshot();
                assert_eq!(state.reload_revision, 1, "discarded results schedule a fresh query");
                assert_ne!(state.status, "old query failed");
                if unread_only {
                    assert!(state.entries.is_empty(), "old query cannot reinsert the now-read row");
                } else {
                    assert!(
                        state.entries[0].is_read && state.entries[0].is_starred,
                        "old query cannot undo confirmed flags"
                    );
                    assert!(state.selected_entry_ids.contains(&1));
                }
            });
            dom.in_scope(ScopeId::APP, || session.spawn_entries_query(async { loaded(2) }));
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                assert_eq!(session.snapshot().entries[0].id, 2, "the next query still publishes");
            });
        }
    }

    fn complete_older_query_after_latest(older_result: Vec<UiIntent>) -> EntriesPageState {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (older_sender, older_receiver) = oneshot::channel();
        let (latest_sender, latest_receiver) = oneshot::channel();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new()),
                Signal::new(EntriesRequestLifecycle::default()),
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
            let mut state = EntriesPageState::new();
            state.entries_page_size = 1;
            let session = EntriesPageSession::new(
                None,
                Signal::new(state),
                Signal::new(EntriesRequestLifecycle::default()),
            );
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
                Signal::new(EntriesPageState::new()),
                Signal::new(EntriesRequestLifecycle::default()),
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
                    let query = state.entry_query_scope(None, None);
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
                Signal::new(EntriesPageState::new()),
                Signal::new(EntriesRequestLifecycle::default()),
            );
            session
                .dispatch(EntriesPageIntent::PreferencesUnavailable("broken preferences".into()));
            assert!(!session.can_save_browsing_preferences(true));
            session.spawn_bootstrap(|load_preferences| {
                assert!(load_preferences, "unavailable preferences are retried");
                async { Vec::new() }
            });
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
            session.spawn_bootstrap(|load_preferences| {
                assert!(!load_preferences, "loaded preferences are retained");
                async { Vec::new() }
            });
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
                Signal::new(EntriesPageState::new()),
                Signal::new(EntriesRequestLifecycle::default()),
            );
            session.spawn_bootstrap(|_| async { older_receiver.await.expect("older bootstrap") });
            session.spawn_bootstrap(|_| async { latest_receiver.await.expect("latest bootstrap") });
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
                Signal::new(EntriesPageState::new()),
                Signal::new(EntriesRequestLifecycle::default()),
            );
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            session.spawn_bootstrap(|_| async { bootstrap_loaded(1, 100) });
            session.spawn_bootstrap(|_| async { bootstrap_receiver.await.expect("bootstrap") });
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

    #[test]
    fn delayed_query_does_not_duplicate_a_bulk_reload_or_restore_old_rows() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (sender, receiver) = oneshot::channel();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new()),
                Signal::new(EntriesRequestLifecycle::default()),
            );
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            for intent in loaded(1).into_iter().filter_map(UiIntent::into_entries_page_intent) {
                session.dispatch(intent);
            }
            session.spawn_entries_query_with_return(async { receiver.await.unwrap() }, Some(9));
            session
        });
        dom.render_immediate_to_vec();
        let before = dom.in_scope(ScopeId::APP, || {
            session.dispatch(EntriesPageIntent::SelectionApplied {
                ids: vec![1],
                action: rssr_domain::EntryBatchAction::Archive,
                count: 1,
            });
            session.snapshot()
        });
        sender.send(loaded(1)).unwrap();
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || assert_eq!(session.snapshot(), before));
    }

    #[test]
    fn page_disposal_cancels_pending_bootstrap_and_query_tasks() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (bootstrap_sender, bootstrap_receiver) = oneshot::channel();
        let (query_sender, query_receiver) = oneshot::channel();
        dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new()),
                Signal::new(EntriesRequestLifecycle::default()),
            );
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            session.spawn_bootstrap(|_| async { bootstrap_receiver.await.unwrap() });
            session.spawn_entries_query(async { query_receiver.await.unwrap() });
        });
        dom.render_immediate_to_vec();
        drop(dom);
        assert!(bootstrap_sender.send(bootstrap_loaded(7, 50)).is_err());
        assert!(query_sender.send(loaded(1)).is_err());
    }

    #[test]
    fn bulk_failure_releases_the_synchronous_guard_and_preserves_retry_selection() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (sender, receiver) = oneshot::channel();
        let calls = Rc::new(Cell::new(0));
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new()),
                Signal::new(EntriesRequestLifecycle::default()),
            );
            for intent in loaded(1).into_iter().filter_map(UiIntent::into_entries_page_intent) {
                session.dispatch(intent);
            }
            session.dispatch(EntriesPageIntent::ToggleSelected(1));
            let first = Rc::clone(&calls);
            session.spawn_bulk(true, move || {
                first.set(first.get() + 1);
                async { receiver.await.unwrap() }
            });
            let duplicate = Rc::clone(&calls);
            session.spawn_bulk(true, move || {
                duplicate.set(duplicate.get() + 1);
                async { Vec::new() }
            });
            let overlap = Rc::clone(&calls);
            session.spawn_entry_write(1, move || {
                overlap.set(overlap.get() + 1);
                async { Vec::new() }
            });
            assert!(session.snapshot().bulk_busy);
            session
        });
        dom.render_immediate_to_vec();
        assert_eq!(calls.get(), 1);
        sender
            .send(vec![UiIntent::EntriesPage(EntriesPageIntent::BulkFailed("failed".into()))])
            .unwrap();
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert!(!session.snapshot().bulk_busy);
            assert!(session.snapshot().selected_entry_ids.contains(&1));
            let retry = Rc::clone(&calls);
            session.spawn_bulk(true, move || {
                retry.set(retry.get() + 1);
                async {
                    vec![UiIntent::EntriesPage(EntriesPageIntent::BulkFailed(
                        "retry failed".into(),
                    ))]
                }
            });
        });
        dom.render_immediate_to_vec();
        assert_eq!(calls.get(), 2);
        dom.in_scope(ScopeId::APP, || assert_eq!(session.snapshot().status, "retry failed"));
    }

    #[test]
    fn retired_bulk_completion_does_not_publish_into_the_current_batch() {
        for failed in [false, true] {
            let mut dom = VirtualDom::new(|| rsx! {});
            dom.rebuild_in_place();
            let (old_sender, old_receiver) = oneshot::channel();
            let (new_sender, new_receiver) = oneshot::channel();
            let session = dom.in_scope(ScopeId::APP, || {
                let session = EntriesPageSession::new(
                    None,
                    Signal::new(EntriesPageState::new()),
                    Signal::new(EntriesRequestLifecycle::default()),
                );
                session.spawn_bulk(false, || async { old_receiver.await.unwrap() });
                session.dispatch(EntriesPageIntent::CancelBulk);
                session.spawn_bulk(true, || async { new_receiver.await.unwrap() });
                session
            });
            dom.render_immediate_to_vec();
            let before = dom.in_scope(ScopeId::APP, || session.snapshot());
            let result = if failed {
                EntriesPageIntent::BulkFailed("stale".into())
            } else {
                EntriesPageIntent::SelectionApplied {
                    ids: vec![1],
                    action: rssr_domain::EntryBatchAction::Archive,
                    count: 1,
                }
            };
            old_sender.send(vec![UiIntent::EntriesPage(result)]).unwrap();
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || assert_eq!(session.snapshot(), before));
            new_sender
                .send(vec![UiIntent::EntriesPage(EntriesPageIntent::BulkFailed("current".into()))])
                .unwrap();
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                assert!(!session.snapshot().bulk_busy);
                assert_eq!(session.snapshot().status, "current");
            });
        }
    }

    #[test]
    fn queued_preference_save_rechecks_readiness_before_executing() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let calls = Rc::new(Cell::new(0));
        dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new()),
                Signal::new(EntriesRequestLifecycle::default()),
            );
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            let started = Rc::clone(&calls);
            session.spawn_preferences_write(EntriesWorkspaceState::default(), move |_| {
                started.set(started.get() + 1);
                async { Vec::new() }
            });
            session.dispatch(EntriesPageIntent::PreferencesUnavailable("failed".into()));
        });
        dom.render_immediate_to_vec();
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn page_disposal_cancels_pending_row_batch_and_preference_tasks() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (row_sender, row_receiver) = oneshot::channel();
        let (batch_sender, batch_receiver) = oneshot::channel();
        let (preferences_sender, preferences_receiver) = oneshot::channel();
        dom.in_scope(ScopeId::APP, || {
            let session = || {
                EntriesPageSession::new(
                    None,
                    Signal::new(EntriesPageState::new()),
                    Signal::new(EntriesRequestLifecycle::default()),
                )
            };
            session().spawn_entry_write(1, || async { row_receiver.await.unwrap() });
            session().spawn_bulk(true, || async { batch_receiver.await.unwrap() });
            let preferences = session();
            preferences.dispatch(EntriesPageIntent::PreferencesLoaded);
            preferences.spawn_preferences_write(EntriesWorkspaceState::default(), |_| async {
                preferences_receiver.await.unwrap()
            });
        });
        dom.render_immediate_to_vec();
        drop(dom);
        assert!(row_sender.send(Vec::new()).is_err());
        assert!(batch_sender.send(Vec::new()).is_err());
        assert!(preferences_sender.send(Vec::new()).is_err());
    }
}
