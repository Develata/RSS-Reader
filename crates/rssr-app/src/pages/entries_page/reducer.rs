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
    let previous_page = state.current_page;
    if matches!(&intent, EntriesPageIntent::SetGroupingMode(mode) if *mode == state.grouping_mode) {
        return;
    }
    if matches!(
        &intent,
        EntriesPageIntent::ClearSelection
            | EntriesPageIntent::ClearFilters
            | EntriesPageIntent::SetGroupingMode(_)
            | EntriesPageIntent::SetShowArchived(_)
            | EntriesPageIntent::SetReadFilter(_)
            | EntriesPageIntent::SetStarredFilter(_)
            | EntriesPageIntent::SetSelectedFeedUrls(_)
            | EntriesPageIntent::ApplyLoadedWorkspaceState(_)
    ) {
        Arc::make_mut(&mut state.selected_entry_ids).clear();
    }
    match intent {
        EntriesPageIntent::ClearSelection => {}
        EntriesPageIntent::ToggleSelected(id) => {
            if !state.bulk_busy && state.current_page_entries().iter().any(|entry| entry.id == id) {
                let selected = Arc::make_mut(&mut state.selected_entry_ids);
                if !selected.remove(&id) {
                    selected.insert(id);
                }
            }
        }
        EntriesPageIntent::TogglePageSelection => {
            if !state.bulk_busy {
                let ids = state
                    .current_page_entries()
                    .iter()
                    .map(|entry| entry.id)
                    .collect::<std::collections::BTreeSet<_>>();
                state.selected_entry_ids = Arc::new(if ids == *state.selected_entry_ids {
                    Default::default()
                } else {
                    ids
                });
            }
        }
        EntriesPageIntent::SelectionApplied { ids, action, count } => {
            state.bulk_busy = false;
            state.bulk_writing = false;
            state.bulk_revision = state.bulk_revision.wrapping_add(1);
            let selected = Arc::make_mut(&mut state.selected_entry_ids);
            for id in ids {
                selected.remove(&id);
            }
            let verb = match action {
                rssr_domain::EntryBatchAction::MarkRead => "标为已读",
                rssr_domain::EntryBatchAction::Archive => "归档",
            };
            let message = format!("已{verb} {count} 篇所选文章。");
            state.bulk_notice = Some(message.clone());
            state.status = message;
            state.status_tone = "success".into();
        }
        EntriesPageIntent::BeginBulk { writing } => {
            state.bulk_generation = state.bulk_generation.wrapping_add(1);
            state.bulk_busy = true;
            state.bulk_writing = writing;
        }
        EntriesPageIntent::CancelBulk => {
            if !state.bulk_writing {
                state.bulk_generation = state.bulk_generation.wrapping_add(1);
                state.bulk_busy = false;
            }
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
        EntriesPageIntent::SetEntries { entries, archived_count } => {
            state.entries_loaded = true;
            if let PreferencesLoadState::Unavailable(message) = &state.preferences_load {
                state.status =
                    format!("{message}；文章偏好暂不保存，共 {} 篇文章。", entries.len());
                state.status_tone = "error".to_string();
            } else {
                state.status = format!("共 {} 篇文章。", entries.len());
                state.status_tone = "info".to_string();
            }
            if let Some(message) = state.bulk_notice.take() {
                state.status = message;
                state.status_tone = "success".into();
            }
            state.entries = Arc::new(entries.into_iter().map(Arc::new).collect());
            state.archived_count = archived_count;
            clamp_current_page(state);
        }
        EntriesPageIntent::PatchEntryFlags { entry_id, is_read, is_starred } => {
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
    }
    if previous_page != state.current_page {
        Arc::make_mut(&mut state.selected_entry_ids).clear();
    } else if !state.selected_entry_ids.is_empty() {
        // Refresh/flag updates may remove rows. Never leave invisible articles selected.
        let start = ((state.current_page.saturating_sub(1) as usize) * state.page_size())
            .min(state.entries.len());
        let end = (start + state.page_size()).min(state.entries.len());
        let entries = &state.entries[start..end];
        Arc::make_mut(&mut state.selected_entry_ids)
            .retain(|id| entries.iter().any(|entry| entry.id == *id));
    }
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

    use rssr_domain::{EntrySummary, ReadFilter};

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
    fn selection_is_page_scoped_and_same_page_directory_navigation_preserves_it() {
        let mut state = EntriesPageState::new();
        state.entries_page_size = 2;
        state.entries = (1..=4).map(|id| Arc::new(entry(id, false))).collect::<Vec<_>>().into();
        reduce_entries_page_intent(&mut state, EntriesPageIntent::ToggleSelected(3));
        assert!(state.selected_entry_ids.is_empty(), "cannot select an invisible row");
        reduce_entries_page_intent(&mut state, EntriesPageIntent::TogglePageSelection);
        assert_eq!(state.selected_entry_ids.iter().copied().collect::<Vec<_>>(), vec![1, 2]);
        reduce_entries_page_intent(&mut state, EntriesPageIntent::SetCurrentPage(1));
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: Some(true),
                is_starred: None,
            },
        );
        assert_eq!(
            state.selected_entry_ids.len(),
            2,
            "same page jump and metadata updates preserve selection"
        );
        reduce_entries_page_intent(&mut state, EntriesPageIntent::SetCurrentPage(2));
        assert!(state.selected_entry_ids.is_empty());
        reduce_entries_page_intent(&mut state, EntriesPageIntent::TogglePageSelection);
        assert_eq!(state.selected_entry_ids.iter().copied().collect::<Vec<_>>(), vec![3, 4]);
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetReadFilter(ReadFilter::UnreadOnly),
        );
        assert!(state.selected_entry_ids.is_empty());
    }

    #[test]
    fn selection_is_reconciled_when_a_selected_row_leaves_the_filter() {
        let mut state = EntriesPageState::new();
        state.read_filter = ReadFilter::UnreadOnly;
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();
        reduce_entries_page_intent(&mut state, EntriesPageIntent::TogglePageSelection);
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: Some(true),
                is_starred: None,
            },
        );
        assert_eq!(state.selected_entry_ids.iter().copied().collect::<Vec<_>>(), vec![2]);
        reduce_entries_page_intent(&mut state, EntriesPageIntent::BeginBulk { writing: true });
        reduce_entries_page_intent(&mut state, EntriesPageIntent::ToggleSelected(2));
        assert_eq!(
            state.selected_entry_ids.len(),
            1,
            "an in-flight batch freezes selection controls"
        );
    }

    #[test]
    fn patch_entry_flags_removes_entry_that_no_longer_matches_filter() {
        let mut state = EntriesPageState::new();
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
        let mut state = EntriesPageState::new();
        state.current_page = 9;

        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetEntries { entries: vec![entry(1, false)], archived_count: 0 },
        );

        assert_eq!(state.current_page, FIRST_PAGE_NUMBER);
    }

    #[test]
    fn changing_filters_resets_current_page() {
        let mut state = EntriesPageState::new();
        state.current_page = 4;

        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SetStarredFilter(rssr_domain::StarredFilter::StarredOnly),
        );

        assert_eq!(state.current_page, FIRST_PAGE_NUMBER);
    }

    #[test]
    fn changing_page_updates_current_page() {
        let mut state = EntriesPageState::new();
        state.entries_page_size = 1;
        state.entries = vec![Arc::new(entry(1, false)), Arc::new(entry(2, false))].into();

        reduce_entries_page_intent(&mut state, EntriesPageIntent::SetCurrentPage(2));

        assert_eq!(state.current_page, 2);
    }

    #[test]
    fn pagination_clamps_and_preserves_filters_and_source_selection() {
        let mut state = EntriesPageState::new();
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
        let mut state = EntriesPageState::new();
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
}
