use rssr_domain::{
    EntriesWorkspaceState, EntrySummary, FeedSummary, ReadFilter, StarredFilter, UserSettings,
};

use super::state::EntryGroupingMode;

#[derive(Debug, Clone)]
pub(crate) enum EntriesPageIntent {
    BeginBulk {
        writing: bool,
    },
    CancelBulk,
    BulkPreview(rssr_domain::MarkReadPreview),
    BulkApplied(u64),
    BulkFailed(String),

    ApplyLoadedSettings(UserSettings),
    ApplyLoadedWorkspaceState(EntriesWorkspaceState),
    PreferencesLoaded,
    PreferencesUnavailable(String),
    SetFeeds(Vec<FeedSummary>),
    /// 列表查询已发出：此前成功的写入都已在存储里，不必再重放。
    EntriesQueryStarted,
    SetEntries {
        entries: Vec<EntrySummary>,
        archived_count: usize,
    },
    PatchEntryFlags {
        entry_id: i64,
        is_read: Option<bool>,
        is_starred: Option<bool>,
    },
    SetStatus {
        message: String,
        tone: String,
    },
    SetGroupingMode(EntryGroupingMode),
    SetShowArchived(bool),
    SetReadFilter(ReadFilter),
    SetStarredFilter(StarredFilter),
    SetSelectedFeedUrls(Vec<String>),
    ClearFilters,
    SetCurrentPage(u32),
    GoToNextPage,
    GoToPreviousPage,
    SetControlsHidden(bool),
}
