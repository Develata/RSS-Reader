use crate::ui::AppShellState;
use rssr_domain::{ReadFilter, StarredFilter};
use time::OffsetDateTime;

use super::{
    browser_interactions::scroll_to_entry_group,
    groups::{
        EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem, EntryMonthGroup,
        EntrySourceGroup,
    },
    intent::EntriesPageIntent,
    presenter::EntriesPagePresenter,
    session::EntriesPageSession,
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

    pub(crate) fn status_message(&self) -> &str {
        &self.snapshot.status
    }

    pub(crate) fn status_tone(&self) -> &str {
        &self.snapshot.status_tone
    }

    pub(crate) fn has_status_message(&self) -> bool {
        !self.status_message().is_empty()
    }

    pub(crate) fn entries_is_empty(&self) -> bool {
        self.snapshot.entries.is_empty()
    }

    pub(crate) fn visible_entries_is_empty(&self) -> bool {
        self.presenter.visible_entries_len == 0
    }

    pub(crate) fn visible_entries_len(&self) -> usize {
        self.presenter.visible_entries_len
    }

    pub(crate) fn archived_entry_count(&self) -> usize {
        self.presenter.archived_count
    }

    pub(crate) fn page_size(&self) -> usize {
        self.presenter.page_size
    }

    pub(crate) fn current_page(&self) -> u32 {
        self.presenter.current_page
    }

    pub(crate) fn total_pages(&self) -> u32 {
        self.presenter.total_pages
    }

    pub(crate) fn page_start(&self) -> usize {
        self.presenter.page_start
    }

    pub(crate) fn page_end(&self) -> usize {
        self.presenter.page_end
    }

    pub(crate) fn can_go_previous_page(&self) -> bool {
        self.current_page() > 1
    }

    pub(crate) fn can_go_next_page(&self) -> bool {
        self.current_page() < self.total_pages()
    }

    pub(crate) fn active_directory_anchor(&self) -> Option<&str> {
        self.presenter
            .active_directory_anchor
            .as_deref()
            .or(self.presenter.active_group_anchor.as_deref())
    }

    pub(crate) fn archived_entries_message(&self) -> String {
        format!(
            "当前已自动归档 {} 篇较旧文章，可勾选“显示已归档文章”查看。",
            self.archived_entry_count()
        )
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

    pub(crate) fn empty_entries_message(&self) -> String {
        if self.session.feed_id().is_some() {
            "这个订阅下还没有可显示的文章，先尝试刷新该 feed。".to_string()
        } else {
            "没有可显示的文章，先去订阅页添加并刷新 feed。".to_string()
        }
    }

    pub(crate) fn archived_entries_state_message(&self) -> &'static str {
        "当前结果中的文章都已被自动归档，勾选“显示已归档文章”即可查看。"
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
        self.session.dispatch(EntriesPageIntent::SetSelectedFeedUrls(value));
    }

    pub(crate) fn toggle_directory_source(&self, anchor_id: String) {
        self.session.dispatch(EntriesPageIntent::ToggleDirectorySource(anchor_id));
    }

    pub(crate) fn go_to_previous_page(&self) {
        self.session.dispatch(EntriesPageIntent::GoToPreviousPage);
    }

    pub(crate) fn go_to_next_page(&self) {
        self.session.dispatch(EntriesPageIntent::GoToNextPage);
    }

    pub(crate) fn navigate_to_directory_target(&self, target_page: u32, anchor_id: String) {
        self.session.dispatch(EntriesPageIntent::SetCurrentPage(target_page));
        scroll_to_entry_group(&anchor_id);
    }

    pub(crate) fn toggle_read(&self, entry_id: i64, title: String, is_read: bool) {
        self.session.toggle_read(entry_id, title, is_read);
    }

    pub(crate) fn toggle_starred(&self, entry_id: i64, title: String, is_starred: bool) {
        self.session.toggle_starred(entry_id, title, is_starred);
    }
}
