use rssr_domain::{EntrySummary, FeedSummary, ReadFilter, StarredFilter, UserSettings};

use super::state::EntryGroupingMode;

#[derive(Debug, Clone)]
pub(crate) enum EntriesPageIntent {
    ApplyLoadedSettings(UserSettings),
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
