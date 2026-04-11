use std::collections::BTreeSet;

use rssr_domain::{
    EntriesWorkspaceState, EntryGroupingPreference, EntryQuery, EntrySummary, FeedSummary,
    ReadFilter, StarredFilter, UserSettings,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryGroupingMode {
    Time,
    Source,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EntriesPageState {
    pub(crate) entries: Vec<EntrySummary>,
    pub(crate) feeds: Vec<FeedSummary>,
    pub(crate) read_filter: ReadFilter,
    pub(crate) starred_filter: StarredFilter,
    pub(crate) selected_feed_urls: Vec<String>,
    pub(crate) show_archived: bool,
    pub(crate) grouping_mode: EntryGroupingMode,
    pub(crate) archive_after_months: u32,
    pub(crate) expanded_directory_sources: BTreeSet<String>,
    pub(crate) controls_hidden: bool,
    pub(crate) reload_tick: u64,
    pub(crate) status: String,
    pub(crate) status_tone: String,
    pub(crate) preferences_loaded: bool,
}

impl EntriesPageState {
    pub(crate) fn new(initial_controls_hidden: bool) -> Self {
        let settings = UserSettings::default();
        let workspace = EntriesWorkspaceState::default();
        Self {
            entries: Vec::new(),
            feeds: Vec::new(),
            read_filter: workspace.read_filter,
            starred_filter: workspace.starred_filter,
            selected_feed_urls: workspace.selected_feed_urls,
            show_archived: workspace.show_archived,
            grouping_mode: entry_grouping_mode_from_preference(workspace.grouping_mode),
            archive_after_months: settings.archive_after_months,
            expanded_directory_sources: BTreeSet::new(),
            controls_hidden: initial_controls_hidden,
            reload_tick: 0,
            status: "正在加载文章列表…".to_string(),
            status_tone: "info".to_string(),
            preferences_loaded: false,
        }
    }

    pub(crate) fn entry_query(
        &self,
        feed_id: Option<i64>,
        search_title: Option<String>,
    ) -> EntryQuery {
        EntryQuery {
            feed_id,
            read_filter: self.read_filter,
            starred_filter: self.starred_filter,
            feed_ids: if feed_id.is_some() {
                Vec::new()
            } else {
                map_selected_feed_urls_to_ids(&self.feeds, &self.selected_feed_urls)
            },
            search_title,
            limit: None,
        }
    }
}

pub(crate) fn entry_grouping_mode_from_preference(
    preference: EntryGroupingPreference,
) -> EntryGroupingMode {
    match preference {
        EntryGroupingPreference::Time => EntryGroupingMode::Time,
        EntryGroupingPreference::Source => EntryGroupingMode::Source,
    }
}

pub(crate) fn grouping_mode_preference(mode: EntryGroupingMode) -> EntryGroupingPreference {
    match mode {
        EntryGroupingMode::Time => EntryGroupingPreference::Time,
        EntryGroupingMode::Source => EntryGroupingPreference::Source,
    }
}

pub(crate) fn map_selected_feed_urls_to_ids(
    feeds: &[FeedSummary],
    selected_feed_urls: &[String],
) -> Vec<i64> {
    if selected_feed_urls.is_empty() {
        return Vec::new();
    }

    let selected = selected_feed_urls.iter().map(String::as_str).collect::<BTreeSet<_>>();
    feeds
        .iter()
        .filter(|feed| selected.contains(feed.url.as_str()))
        .map(|feed| feed.id)
        .collect::<Vec<_>>()
}
