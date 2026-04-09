use rssr_domain::{EntryGroupingPreference, EntryQuery, ReadFilter, StarredFilter};

#[derive(Debug, Clone)]
pub(crate) enum EntriesPageEffect {
    Bootstrap {
        feed_id: Option<i64>,
        load_preferences: bool,
        load_feeds: bool,
    },
    LoadEntries(EntryQuery),
    ToggleRead {
        entry_id: i64,
        entry_title: String,
        currently_read: bool,
    },
    ToggleStarred {
        entry_id: i64,
        entry_title: String,
        currently_starred: bool,
    },
    SaveBrowsingPreferences {
        grouping_mode: EntryGroupingPreference,
        show_archived: bool,
        read_filter: ReadFilter,
        starred_filter: StarredFilter,
        selected_feed_urls: Vec<String>,
    },
}
