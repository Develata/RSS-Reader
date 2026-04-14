use rssr_domain::{
    EntriesWorkspaceState, EntrySummary, FeedSummary, ReadFilter, StarredFilter, UserSettings,
};

use super::state::EntryGroupingMode;

#[derive(Debug, Clone)]
pub(crate) enum EntriesPageIntent {
    ApplyLoadedSettings(UserSettings),
    ApplyLoadedWorkspaceState(EntriesWorkspaceState),
    SetFeeds(Vec<FeedSummary>),
    SetEntries(Vec<EntrySummary>),
    PatchEntryFlags { entry_id: i64, is_read: Option<bool>, is_starred: Option<bool> },
    SetStatus { message: String, tone: String },
    SetGroupingMode(EntryGroupingMode),
    SetShowArchived(bool),
    SetReadFilter(ReadFilter),
    SetStarredFilter(StarredFilter),
    SetSelectedFeedUrls(Vec<String>),
    SetCurrentPage(u32),
    GoToNextPage,
    GoToPreviousPage,
    SetControlsHidden(bool),
    ToggleDirectorySection(String),
}
