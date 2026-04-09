use rssr_domain::{EntryGroupingPreference, EntryQuery, ReadFilter, StarredFilter, UserSettings};

#[derive(Debug, Clone)]
pub(crate) enum UiCommand {
    LoadAuthenticatedShell,
    ResolveStartupRoute,
    EntriesBootstrap {
        feed_id: Option<i64>,
        load_preferences: bool,
        load_feeds: bool,
    },
    EntriesLoadEntries {
        query: EntryQuery,
    },
    EntriesToggleRead {
        entry_id: i64,
        entry_title: String,
        currently_read: bool,
    },
    EntriesToggleStarred {
        entry_id: i64,
        entry_title: String,
        currently_starred: bool,
    },
    EntriesSaveBrowsingPreferences {
        grouping_mode: EntryGroupingPreference,
        show_archived: bool,
        read_filter: ReadFilter,
        starred_filter: StarredFilter,
        selected_feed_urls: Vec<String>,
    },
    ReaderLoadEntry {
        entry_id: i64,
    },
    ReaderToggleRead {
        entry_id: i64,
        currently_read: bool,
        via_shortcut: bool,
    },
    ReaderToggleStarred {
        entry_id: i64,
        currently_starred: bool,
        via_shortcut: bool,
    },
    FeedsLoadSnapshot,
    FeedsAddFeed {
        raw_url: String,
    },
    FeedsRefreshAll,
    FeedsRefreshFeed {
        feed_id: i64,
        feed_title: String,
    },
    FeedsRemoveFeed {
        feed_id: i64,
        feed_title: String,
        confirmed: bool,
    },
    FeedsExportConfig,
    FeedsImportConfig {
        raw: String,
        confirmed: bool,
    },
    FeedsExportOpml,
    FeedsImportOpml {
        raw: String,
    },
    FeedsReadFeedUrlFromClipboard,
    SettingsLoad,
    SettingsSaveAppearance {
        settings: UserSettings,
        success_message: String,
    },
    SettingsPushConfig {
        endpoint: String,
        remote_path: String,
    },
    SettingsPullConfig {
        endpoint: String,
        remote_path: String,
    },
}
