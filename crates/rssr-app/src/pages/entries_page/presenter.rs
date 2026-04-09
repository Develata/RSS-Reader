use rssr_domain::{EntrySummary, is_entry_archived};
use time::OffsetDateTime;

use super::{
    groups::{
        EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem, EntryMonthGroup,
        EntrySourceGroup, build_directory_months, build_directory_sources, build_group_nav_items,
        build_month_nav_items, group_entries_by_source_tree, group_entries_by_time_tree,
    },
    state::{EntriesPageState, EntryGroupingMode},
};

#[derive(Clone)]
pub(crate) struct EntriesPagePresenter {
    pub(crate) archived_count: usize,
    pub(crate) visible_entries: Vec<EntrySummary>,
    pub(crate) source_filter_options: Vec<(i64, String, String)>,
    pub(crate) source_grouped_entries: Vec<EntrySourceGroup>,
    pub(crate) time_grouped_entries: Vec<EntryMonthGroup>,
    pub(crate) directory_months: Vec<EntryDirectoryMonth>,
    pub(crate) directory_sources: Vec<EntryDirectorySource>,
    pub(crate) group_nav_items: Vec<EntryGroupNavItem>,
}

impl EntriesPagePresenter {
    pub(crate) fn from_state(
        state: &EntriesPageState,
        feed_id: Option<i64>,
        now: OffsetDateTime,
    ) -> Self {
        let archived_count = state
            .entries
            .iter()
            .filter(|entry| is_entry_archived(entry.published_at, state.archive_after_months, now))
            .count();
        let visible_entries = state
            .entries
            .iter()
            .filter(|entry| {
                state.show_archived
                    || !is_entry_archived(entry.published_at, state.archive_after_months, now)
            })
            .cloned()
            .collect::<Vec<_>>();
        let source_filter_options = if feed_id.is_some() {
            Vec::new()
        } else {
            state
                .feeds
                .iter()
                .map(|feed| (feed.id, feed.title.clone(), feed.url.clone()))
                .collect::<Vec<_>>()
        };
        let source_grouped_entries = group_entries_by_source_tree(&visible_entries);
        let time_grouped_entries = group_entries_by_time_tree(&visible_entries);
        let directory_months = build_directory_months(&time_grouped_entries);
        let directory_sources = build_directory_sources(&source_grouped_entries);
        let group_nav_items = match state.grouping_mode {
            EntryGroupingMode::Time => build_month_nav_items(&time_grouped_entries),
            EntryGroupingMode::Source => build_group_nav_items(&source_grouped_entries),
        };

        Self {
            archived_count,
            visible_entries,
            source_filter_options,
            source_grouped_entries,
            time_grouped_entries,
            directory_months,
            directory_sources,
            group_nav_items,
        }
    }
}
