use std::sync::Arc;

use rssr_domain::{EntrySummary, is_entry_archived};
use time::OffsetDateTime;

use super::{
    groups::{
        EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem, EntryMonthGroup,
        EntrySourceGroup, build_directory_months, build_directory_sources, build_group_nav_items,
        build_month_nav_items, group_entries_by_source_tree, group_entries_by_time_tree,
        limit_source_groups, limit_time_groups,
    },
    state::{EntriesPageState, EntryGroupingMode},
};

#[derive(Clone)]
pub(crate) struct EntriesPagePresenter {
    pub(crate) archived_count: usize,
    pub(crate) visible_entries_len: usize,
    pub(crate) rendered_entries_len: usize,
    pub(crate) remaining_entries_count: usize,
    pub(crate) visible_entries: Vec<Arc<EntrySummary>>,
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
        let mut archived_count = 0;
        let mut visible_entries = Vec::with_capacity(state.entries.len());

        for entry in &state.entries {
            let is_archived =
                is_entry_archived(entry.published_at, state.archive_after_months, now);
            if is_archived {
                archived_count += 1;
            }
            if state.show_archived || !is_archived {
                visible_entries.push(Arc::new(entry.clone()));
            }
        }

        let visible_entries_len = visible_entries.len();
        let rendered_entries_len = visible_entries_len.min(state.rendered_entry_limit);
        let remaining_entries_count = visible_entries_len.saturating_sub(rendered_entries_len);
        let source_filter_options = if feed_id.is_some() {
            Vec::new()
        } else {
            state
                .feeds
                .iter()
                .map(|feed| (feed.id, feed.title.clone(), feed.url.clone()))
                .collect::<Vec<_>>()
        };

        let (time_grouped_entries, source_grouped_entries, directory_months, directory_sources, group_nav_items) =
            match state.grouping_mode {
                EntryGroupingMode::Time => {
                    let all_groups = group_entries_by_time_tree(&visible_entries);
                    let directory_months = build_directory_months(&all_groups);
                    let group_nav_items = build_month_nav_items(&all_groups);
                    let rendered_groups = limit_time_groups(&all_groups, state.rendered_entry_limit);
                    (
                        rendered_groups,
                        Vec::new(),
                        directory_months,
                        Vec::new(),
                        group_nav_items,
                    )
                }
                EntryGroupingMode::Source => {
                    let all_groups = group_entries_by_source_tree(&visible_entries);
                    let directory_sources = build_directory_sources(&all_groups);
                    let group_nav_items = build_group_nav_items(&all_groups);
                    let rendered_groups =
                        limit_source_groups(&all_groups, state.rendered_entry_limit);
                    (
                        Vec::new(),
                        rendered_groups,
                        Vec::new(),
                        directory_sources,
                        group_nav_items,
                    )
                }
            };

        Self {
            archived_count,
            visible_entries_len,
            rendered_entries_len,
            remaining_entries_count,
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

#[cfg(test)]
mod tests {
    use rssr_domain::{EntrySummary, FeedSummary, ReadFilter, StarredFilter};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    use super::EntriesPagePresenter;
    use crate::pages::entries_page::state::{EntriesPageState, EntryGroupingMode};

    fn parse_datetime(raw: &str) -> OffsetDateTime {
        OffsetDateTime::parse(raw, &Rfc3339).expect("parse datetime")
    }

    fn entry(id: i64, feed_id: i64, feed_title: &str, published_at: &str) -> EntrySummary {
        EntrySummary {
            id,
            feed_id,
            title: format!("Entry {id}"),
            feed_title: feed_title.to_string(),
            published_at: Some(parse_datetime(published_at)),
            is_read: false,
            is_starred: false,
        }
    }

    fn feed(id: i64, title: &str, url: &str) -> FeedSummary {
        FeedSummary {
            id,
            title: title.to_string(),
            url: url.to_string(),
            unread_count: 0,
            entry_count: 0,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
        }
    }

    #[test]
    fn presenter_builds_only_active_grouping_tree() {
        let mut state = EntriesPageState::new(true);
        state.grouping_mode = EntryGroupingMode::Source;
        state.entries = vec![
            entry(1, 1, "Alpha", "2026-04-03T08:00:00Z"),
            entry(2, 2, "Beta", "2026-04-02T08:00:00Z"),
        ];
        state.feeds = vec![
            feed(1, "Alpha", "https://example.com/alpha.xml"),
            feed(2, "Beta", "https://example.com/beta.xml"),
        ];

        let presenter = EntriesPagePresenter::from_state(
            &state,
            None,
            parse_datetime("2026-04-13T08:00:00Z"),
        );

        assert!(presenter.time_grouped_entries.is_empty());
        assert_eq!(presenter.source_grouped_entries.len(), 2);
        assert_eq!(presenter.group_nav_items.len(), 2);
        assert_eq!(presenter.directory_sources.len(), 2);
        assert_eq!(presenter.source_filter_options.len(), 2);
    }

    #[test]
    fn presenter_respects_rendered_entry_limit() {
        let mut state = EntriesPageState::new(true);
        state.grouping_mode = EntryGroupingMode::Time;
        state.rendered_entry_limit = 2;
        state.read_filter = ReadFilter::All;
        state.starred_filter = StarredFilter::All;
        state.entries = vec![
            entry(1, 1, "Alpha", "2026-04-03T08:00:00Z"),
            entry(2, 1, "Alpha", "2026-04-02T08:00:00Z"),
            entry(3, 2, "Beta", "2026-04-01T08:00:00Z"),
        ];

        let presenter = EntriesPagePresenter::from_state(
            &state,
            None,
            parse_datetime("2026-04-13T08:00:00Z"),
        );

        assert_eq!(presenter.visible_entries_len, 3);
        assert_eq!(presenter.rendered_entries_len, 2);
        assert_eq!(presenter.remaining_entries_count, 1);
        let rendered_total = presenter
            .time_grouped_entries
            .iter()
            .flat_map(|month| month.dates.iter())
            .flat_map(|date| date.sources.iter())
            .map(|source| source.entries.len())
            .sum::<usize>();
        assert_eq!(rendered_total, 2);
    }
}
