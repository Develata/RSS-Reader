use rssr_domain::{EntryGroupingPreference, ReadFilter, StarredFilter};

#[derive(Debug, Clone)]
pub(crate) enum EntriesPageCommand {
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

pub(crate) struct EntriesPageCommandOutcome {
    pub(crate) status_message: Option<String>,
    pub(crate) status_tone: Option<&'static str>,
    pub(crate) reload: bool,
}

pub(crate) fn info(message: impl Into<String>, reload: bool) -> EntriesPageCommandOutcome {
    EntriesPageCommandOutcome {
        status_message: Some(message.into()),
        status_tone: Some("info"),
        reload,
    }
}

pub(crate) fn error(message: impl Into<String>, reload: bool) -> EntriesPageCommandOutcome {
    EntriesPageCommandOutcome {
        status_message: Some(message.into()),
        status_tone: Some("error"),
        reload,
    }
}

pub(crate) fn silent(reload: bool) -> EntriesPageCommandOutcome {
    EntriesPageCommandOutcome { status_message: None, status_tone: None, reload }
}
