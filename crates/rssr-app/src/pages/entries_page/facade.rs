use crate::ui::AppShellState;
use rssr_domain::{ReadFilter, StarredFilter};
use time::OffsetDateTime;

use super::{
    groups::{
        EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem, EntryMonthGroup,
        EntrySourceGroup,
    },
    intent::EntriesPageIntent, presenter::EntriesPagePresenter, session::EntriesPageSession,
    state::{EntriesPageState, EntryGroupingMode},
};

#[derive(Clone)]
pub(crate) struct EntriesPageFacade {
    ui: AppShellState,
    session: EntriesPageSession,
    snapshot: EntriesPageState,
    presenter: EntriesPagePresenter,
}

impl EntriesPageFacade {
    pub(crate) fn new(
        ui: AppShellState,
        session: EntriesPageSession,
        snapshot: EntriesPageState,
        now: OffsetDateTime,
    ) -> Self {
        let presenter = session.presenter(now);
        Self { ui, session, snapshot, presenter }
    }

    pub(crate) fn entry_search(&self) -> String {
        self.ui.entry_search()
    }

    pub(crate) fn set_entry_search(&self, value: String) {
        self.ui.set_entry_search(value);
    }

    pub(crate) fn controls_hidden(&self) -> bool {
        self.snapshot.controls_hidden
    }

    pub(crate) fn grouping_mode(&self) -> EntryGroupingMode {
        self.snapshot.grouping_mode
    }

    pub(crate) fn show_archived(&self) -> bool {
        self.snapshot.show_archived
    }

    pub(crate) fn archive_after_months(&self) -> u32 {
        self.snapshot.archive_after_months
    }

    pub(crate) fn read_filter(&self) -> ReadFilter {
        self.snapshot.read_filter
    }

    pub(crate) fn starred_filter(&self) -> StarredFilter {
        self.snapshot.starred_filter
    }

    pub(crate) fn selected_feed_urls(&self) -> &[String] {
        &self.snapshot.selected_feed_urls
    }

    pub(crate) fn status(&self) -> &str {
        &self.snapshot.status
    }

    pub(crate) fn status_tone(&self) -> &str {
        &self.snapshot.status_tone
    }

    pub(crate) fn entries_is_empty(&self) -> bool {
        self.snapshot.entries.is_empty()
    }

    pub(crate) fn visible_entries_is_empty(&self) -> bool {
        self.presenter.visible_entries.is_empty()
    }

    pub(crate) fn visible_entries_len(&self) -> usize {
        self.presenter.visible_entries.len()
    }

    pub(crate) fn archived_count(&self) -> usize {
        self.presenter.archived_count
    }

    pub(crate) fn source_filter_options(&self) -> &[(i64, String, String)] {
        &self.presenter.source_filter_options
    }

    pub(crate) fn group_nav_items(&self) -> &[EntryGroupNavItem] {
        &self.presenter.group_nav_items
    }

    pub(crate) fn time_grouped_entries(&self) -> &[EntryMonthGroup] {
        &self.presenter.time_grouped_entries
    }

    pub(crate) fn source_grouped_entries(&self) -> &[EntrySourceGroup] {
        &self.presenter.source_grouped_entries
    }

    pub(crate) fn directory_months(&self) -> &[EntryDirectoryMonth] {
        &self.presenter.directory_months
    }

    pub(crate) fn directory_sources(&self) -> &[EntryDirectorySource] {
        &self.presenter.directory_sources
    }

    pub(crate) fn expanded_directory_sources(&self) -> &std::collections::BTreeSet<String> {
        &self.snapshot.expanded_directory_sources
    }

    pub(crate) fn set_controls_hidden(&self, hidden: bool) {
        self.session.dispatch(EntriesPageIntent::SetControlsHidden(hidden));
    }

    pub(crate) fn set_grouping_mode(&self, mode: EntryGroupingMode) {
        self.session.dispatch(EntriesPageIntent::SetGroupingMode(mode));
    }

    pub(crate) fn set_show_archived(&self, value: bool) {
        self.session.dispatch(EntriesPageIntent::SetShowArchived(value));
    }

    pub(crate) fn set_read_filter(&self, value: ReadFilter) {
        self.session.dispatch(EntriesPageIntent::SetReadFilter(value));
    }

    pub(crate) fn set_starred_filter(&self, value: StarredFilter) {
        self.session.dispatch(EntriesPageIntent::SetStarredFilter(value));
    }

    pub(crate) fn set_selected_feed_urls(&self, value: Vec<String>) {
        self.session
            .dispatch(EntriesPageIntent::SetSelectedFeedUrls(value));
    }

    pub(crate) fn toggle_directory_source(&self, anchor_id: String) {
        self.session
            .dispatch(EntriesPageIntent::ToggleDirectorySource(anchor_id));
    }

    pub(crate) fn toggle_read(&self, entry_id: i64, title: String, is_read: bool) {
        self.session.toggle_read(entry_id, title, is_read);
    }

    pub(crate) fn toggle_starred(&self, entry_id: i64, title: String, is_starred: bool) {
        self.session.toggle_starred(entry_id, title, is_starred);
    }
}
