use rssr_domain::{EntryGroupingPreference, EntryQuery, ReadFilter, StarredFilter};

#[derive(Debug, Clone)]
pub(crate) enum EntriesCommand {
    PreviewMarkRead {
        query: EntryQuery,
    },
    ConfirmMarkRead {
        preview: rssr_domain::MarkReadPreview,
    },

    Bootstrap {
        feed_id: Option<i64>,
        load_preferences: bool,
        load_feeds: bool,
    },
    LoadEntries {
        query: EntryQuery,
    },
    SetRead {
        entry_id: i64,
        entry_title: String,
        read: bool,
    },
    SetStarred {
        entry_id: i64,
        entry_title: String,
        starred: bool,
    },
    SaveBrowsingPreferences {
        grouping_mode: EntryGroupingPreference,
        show_archived: bool,
        read_filter: ReadFilter,
        starred_filter: StarredFilter,
        selected_feed_urls: Vec<String>,
    },
}
