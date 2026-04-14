use std::sync::Arc;

use time::OffsetDateTime;

use super::{
    groups::{
        EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem, EntryMonthGroup,
        EntrySourceGroup, build_directory_months, build_directory_sources, build_group_nav_items,
        build_month_nav_items, find_active_source_anchors, find_active_time_anchors,
        group_entries_by_source_tree, group_entries_by_time_tree,
    },
    state::{EntriesPageState, EntryGroupingMode},
};

#[derive(Clone)]
pub(crate) struct EntriesPagePresenter {
    pub(crate) archived_count: usize,
    pub(crate) visible_entries_len: usize,
    pub(crate) page_size: usize,
    pub(crate) current_page: u32,
    pub(crate) total_pages: u32,
    pub(crate) page_start: usize,
    pub(crate) page_end: usize,
    pub(crate) source_filter_options: Vec<(i64, String, String)>,
    pub(crate) source_grouped_entries: Vec<EntrySourceGroup>,
    pub(crate) time_grouped_entries: Vec<EntryMonthGroup>,
    pub(crate) directory_months: Vec<EntryDirectoryMonth>,
    pub(crate) directory_sources: Vec<EntryDirectorySource>,
    pub(crate) group_nav_items: Vec<EntryGroupNavItem>,
    pub(crate) active_group_anchor: Option<String>,
    pub(crate) active_directory_anchor: Option<String>,
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
            let is_archived = rssr_domain::is_entry_archived(
                entry.published_at,
                state.archive_after_months,
                now,
            );
            if is_archived {
                archived_count += 1;
            }
            if state.show_archived || !is_archived {
                visible_entries.push(Arc::new(entry.clone()));
            }
        }

        let visible_entries_len = visible_entries.len();
        let page_size = state.page_size();
        let total_pages = if visible_entries_len == 0 {
            0
        } else {
            ((visible_entries_len - 1) / page_size) as u32 + 1
        };
        let current_page = if total_pages == 0 {
            1
        } else {
            state.current_page.min(total_pages).max(1)
        };
        let page_start_index = if total_pages == 0 {
            0
        } else {
            ((current_page - 1) as usize) * page_size
        };
        let page_end_index = visible_entries_len.min(page_start_index.saturating_add(page_size));
        let paged_entries = visible_entries[page_start_index..page_end_index].to_vec();
        let page_start = if visible_entries_len == 0 { 0 } else { page_start_index + 1 };
        let page_end = if visible_entries_len == 0 { 0 } else { page_end_index };
        let source_filter_options = if feed_id.is_some() {
            Vec::new()
        } else {
            state
                .feeds
                .iter()
                .map(|feed| (feed.id, feed.title.clone(), feed.url.clone()))
                .collect::<Vec<_>>()
        };

        let current_entry_id = paged_entries.first().map(|entry| entry.id);

        let (time_grouped_entries, source_grouped_entries, directory_months, directory_sources, group_nav_items, active_group_anchor, active_directory_anchor) =
            match state.grouping_mode {
                EntryGroupingMode::Time => {
                    let all_groups = group_entries_by_time_tree(&visible_entries, page_size);
                    let paged_groups = group_entries_by_time_tree(&paged_entries, page_size);
                    let (active_group_anchor, active_directory_anchor) =
                        find_active_time_anchors(&all_groups, current_entry_id);
                    let directory_months = build_directory_months(
                        &all_groups,
                        active_group_anchor.as_deref(),
                        active_directory_anchor.as_deref(),
                    );
                    let group_nav_items =
                        build_month_nav_items(&all_groups, active_group_anchor.as_deref());
                    (
                        paged_groups,
                        Vec::new(),
                        directory_months,
                        Vec::new(),
                        group_nav_items,
                        active_group_anchor,
                        active_directory_anchor,
                    )
                }
                EntryGroupingMode::Source => {
                    let all_groups = group_entries_by_source_tree(&visible_entries, page_size);
                    let paged_groups = group_entries_by_source_tree(&paged_entries, page_size);
                    let (active_group_anchor, active_directory_anchor) =
                        find_active_source_anchors(&all_groups, current_entry_id);
                    let directory_sources = build_directory_sources(
                        &all_groups,
                        active_group_anchor.as_deref(),
                        active_directory_anchor.as_deref(),
                    );
                    let group_nav_items =
                        build_group_nav_items(&all_groups, active_group_anchor.as_deref());
                    (
                        Vec::new(),
                        paged_groups,
                        Vec::new(),
                        directory_sources,
                        group_nav_items,
                        active_group_anchor,
                        active_directory_anchor,
                    )
                }
            };

        Self {
            archived_count,
            visible_entries_len,
            page_size,
            current_page,
            total_pages,
            page_start,
            page_end,
            source_filter_options,
            source_grouped_entries,
            time_grouped_entries,
            directory_months,
            directory_sources,
            group_nav_items,
            active_group_anchor,
            active_directory_anchor,
        }
    }
}

#[cfg(test)]
mod tests {
    use rssr_domain::{EntrySummary, FeedSummary};
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
    fn presenter_uses_page_slice_for_rendering_and_full_scope_for_directory() {
        let mut state = EntriesPageState::new(true);
        state.entries_page_size = 2;
        state.current_page = 2;
        state.grouping_mode = EntryGroupingMode::Time;
        state.entries = vec![
            entry(1, 1, "Alpha", "2026-04-04T08:00:00Z"),
            entry(2, 1, "Alpha", "2026-04-03T08:00:00Z"),
            entry(3, 2, "Beta", "2026-04-02T08:00:00Z"),
            entry(4, 2, "Beta", "2026-04-01T08:00:00Z"),
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

        assert_eq!(presenter.visible_entries_len, 4);
        assert_eq!(presenter.current_page, 2);
        assert_eq!(presenter.total_pages, 2);
        assert_eq!(presenter.page_start, 3);
        assert_eq!(presenter.page_end, 4);
        let rendered_total = presenter
            .time_grouped_entries
            .iter()
            .flat_map(|month| month.dates.iter())
            .flat_map(|date| date.sources.iter())
            .map(|source| source.entries.len())
            .sum::<usize>();
        assert_eq!(rendered_total, 2);
        assert!(!presenter.directory_months.is_empty());
    }

    #[test]
    fn presenter_marks_directory_item_for_current_page_first_entry() {
        let mut state = EntriesPageState::new(true);
        state.entries_page_size = 1;
        state.current_page = 2;
        state.grouping_mode = EntryGroupingMode::Source;
        state.entries = vec![
            entry(1, 1, "Alpha", "2026-04-04T08:00:00Z"),
            entry(2, 2, "Beta", "2026-04-03T08:00:00Z"),
        ];

        let presenter = EntriesPagePresenter::from_state(
            &state,
            None,
            parse_datetime("2026-04-13T08:00:00Z"),
        );

        assert_eq!(presenter.active_group_anchor.as_deref(), Some("entry-group-beta"));
        assert!(presenter.group_nav_items.iter().any(|item| item.is_active));
        assert!(presenter.directory_sources.iter().any(|item| item.is_active));
    }
}
