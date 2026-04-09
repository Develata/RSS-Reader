use dioxus::prelude::*;

use super::{
    bindings::EntriesPageBindings, controls::remember_entry_controls_hidden,
    effect::EntriesPageEffect, intent::EntriesPageIntent, presenter::EntriesPagePresenter,
    reducer::dispatch_entries_page_intent, runtime::execute_entries_page_effect,
    state::EntriesPageState,
};
use rssr_domain::EntryQuery;
use time::OffsetDateTime;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct EntriesPageSession {
    feed_id: Option<i64>,
    state: Signal<EntriesPageState>,
    bindings: EntriesPageBindings,
}

impl EntriesPageSession {
    pub(crate) fn new(feed_id: Option<i64>, state: Signal<EntriesPageState>) -> Self {
        Self { feed_id, state, bindings: EntriesPageBindings::new(state) }
    }

    pub(crate) fn snapshot(self) -> EntriesPageState {
        (self.state)()
    }

    pub(crate) fn reload_tick(self) -> u64 {
        (self.state)().reload_tick
    }

    pub(crate) fn presenter(self, now: OffsetDateTime) -> EntriesPagePresenter {
        EntriesPagePresenter::from_state(&self.snapshot(), self.feed_id, now)
    }

    pub(crate) fn remember_last_opened_feed(self) {
        if let Some(feed_id) = self.feed_id {
            self.spawn_effect(EntriesPageEffect::RememberLastOpenedFeed(feed_id));
        }
    }

    pub(crate) fn load_preferences(self) {
        self.spawn_effect(EntriesPageEffect::LoadPreferences);
    }

    pub(crate) fn load_feeds(self) {
        self.spawn_effect(EntriesPageEffect::LoadFeeds);
    }

    pub(crate) fn load_entries_query(self, query: EntryQuery) {
        self.spawn_effect(EntriesPageEffect::LoadEntries(query));
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

        self.spawn_effect(EntriesPageEffect::SaveBrowsingPreferences {
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        });
    }

    pub(crate) fn toggle_read(self, entry_id: i64, entry_title: String, currently_read: bool) {
        self.spawn_effect(EntriesPageEffect::ToggleRead { entry_id, entry_title, currently_read });
    }

    pub(crate) fn toggle_starred(
        self,
        entry_id: i64,
        entry_title: String,
        currently_starred: bool,
    ) {
        self.spawn_effect(EntriesPageEffect::ToggleStarred {
            entry_id,
            entry_title,
            currently_starred,
        });
    }

    pub(crate) fn dispatch(self, intent: EntriesPageIntent) {
        if let EntriesPageIntent::SetControlsHidden(hidden) = &intent {
            remember_entry_controls_hidden(*hidden);
        }
        dispatch_entries_page_intent(self.state, intent);
    }

    fn spawn_effect(self, effect: EntriesPageEffect) {
        spawn(async move {
            let outcome = execute_entries_page_effect(effect).await;
            self.apply_runtime_outcome(outcome);
        });
    }

    fn apply_runtime_outcome(self, outcome: super::runtime::EntriesPageRuntimeOutcome) {
        self.bindings.apply_runtime_outcome(outcome);
    }
}
