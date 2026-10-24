use rssr_domain::{
    EntriesWorkspaceState, EntrySummary, FeedSummary, ReadFilter, StarredFilter, UserSettings,
};

use super::state::EntryGroupingMode;

#[derive(Debug, Clone)]
pub(crate) enum EntriesPageIntent {
    ToggleSelected(i64),
    TogglePageSelection,
    ClearSelection,
    ReloadEntries,
    EntryWriteStarted(i64),
    EntryWriteFinished(i64),
    SelectionApplied { ids: Vec<i64>, action: rssr_domain::EntryBatchAction, count: u64 },
    BeginBulk { writing: bool },
    CancelBulk,
    BulkFailed(String),

    ApplyLoadedSettings(UserSettings),
    ApplyLoadedWorkspaceState(EntriesWorkspaceState),
    PreferencesLoaded,
    PreferencesUnavailable(String),
    SetFeeds(Vec<FeedSummary>),
    SetEntries { entries: Vec<EntrySummary>, archived_count: usize },
    PatchEntryFlags { entry_id: i64, is_read: Option<bool>, is_starred: Option<bool> },
    SetStatus { message: String, tone: String },
    SetGroupingMode(EntryGroupingMode),
    SetShowArchived(bool),
    SetReadFilter(ReadFilter),
    SetStarredFilter(StarredFilter),
    SetSelectedFeedUrls(Vec<String>),
    ClearFilters,
    SetCurrentPage(u32),
    GoToNextPage,
    GoToPreviousPage,
}
