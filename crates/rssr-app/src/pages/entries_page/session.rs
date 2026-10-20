use std::future::Future;

use dioxus::prelude::*;

use super::{
    intent::EntriesPageIntent, reducer::dispatch_entries_page_intent, state::EntriesPageState,
};
use crate::ui::{
    EntriesCommand, UiCommand, UiIntent, remember_entry_controls_hidden, run_ui_command,
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
            let intents = run_ui_command(UiCommand::Entries(command)).await;
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
        self.spawn_bootstrap(run_ui_command(self.bootstrap_command()));
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
        self.dispatch(EntriesPageIntent::BootstrapStarted);
        spawn(async move {
            let intents = bootstrap.await;
            if *self.bootstrap_generation.peek() != generation {
                return;
            }
            self.dispatch(EntriesPageIntent::BootstrapFinished);
            for intent in intents.into_iter().filter_map(UiIntent::into_entries_page_intent) {
                self.dispatch(intent);
            }
        });
    }

    /// The page's read-only refresh (C4b item 2): the preferences and subscriptions again,
    /// superseding a start's read still on its way (the reducer then takes it as the start's).
    pub(crate) fn reread(self) {
        self.spawn_bootstrap(run_ui_command(UiCommand::Entries(EntriesCommand::Reread)));
    }

    /// The first run reads the page's start; later ones are its read-only refresh.
    pub(crate) fn bootstrap_or_reread(self) {
        if *self.bootstrap_generation.peek() == 0 {
            self.bootstrap();
        } else {
            self.reread();
        }
    }

    fn bootstrap_command(self) -> UiCommand {
        // Reading readiness must not subscribe the bootstrap effect to its own
        // completion and start a second feed-summary query.
        let load_preferences = !self.state.peek().preferences_load.is_loaded();
        UiCommand::Entries(EntriesCommand::Bootstrap { load_preferences, load_feeds: true })
    }

    /// Opening a subscription's page remembers it for the startup view; a failure is shown.
    pub(crate) fn remember_opened_feed(self, feed_id: i64) {
        self.spawn_ui_command(UiCommand::Entries(EntriesCommand::RememberOpenedFeed { feed_id }));
    }

    pub(crate) fn load_entries_query(self, query: EntryQuery) {
        let key = self.state.peek().position_key(self.feed_id, query.search_title.as_deref());
        let return_page = (!self.state.peek().entries_loaded)
            .then(|| crate::ui::reading_position::remembered_page(&key))
            .flatten();
        self.spawn_entries_query_with_return(
            run_ui_command(UiCommand::Entries(EntriesCommand::LoadEntries { query })),
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
        self.dispatch(EntriesPageIntent::EntriesQueryStarted);
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
            self.dispatch(EntriesPageIntent::EntriesQueryFinished);
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
        self.save_workspace(rssr_domain::EntriesWorkspaceState {
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        });
    }

    /// Saves the browsing preferences the user chose, one save at a time. Only what differs from
    /// the stored ones is sent: preferences that were read (at start or again) are the library's
    /// already, and a read never writes (C4b item 2). A choice made while a save is on its way
    /// waits for it, then is saved if it still differs, so the last choice is the one kept
    /// (review C4B-B-05).
    fn save_workspace(self, next: rssr_domain::EntriesWorkspaceState) {
        let state = self.state.peek();
        if state.workspace_saving {
            drop(state);
            self.dispatch(EntriesPageIntent::WorkspaceWanted(next));
            return;
        }
        // The stored ones, or the filters a read recovered: the effect fires for them when the
        // page becomes ready, and that is not the user's change (review C4B-B-R2-06).
        if state.stored_workspace.as_ref() == Some(&next)
            || state.recovered_edits.as_ref() == Some(&next)
        {
            return;
        }
        drop(state);
        self.dispatch(EntriesPageIntent::WorkspaceSaveStarted);
        let command = UiCommand::Entries(EntriesCommand::SaveBrowsingPreferences {
            grouping_mode: next.grouping_mode,
            show_archived: next.show_archived,
            read_filter: next.read_filter,
            starred_filter: next.starred_filter,
            selected_feed_urls: next.selected_feed_urls,
        });
        spawn(async move {
            let intents = run_ui_command(command).await;
            let unknown = intents
                .iter()
                .filter_map(UiIntent::failure)
                .any(crate::ui::failure::UiFailure::is_unknown);
            for intent in intents.into_iter().filter_map(UiIntent::into_entries_page_intent) {
                self.dispatch(intent);
            }
            if unknown {
                // The save may have been stored: a choice made meanwhile is not taken for one
                // already stored (review C4B-B-R2-04).
                self.dispatch(EntriesPageIntent::WorkspaceSaveUnknown);
            }
            // Saved or not, the save ended; a choice made meanwhile goes next.
            let wanted = self.state.peek().workspace_wanted.clone();
            self.dispatch(EntriesPageIntent::WorkspaceSaveEnded);
            if let Some(wanted) = wanted {
                self.save_workspace(wanted);
            }
        });
    }

    fn can_save_browsing_preferences(self, preferences_loaded: bool) -> bool {
        // The captured flag also matters: an effect queued before bootstrap
        // finished still contains default values, even if the live state is ready.
        preferences_loaded && self.state.peek().preferences_load.is_loaded()
    }

    /// The card's button toggles: it asks for the opposite of what the card shows.
    pub(crate) fn toggle_read(self, entry_id: i64, entry_title: String, currently_read: bool) {
        self.spawn_ui_command(UiCommand::Entries(EntriesCommand::SetRead {
            entry_id,
            entry_title,
            read: !currently_read,
        }));
    }

    pub(crate) fn toggle_starred(
        self,
        entry_id: i64,
        entry_title: String,
        currently_starred: bool,
    ) {
        self.spawn_ui_command(UiCommand::Entries(EntriesCommand::SetStarred {
            entry_id,
            entry_title,
            starred: !currently_starred,
        }));
    }

    pub(crate) fn dispatch(mut self, intent: EntriesPageIntent) {
        if let EntriesPageIntent::SetControlsHidden(hidden) = &intent {
            remember_entry_controls_hidden(*hidden);
        }
        let supersedes_query = matches!(intent, EntriesPageIntent::PatchEntryFlags { .. })
            && self.state.peek().query_in_flight;
        // A read of the preferences still on its way when a save of them landed may have read the
        // store before the save: its answer would put the saved filters back. A new read
        // replaces it (C4b item 2, R2-03).
        let supersedes_reread = matches!(intent, EntriesPageIntent::WorkspaceSaved(_))
            && self.state.peek().bootstrap_in_flight;
        dispatch_entries_page_intent(self.state, intent);
        if supersedes_query {
            // 写入成功时还在途的查询可能读的是写入前的存储：丢弃它的结果，由 reducer 递增的
            // requery_revision 触发的新查询接替（审计 A3）。
            self.query_generation.with_mut(|generation| *generation = generation.wrapping_add(1));
        }
        if supersedes_reread {
            self.reread();
        }
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
    use rssr_domain::{
        EntriesWorkspaceState, EntrySummary, FeedSummary, ReadFilter, StarredFilter, UserSettings,
    };
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
            truncated: false,
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

    /// 列表查询在已读写入提交前读取了存储；写入成功并已修补列表后，这个仍是“最新一代”的
    /// 旧快照不能把条目恢复成未读。查询代际只防“旧查询覆盖新查询”，不防“旧查询覆盖写入”。
    #[test]
    fn in_flight_query_cannot_resurrect_flags_overwritten_by_a_successful_write() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (stale_sender, stale_receiver) = oneshot::channel();
        let session = dom.in_scope(ScopeId::APP, || {
            let mut state = EntriesPageState::new(true);
            state.read_filter = ReadFilter::UnreadOnly;
            let session =
                EntriesPageSession::new(None, Signal::new(state), Signal::new(0), Signal::new(0));
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            session.spawn_entries_query(async { loaded(1) });
            session
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().entries.len(), 1);
            // 例如：跨秒重渲染或刷新完成触发了新查询，它先于下面的写入读到“未读”。
            session
                .spawn_entries_query(async { stale_receiver.await.expect("stale query result") });
            // SetRead 成功后 runtime 投递的修补。
            session.dispatch(EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: Some(true),
                is_starred: None,
            });
            assert!(session.snapshot().entries.is_empty(), "read entry leaves UnreadOnly");
        });

        stale_sender.send(loaded(1)).expect("deliver stale unread snapshot");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert!(
                session.snapshot().entries.iter().all(|entry| entry.id != 1 || entry.is_read),
                "a stale snapshot resurrected an entry the user just marked as read"
            );
        });
    }

    /// 切到 StarredOnly 的查询还在途时收藏了仍显示的条目：那次查询读的是写入前的存储，
    /// 它的空结果不能抹掉这个条目；被作废的查询由新查询接替，带回它（独立评审的反例）。
    #[test]
    fn a_write_during_a_query_supersedes_it_even_when_the_entry_newly_matches() {
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let (stale_sender, stale_receiver) = oneshot::channel();
        let (fresh_sender, fresh_receiver) = oneshot::channel();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
                Signal::new(0),
            );
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            session.spawn_entries_query(async { loaded(1) });
            session
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            session.dispatch(EntriesPageIntent::SetStarredFilter(StarredFilter::StarredOnly));
            session.spawn_entries_query(async { stale_receiver.await.expect("stale result") });
            session.dispatch(EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: None,
                is_starred: Some(true),
            });
            assert_eq!(session.snapshot().requery_revision, 1, "the write asks for a new query");
        });

        stale_sender
            .send(vec![UiIntent::EntriesPage(EntriesPageIntent::SetEntries {
                entries: Vec::new(),
                archived_count: 0,
                truncated: false,
            })])
            .expect("deliver the stale empty result");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert!(
                session.snapshot().entries.iter().any(|entry| entry.id == 1 && entry.is_starred),
                "the superseded empty result must not wipe the newly starred entry"
            );
            // 页面对 requery_revision 的反应：重新发出查询。
            session.spawn_entries_query(async { fresh_receiver.await.expect("fresh result") });
        });
        let mut fresh = loaded(1);
        if let UiIntent::EntriesPage(EntriesPageIntent::SetEntries { entries, .. }) = &mut fresh[0]
        {
            entries[0].is_starred = true;
        }
        fresh_sender.send(fresh).expect("deliver the fresh result");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let snapshot = session.snapshot();
            let flags = snapshot.entries.iter().map(|e| (e.id, e.is_starred)).collect::<Vec<_>>();
            assert_eq!(flags, [(1, true)]);
            assert!(!snapshot.query_in_flight);
        });
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
                    assert_eq!(query.feed_ids, Some(vec![7]));
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
            session.dispatch(EntriesPageIntent::PreferencesUnavailable(crate::ui::failure::plain(
                "broken preferences",
            )));
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
            EntriesPageIntent::PreferencesUnavailable(crate::ui::failure::plain(
                "old bootstrap failed",
            )),
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

    /// The browsing preferences are saved only when they differ from the stored ones: the values
    /// a read applied (at start or again) are not written back, and neither are ones just saved
    /// (C4b item 2: a read-only refresh writes nothing).
    #[tokio::test(flavor = "current_thread")]
    async fn only_preferences_the_user_changed_are_saved() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let stored = EntriesWorkspaceState::default();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
                Signal::new(0),
            );
            session.dispatch(EntriesPageIntent::ApplyLoadedWorkspaceState(stored.clone()));
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            session
        });
        let save = |workspace: &EntriesWorkspaceState| {
            session.save_browsing_preferences_with(
                true,
                workspace.grouping_mode,
                workspace.show_archived,
                workspace.read_filter,
                workspace.starred_filter,
                workspace.selected_feed_urls.clone(),
            )
        };
        dom.in_scope(ScopeId::APP, || save(&stored));
        assert_eq!(backend.calls("save_entries_workspace_if_changed"), 0, "as read");

        let changed =
            EntriesWorkspaceState { read_filter: ReadFilter::UnreadOnly, ..stored.clone() };
        dom.in_scope(ScopeId::APP, || save(&changed));
        for _ in 0..100 {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(20), dom.wait_for_work())
                .await;
            if dom.in_scope(ScopeId::APP, || session.snapshot().stored_workspace)
                == Some(changed.clone())
            {
                break;
            }
        }
        assert_eq!(backend.calls("save_entries_workspace_if_changed"), 1);
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().stored_workspace, Some(changed.clone()));
            save(&changed);
        });
        assert_eq!(backend.calls("save_entries_workspace_if_changed"), 1, "as saved");
    }

    /// A re-read issued before a save of the preferences landed may have read the store before
    /// the save: its answer does not put the saved filters back. A new read replaces it, and the
    /// page keeps what was saved (C4b item 2, R2-03; advisor's trace).
    #[tokio::test(flavor = "current_thread")]
    async fn a_reread_issued_before_a_save_landed_does_not_undo_it() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let before = EntriesWorkspaceState::default();
        let saved = EntriesWorkspaceState { read_filter: ReadFilter::UnreadOnly, ..before.clone() };
        let (sender, receiver) = oneshot::channel::<Vec<UiIntent>>();
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
                Signal::new(0),
            );
            session.dispatch(EntriesPageIntent::ApplyLoadedWorkspaceState(before.clone()));
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            // A focus: the re-read reads the store before the save below.
            session.spawn_bootstrap(async { receiver.await.expect("the early re-read") });
            session.dispatch(EntriesPageIntent::SetReadFilter(ReadFilter::UnreadOnly));
            session.save_browsing_preferences_with(
                true,
                saved.grouping_mode,
                saved.show_archived,
                saved.read_filter,
                saved.starred_filter,
                saved.selected_feed_urls.clone(),
            );
            session
        });
        for _ in 0..100 {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(20), dom.wait_for_work())
                .await;
            if backend.calls("bootstrap_entries") == 1
                && !dom.in_scope(ScopeId::APP, || session.snapshot().bootstrap_in_flight)
            {
                break;
            }
        }
        assert_eq!(backend.calls("save_entries_workspace_if_changed"), 1);
        assert_eq!(backend.calls("bootstrap_entries"), 1, "a new read after the save");

        // The early re-read answers with what it read before the save.
        sender
            .send(vec![UiIntent::EntriesPage(EntriesPageIntent::PreferencesReread {
                settings: UserSettings::default(),
                workspace: before,
            })])
            .expect("deliver");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let state = session.snapshot();
            assert_eq!(state.read_filter, ReadFilter::UnreadOnly, "the saved filter stays");
            assert_eq!(state.stored_workspace, Some(saved));
        });
    }

    /// A choice made while a save of the preferences is on its way is saved after it, even when
    /// it goes back to what was stored before: the last choice is the one kept (review
    /// C4B-B-05).
    #[tokio::test(flavor = "current_thread")]
    async fn a_choice_made_during_a_save_is_saved_after_it() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

        let backend = Scripted::over_new_library();
        let hold = backend.hold("save_entries_workspace_if_changed");
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let first = EntriesWorkspaceState::default();
        let second = EntriesWorkspaceState { read_filter: ReadFilter::UnreadOnly, ..first.clone() };
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
                Signal::new(0),
            );
            session.dispatch(EntriesPageIntent::ApplyLoadedWorkspaceState(first.clone()));
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            session
        });
        let save = |workspace: &EntriesWorkspaceState| {
            dom.in_scope(ScopeId::APP, || {
                session.save_browsing_preferences_with(
                    true,
                    workspace.grouping_mode,
                    workspace.show_archived,
                    workspace.read_filter,
                    workspace.starred_filter,
                    workspace.selected_feed_urls.clone(),
                )
            })
        };
        save(&second);
        save(&first);
        for _ in 0..100 {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(20), dom.wait_for_work())
                .await;
            if hold.waiting() == 1 {
                break;
            }
        }
        assert_eq!(backend.calls("save_entries_workspace_if_changed"), 1, "one at a time");
        for _ in 0..2 {
            hold.release();
        }
        for _ in 0..100 {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(20), dom.wait_for_work())
                .await;
            if dom.in_scope(ScopeId::APP, || {
                let state = session.snapshot();
                !state.workspace_saving && backend.calls("save_entries_workspace_if_changed") == 2
            }) {
                break;
            }
        }
        assert_eq!(backend.calls("save_entries_workspace_if_changed"), 2);
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().stored_workspace, Some(first.clone()), "the last choice");
        });
        let stored = rssr_api::Backend::bootstrap_entries(
            &*backend,
            rssr_application::EntriesBootstrapInput { load_preferences: true, load_feeds: false },
        )
        .await
        .expect("read")
        .workspace;
        assert_eq!(stored, Some(first), "the library has it");
    }

    /// A page whose preferences were read as `stored`, with the scripted backend.
    fn page_with_preferences(
        dom: &mut VirtualDom,
        stored: &EntriesWorkspaceState,
    ) -> EntriesPageSession {
        dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
                Signal::new(0),
            );
            session.dispatch(EntriesPageIntent::ApplyLoadedWorkspaceState(stored.clone()));
            session.dispatch(EntriesPageIntent::PreferencesLoaded);
            session
        })
    }

    /// What the page's save effect does for `workspace` (the filters it shows).
    fn choose(
        dom: &mut VirtualDom,
        session: EntriesPageSession,
        workspace: &EntriesWorkspaceState,
    ) {
        dom.in_scope(ScopeId::APP, || {
            session.dispatch(EntriesPageIntent::SetReadFilter(workspace.read_filter));
            session.save_browsing_preferences_with(
                true,
                workspace.grouping_mode,
                workspace.show_archived,
                workspace.read_filter,
                workspace.starred_filter,
                workspace.selected_feed_urls.clone(),
            )
        })
    }

    async fn until_saves_end(
        dom: &mut VirtualDom,
        session: EntriesPageSession,
        calls: usize,
        backend: &Scripted,
    ) {
        for _ in 0..150 {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(20), dom.wait_for_work())
                .await;
            if backend.calls("save_entries_workspace_if_changed") == calls
                && !dom.in_scope(ScopeId::APP, || session.snapshot().workspace_saving)
            {
                return;
            }
        }
        panic!("the saves did not end");
    }

    use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

    /// A re-read that answers while a save is on its way and the user's next choice waits does
    /// not replace that choice, and nothing it read is saved (review C4B-B-R2-03).
    #[tokio::test(flavor = "current_thread")]
    async fn a_reread_during_a_save_leaves_the_users_choice() {
        let backend = Scripted::over_new_library();
        let hold = backend.hold("save_entries_workspace_if_changed");
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let a = EntriesWorkspaceState::default();
        let b = EntriesWorkspaceState { read_filter: ReadFilter::UnreadOnly, ..a.clone() };
        let c = EntriesWorkspaceState { read_filter: ReadFilter::ReadOnly, ..a.clone() };
        let session = page_with_preferences(&mut dom, &a);
        choose(&mut dom, session, &b);
        choose(&mut dom, session, &a);
        // Another browser stored C; a focus re-read brings it while B's save is on its way.
        dom.in_scope(ScopeId::APP, || {
            session.spawn_bootstrap(async move {
                vec![UiIntent::EntriesPage(EntriesPageIntent::PreferencesReread {
                    settings: UserSettings::default(),
                    workspace: c,
                })]
            })
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let state = session.snapshot();
            assert_eq!(state.workspace(), a, "the user's choice stays shown");
            assert_eq!(state.workspace_wanted, Some(a.clone()), "and waits");
        });
        for _ in 0..2 {
            hold.release();
        }
        until_saves_end(&mut dom, session, 2, &backend).await;
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().stored_workspace, Some(a.clone()), "the last choice");
        });
    }

    /// A save whose outcome is unknown may have been stored: the user's next choice, made while
    /// it was on its way, is still saved after it (review C4B-B-R2-04).
    #[tokio::test(flavor = "current_thread")]
    async fn a_choice_after_a_save_of_unknown_outcome_is_saved() {
        use rssr_api::{BackendError, ErrorCode};

        let backend = Scripted::over_new_library();
        let hold = backend.hold("save_entries_workspace_if_changed");
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let a = EntriesWorkspaceState::default();
        let b = EntriesWorkspaceState { read_filter: ReadFilter::UnreadOnly, ..a.clone() };
        let session = page_with_preferences(&mut dom, &a);
        choose(&mut dom, session, &b);
        choose(&mut dom, session, &a);
        backend.fail_once(
            "save_entries_workspace_if_changed",
            BackendError::new(ErrorCode::OutcomeUnknown, "结果未知"),
        );
        for _ in 0..2 {
            hold.release();
        }
        until_saves_end(&mut dom, session, 2, &backend).await;
        dom.in_scope(ScopeId::APP, || {
            let state = session.snapshot();
            assert_eq!(state.stored_workspace, Some(a.clone()), "the choice was saved after it");
            assert_eq!(state.workspace(), a);
        });
    }

    /// After the preferences could not be read and the user chose filters, a read that recovers
    /// them makes the page ready, keeps the user's filters and saves nothing by itself; the
    /// user's next change is saved (review C4B-B-R2-06).
    #[tokio::test(flavor = "current_thread")]
    async fn a_recovering_read_keeps_the_users_filters_without_saving_them() {
        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(|| rsx! {});
        dom.rebuild_in_place();
        let fallback = EntriesWorkspaceState::default();
        let chosen =
            EntriesWorkspaceState { read_filter: ReadFilter::UnreadOnly, ..fallback.clone() };
        let session = dom.in_scope(ScopeId::APP, || {
            let session = EntriesPageSession::new(
                None,
                Signal::new(EntriesPageState::new(true)),
                Signal::new(0),
                Signal::new(0),
            );
            session.dispatch(EntriesPageIntent::PreferencesUnavailable(crate::ui::failure::plain(
                "读不到",
            )));
            session
        });
        choose(&mut dom, session, &chosen);
        dom.in_scope(ScopeId::APP, || {
            session.dispatch(EntriesPageIntent::PreferencesReread {
                settings: UserSettings::default(),
                workspace: fallback.clone(),
            });
            assert!(session.snapshot().preferences_load.is_loaded());
            assert_eq!(session.snapshot().workspace(), chosen);
        });
        // The save effect fires when the page becomes ready: not the user's change.
        choose(&mut dom, session, &chosen);
        assert_eq!(backend.calls("save_entries_workspace_if_changed"), 0, "the read saved nothing");

        let next = EntriesWorkspaceState { read_filter: ReadFilter::ReadOnly, ..fallback };
        choose(&mut dom, session, &next);
        until_saves_end(&mut dom, session, 1, &backend).await;
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().stored_workspace, Some(next.clone()));
        });
    }
}
