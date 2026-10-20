use rssr_domain::{
    EntriesWorkspaceState, EntrySummary, FeedSummary, ReadFilter, StarredFilter, UserSettings,
};

use super::state::EntryGroupingMode;
use crate::ui::failure::UiFailure;

#[derive(Debug, Clone)]
pub(crate) enum EntriesPageIntent {
    BeginBulk {
        writing: bool,
    },
    CancelBulk,
    BulkPreview(rssr_domain::MarkReadPreview),
    BulkApplied(u64),
    /// The preview or the confirmed write failed; an unknown outcome also drops the preview
    /// (the write may have applied).
    BulkFailed(UiFailure),

    ApplyLoadedSettings(UserSettings),
    ApplyLoadedWorkspaceState(EntriesWorkspaceState),
    PreferencesLoaded,
    PreferencesUnavailable(UiFailure),
    SetFeeds(Vec<FeedSummary>),
    /// 列表查询已发出 / 最新一代查询的结果已落地（成功或失败）。
    EntriesQueryStarted,
    EntriesQueryFinished,
    SetEntries {
        entries: Vec<EntrySummary>,
        archived_count: usize,
        /// More entries matched than the list returns (`rssr_application::MAX_LISTED_ENTRIES`).
        truncated: bool,
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
    /// A backend call failed (C4b item 1).
    Failed(UiFailure),
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

impl EntriesPageIntent {
    /// The backend failure this intent carries.
    pub(crate) fn failure(&self) -> Option<&UiFailure> {
        match self {
            Self::BulkFailed(failure)
            | Self::PreferencesUnavailable(failure)
            | Self::Failed(failure) => Some(failure),
            _ => None,
        }
    }
}
