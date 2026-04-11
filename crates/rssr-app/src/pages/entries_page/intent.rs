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
    SetStatus { message: String, tone: String },
    SetGroupingMode(EntryGroupingMode),
    SetShowArchived(bool),
    SetReadFilter(ReadFilter),
    SetStarredFilter(StarredFilter),
    SetSelectedFeedUrls(Vec<String>),
    SetControlsHidden(bool),
    ToggleDirectorySource(String),
    BumpReload,
}
