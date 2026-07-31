use dioxus::prelude::*;

use super::{
    intent::EntriesPageIntent, reducer::dispatch_entries_page_intent, state::EntriesPageState,
};
use crate::ui::{
    EntriesCommand, UiCommand, UiIntent, remember_entry_controls_hidden, spawn_projected_ui_command,
};
use rssr_domain::EntryQuery;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct EntriesPageSession {
    feed_id: Option<i64>,
    state: Signal<EntriesPageState>,
}

impl EntriesPageSession {
    pub(crate) fn new(feed_id: Option<i64>, state: Signal<EntriesPageState>) -> Self {
        Self { feed_id, state }
    }

    pub(crate) fn snapshot(self) -> EntriesPageState {
        (self.state)()
    }

    /// 借用状态而不是克隆整份。
    ///
    /// `snapshot()` 会深拷贝整个 `EntriesPageState`（含 status / selected_feed_urls / feeds），
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
        self.spawn_ui_command(UiCommand::Entries(EntriesCommand::LoadEntries { query }));
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
