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
        load_preferences: bool,
        load_feeds: bool,
    },
    /// The page's read-only refresh (C4b item 2): its preferences and subscriptions again.
    Reread,
    LoadEntries {
        query: EntryQuery,
    },
    /// The page opened this subscription: remember it for the startup view.
    RememberOpenedFeed {
        feed_id: i64,
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
