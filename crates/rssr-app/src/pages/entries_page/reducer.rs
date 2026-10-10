use std::sync::Arc;

use dioxus::prelude::*;
use rssr_domain::{ReadFilter, StarredFilter};

use super::{
    intent::EntriesPageIntent, state::EntriesPageState, state::FIRST_PAGE_NUMBER,
    state::PreferencesLoadState, state::entry_grouping_mode_from_preference,
};

pub(crate) fn dispatch_entries_page_intent(
    mut state: Signal<EntriesPageState>,
    intent: EntriesPageIntent,
) {
    state.with_mut(|state| reduce_entries_page_intent(state, intent));
}

pub(crate) fn reduce_entries_page_intent(state: &mut EntriesPageState, intent: EntriesPageIntent) {
    match intent {
        EntriesPageIntent::BeginBulk { writing } => {
            state.bulk_generation = state.bulk_generation.wrapping_add(1);
            state.bulk_busy = true;
            state.bulk_writing = writing;
        }
        EntriesPageIntent::CancelBulk => {
            state.bulk_preview = None;
            if !state.bulk_writing {
                state.bulk_generation = state.bulk_generation.wrapping_add(1);
                state.bulk_busy = false;
            }
        }
        EntriesPageIntent::BulkPreview(preview) => {
            let changed = state.bulk_writing;
            state.bulk_busy = false;
            state.bulk_writing = false;
            state.status = if changed {
                "匹配的未读文章已变化，请重新确认。".into()
            } else if preview.unread_entry_ids.is_empty() {
                "当前筛选下没有未读文章。".into()
            } else {
                "请确认批量已读范围。".into()
            };
            state.status_tone = "info".into();
            state.bulk_preview = Some(preview);
        }
        EntriesPageIntent::BulkApplied(count) => {
            state.bulk_busy = false;
            state.bulk_writing = false;
            state.bulk_preview = None;
            state.bulk_revision = state.bulk_revision.wrapping_add(1);
            state.bulk_notice = Some(format!("已将 {count} 篇文章标为已读。"));
            state.status = format!("已将 {count} 篇文章标为已读。");
            state.status_tone = "success".into();
        }
        EntriesPageIntent::BulkFailed(message) => {
            state.bulk_busy = false;
            state.bulk_writing = false;
            state.status = message;
            state.status_tone = "error".into();
        }

        EntriesPageIntent::ApplyLoadedSettings(settings) => {
            state.archive_after_months = settings.archive_after_months;
            state.entries_page_size = settings.entries_page_size.max(1);
            clamp_current_page(state);
        }
        EntriesPageIntent::ApplyLoadedWorkspaceState(workspace) => {
            state.read_filter = workspace.read_filter;
            state.starred_filter = workspace.starred_filter;
            state.selected_feed_urls = workspace.selected_feed_urls;
            state.show_archived = workspace.show_archived;
            state.grouping_mode = entry_grouping_mode_from_preference(workspace.grouping_mode);
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::PreferencesLoaded => {
            if matches!(state.preferences_load, PreferencesLoadState::Unavailable(_)) {
                state.status = format!("共 {} 篇文章。", state.entries.len());
                state.status_tone = "info".to_string();
            }
            state.preferences_load = PreferencesLoadState::Loaded;
        }
        EntriesPageIntent::PreferencesUnavailable(message) => {
            state.status = message.clone();
            state.status_tone = "error".to_string();
            state.preferences_load = PreferencesLoadState::Unavailable(message);
        }
        EntriesPageIntent::SetFeeds(feeds) => state.feeds = Arc::new(feeds),
        EntriesPageIntent::EntriesQueryStarted => state.query_in_flight = true,
        EntriesPageIntent::EntriesQueryFinished => state.query_in_flight = false,
        EntriesPageIntent::SetEntries { entries, archived_count } => {
            state.entries_loaded = true;
            state.entries = Arc::new(entries.into_iter().map(Arc::new).collect());
            state.archived_count = archived_count;
            let count = state.entries.len();
            if let PreferencesLoadState::Unavailable(message) = &state.preferences_load {
                state.status = format!("{message}；文章偏好暂不保存，共 {count} 篇文章。");
                state.status_tone = "error".to_string();
            } else {
                state.status = format!("共 {count} 篇文章。");
                state.status_tone = "info".to_string();
            }
            if let Some(message) = state.bulk_notice.take() {
                state.status = message;
                state.status_tone = "success".into();
            }
            clamp_current_page(state);
        }
        EntriesPageIntent::PatchEntryFlags { entry_id, is_read, is_starred } => {
            let listed = state.entries.iter().any(|entry| entry.id == entry_id);
            apply_flag_patch(state, entry_id, is_read, is_starred);
            // 在途查询可能早于这次写入读了存储，它的结果会把条目改回旧状态，也补不回
            // 因此新满足筛选的条目：session 丢弃它，这里要求重新查询（审计 A3）。
            // 查询也可能在写入完成之前就读完并返回了：例如点了收藏、随即切到“只看收藏”，
            // 列表里没有这一条，写入完成时也没有在途查询。条目不在列表里、写入后又可能符合
            // 筛选时同样重新查询（独立评审）。
            if state.query_in_flight
                || (!listed && patch_may_match_filters(state, is_read, is_starred))
            {
                state.requery_revision = state.requery_revision.wrapping_add(1);
            }
        }
        EntriesPageIntent::SetStatus { message, tone } => {
            state.status = message;
            state.status_tone = tone;
        }
        EntriesPageIntent::SetGroupingMode(grouping_mode) => {
            state.grouping_mode = grouping_mode;
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::SetShowArchived(show_archived) => {
            state.show_archived = show_archived;
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::SetReadFilter(read_filter) => {
            state.read_filter = read_filter;
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::SetStarredFilter(starred_filter) => {
            state.starred_filter = starred_filter;
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::SetSelectedFeedUrls(selected_feed_urls) => {
            state.selected_feed_urls = selected_feed_urls;
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::ClearFilters => {
            state.read_filter = ReadFilter::All;
            state.starred_filter = StarredFilter::All;
            state.selected_feed_urls.clear();
            state.current_page = FIRST_PAGE_NUMBER;
        }
        EntriesPageIntent::SetCurrentPage(page) => {
            state.current_page = page.max(FIRST_PAGE_NUMBER);
            clamp_current_page(state);
        }
        EntriesPageIntent::GoToNextPage => {
            state.current_page = state.current_page.saturating_add(1);
            clamp_current_page(state);
        }
        EntriesPageIntent::GoToPreviousPage => {
            state.current_page = state.current_page.saturating_sub(1).max(FIRST_PAGE_NUMBER);
        }
        EntriesPageIntent::SetControlsHidden(hidden) => state.controls_hidden = hidden,
    }
}

fn apply_flag_patch(
    state: &mut EntriesPageState,
    entry_id: i64,
    is_read: Option<bool>,
    is_starred: Option<bool>,
) {
    let entries = Arc::make_mut(&mut state.entries);
    if let Some(entry) = entries.iter_mut().find(|entry| entry.id == entry_id) {
        // 只有被点的那一条需要真正写入，其余条目继续共享原有 Arc。
        let entry = Arc::make_mut(entry);
        if let Some(is_read) = is_read {
            entry.is_read = is_read;
        }
        if let Some(is_starred) = is_starred {
            entry.is_starred = is_starred;
        }
    }
    let read_filter = state.read_filter;
    let starred_filter = state.starred_filter;
    entries.retain(|entry| matches_current_filters(read_filter, starred_filter, entry));
    clamp_current_page(state);
}

/// 只知道这次改动的标记时，条目是否可能符合当前筛选；没改动的标记按“可能符合”算。
fn patch_may_match_filters(
    state: &EntriesPageState,
    is_read: Option<bool>,
    is_starred: Option<bool>,
) -> bool {
    let read_may_match = match (state.read_filter, is_read) {
        (ReadFilter::UnreadOnly, Some(is_read)) => !is_read,
        (ReadFilter::ReadOnly, Some(is_read)) => is_read,
        _ => true,
    };
    let starred_may_match = match (state.starred_filter, is_starred) {
        (StarredFilter::StarredOnly, Some(is_starred)) => is_starred,
        (StarredFilter::UnstarredOnly, Some(is_starred)) => !is_starred,
        _ => true,
    };
    read_may_match && starred_may_match
}

fn matches_current_filters(
    read_filter: ReadFilter,
    starred_filter: StarredFilter,
    entry: &rssr_domain::EntrySummary,
) -> bool {
    let matches_read = match read_filter {
        ReadFilter::All => true,
        ReadFilter::UnreadOnly => !entry.is_read,
        ReadFilter::ReadOnly => entry.is_read,
    };
    let matches_starred = match starred_filter {
        StarredFilter::All => true,
        StarredFilter::StarredOnly => entry.is_starred,
        StarredFilter::UnstarredOnly => !entry.is_starred,
    };

    matches_read && matches_starred
}

fn clamp_current_page(state: &mut EntriesPageState) {
    let page_size = state.page_size().max(1);
    let total_entries = state.entries.len();
    let total_pages = if total_entries == 0 {
        FIRST_PAGE_NUMBER
    } else {
        ((total_entries - 1) / page_size) as u32 + 1
    };
    state.current_page = state.current_page.clamp(FIRST_PAGE_NUMBER, total_pages);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{EntrySummary, ReadFilter, StarredFilter};

    use super::reduce_entries_page_intent;
    use crate::pages::entries_page::{
        intent::EntriesPageIntent,
        state::{EntriesPageState, FIRST_PAGE_NUMBER},
    };

    fn entry(id: i64, is_read: bool) -> EntrySummary {
        EntrySummary {
            id,
            feed_id: 1,
            title: format!("Entry {id}"),
            feed_title: "Feed".to_string(),
            published_at: None,
            is_read,
            is_starred: false,
        }
    }

    #[test]
    fn patch_entry_flags_removes_entry_that_no_longer_matches_filter() {
        let mut state = EntriesPageState::new(true);
        state.read_filter = ReadFilter::UnreadOnly;
        state.entries = vec![Arc::new(entry(1, false))].into();

        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: Some(true),
                is_starred: None,
            },
        );

        assert!(state.entries.is_empty());
    }

    #[test]
    fn set_entries_clamps_current_page() {
        let mut state = EntriesPageState::new(true);
        state.current_page = 9;

        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetEntries { entries: vec![entry(1, false)], archived_count: 0 },
        );

        assert_eq!(state.current_page, FIRST_PAGE_NUMBER);
    }

    #[test]
    fn changing_filters_resets_current_page() {
        let mut state = EntriesPageState::new(true);
        state.current_page = 4;

        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetStarredFilter(rssr_domain::StarredFilter::StarredOnly),
        );

        assert_eq!(state.current_page, FIRST_PAGE_NUMBER);
    }

    #[test]
    fn changing_page_updates_current_page() {
        let mut state = EntriesPageState::new(true);
        state.entries_page_size = 1;
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();

        reduce_entries_page_intent(&mut state, EntriesPageIntent::SetCurrentPage(2));

        assert_eq!(state.current_page, 2);
    }

    #[test]
    fn pagination_clamps_and_preserves_filters_and_source_selection() {
        let mut state = EntriesPageState::new(true);
        state.entries_page_size = 1;
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();
        state.selected_feed_urls = vec!["https://example.com/中文 feed".into()];
        state.read_filter = ReadFilter::UnreadOnly;
        let workspace = state.clone();
        reduce_entries_page_intent(&mut state, EntriesPageIntent::GoToPreviousPage);
        assert_eq!(state.current_page, 1);
        for _ in 0..3 {
            reduce_entries_page_intent(&mut state, EntriesPageIntent::GoToNextPage);
        }
        assert_eq!(state.current_page, 2);
        assert_eq!(state.read_filter, workspace.read_filter);
        assert_eq!(state.selected_feed_urls, workspace.selected_feed_urls);
        assert_eq!(state.grouping_mode, workspace.grouping_mode);
    }

    #[test]
    fn flag_updates_preserve_previous_shared_snapshot() {
        let mut state = EntriesPageState::new(true);
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();
        let snapshot = state.clone();
        assert!(Arc::ptr_eq(&state.entries, &snapshot.entries));
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: Some(true),
                is_starred: None,
            },
        );
        assert!(!snapshot.entries[0].is_read);
        assert!(state.entries[0].is_read);
        assert!(Arc::ptr_eq(&state.entries[1], &snapshot.entries[1]));
    }

    fn patch(entry_id: i64, is_read: Option<bool>, is_starred: Option<bool>) -> EntriesPageIntent {
        EntriesPageIntent::PatchEntryFlags { entry_id, is_read, is_starred }
    }

    /// 查询在途时的成功写入要求重查；没有在途查询时只修补列表，不排队（审计 A3）。
    #[test]
    fn writes_during_an_in_flight_query_request_a_requery() {
        let mut state = EntriesPageState::new(true);
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();

        for _ in 0..3 {
            reduce_entries_page_intent(&mut state, patch(1, None, Some(true)));
            reduce_entries_page_intent(&mut state, patch(1, None, Some(false)));
        }
        assert_eq!(state.requery_revision, 0, "no query in flight, nothing to redo");

        reduce_entries_page_intent(&mut state, EntriesPageIntent::EntriesQueryStarted);
        reduce_entries_page_intent(&mut state, patch(2, Some(true), None));
        assert_eq!(state.requery_revision, 1);
        assert!(state.entries.iter().any(|e| e.id == 2 && e.is_read), "the list is patched now");

        reduce_entries_page_intent(&mut state, EntriesPageIntent::EntriesQueryFinished);
        reduce_entries_page_intent(&mut state, patch(1, Some(true), None));
        assert_eq!(state.requery_revision, 1);
        assert!(!state.query_in_flight);
    }

    /// 另一种完成顺序：点了收藏、随即切到“只看收藏”，查询先读到旧存储、返回不含这一条的
    /// 列表并结束，收藏写入随后才成功。写入后可能符合筛选的缺席条目要求重查；写入后不可能
    /// 符合的（取消收藏）不用（独立评审）。
    #[test]
    fn a_write_that_lands_after_the_query_returned_requests_a_requery() {
        let mut state = EntriesPageState::new(true);
        state.starred_filter = StarredFilter::StarredOnly;
        reduce_entries_page_intent(&mut state, EntriesPageIntent::EntriesQueryStarted);
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetEntries { entries: Vec::new(), archived_count: 0 },
        );
        reduce_entries_page_intent(&mut state, EntriesPageIntent::EntriesQueryFinished);

        reduce_entries_page_intent(&mut state, patch(1, None, Some(false)));
        assert_eq!(state.requery_revision, 0, "an unstarred entry cannot join the list");

        reduce_entries_page_intent(&mut state, patch(1, None, Some(true)));
        assert_eq!(state.requery_revision, 1, "the starred entry must be queried back in");
    }
}
