use rssr_domain::{
    EntriesWorkspaceState, EntrySummary, FeedSummary, ReadFilter, StarredFilter, UserSettings,
};

use super::state::EntryGroupingMode;
use crate::ui::failure::UiFailure;

#[derive(Debug, Clone)]
pub(crate) enum EntriesPageIntent {
    ToggleSelected(i64),
    TogglePageSelection,
    ClearSelection,
    ReloadEntries,
    EntryWriteStarted(i64),
    EntryWriteFinished(i64),
    SelectionApplied {
        ids: Vec<i64>,
        action: rssr_domain::EntryBatchAction,
        count: u64,
    },
    BeginBulk {
        writing: bool,
    },
    CancelBulk,
    BulkFailed(UiFailure),

    ApplyLoadedSettings(UserSettings),
    ApplyLoadedWorkspaceState(EntriesWorkspaceState),
    PreferencesLoaded,
    PreferencesUnavailable(UiFailure),
    /// The read-only refresh's preferences (C4b item 2): the settings apply; the browsing
    /// preferences only where the page has none of its own unsaved.
    PreferencesReread {
        settings: UserSettings,
        workspace: EntriesWorkspaceState,
    },
    /// The read-only refresh's read failed.
    PreferencesRereadFailed(UiFailure),
    /// The browsing preferences were saved.
    WorkspaceSaved(EntriesWorkspaceState),
    /// A save of the browsing preferences was sent / ended (saved or not).
    WorkspaceSaveStarted,
    WorkspaceSaveEnded,
    /// The user chose these preferences while a save was on its way.
    WorkspaceWanted(EntriesWorkspaceState),
    /// A save of the preferences ended with an unknown outcome: what the library holds is not
    /// known until a read tells again.
    WorkspaceSaveUnknown,
    /// The save effect fired once since a read recovered the user's filters: the recovery's
    /// firing is handled (review C4B-B-R3-01).
    RecoveredEditsHandled,
    /// A read of the preferences and subscriptions was issued / the latest one answered.
    BootstrapStarted,
    BootstrapFinished,
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
    /// A backend call failed (C4b item 1): a write's failure, kept over later list reads.
    Failed(UiFailure),
    /// A read failed (the list, the subscriptions): the next read replaces it.
    ReadFailed(UiFailure),
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

impl EntriesPageIntent {
    /// The backend failure this intent carries.
    pub(crate) fn failure(&self) -> Option<&UiFailure> {
        match self {
            Self::BulkFailed(failure)
            | Self::PreferencesUnavailable(failure)
            | Self::PreferencesRereadFailed(failure)
            | Self::Failed(failure)
            | Self::ReadFailed(failure) => Some(failure),
            _ => None,
        }
    }
}
