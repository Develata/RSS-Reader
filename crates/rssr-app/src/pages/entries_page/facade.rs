use crate::ui::AppShellState;
use rssr_domain::{ReadFilter, StarredFilter};
use time::OffsetDateTime;

use super::{
    intent::EntriesPageIntent, presenter::EntriesPagePresenter, session::EntriesPageSession,
    state::{EntriesPageState, EntryGroupingMode},
};

#[derive(Clone)]
pub(crate) struct EntriesPageFacade {
    pub(crate) ui: AppShellState,
    pub(crate) session: EntriesPageSession,
    pub(crate) snapshot: EntriesPageState,
    pub(crate) presenter: EntriesPagePresenter,
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
