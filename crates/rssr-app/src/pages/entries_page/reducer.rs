use dioxus::prelude::*;
use rssr_domain::{ReadFilter, StarredFilter};

use super::{
    intent::EntriesPageIntent, state::EntriesPageState, state::entry_grouping_mode_from_preference,
    state::FIRST_PAGE_NUMBER,
};

pub(crate) fn dispatch_entries_page_intent(
    mut state: Signal<EntriesPageState>,
    intent: EntriesPageIntent,
) {
    state.with_mut(|state| reduce_entries_page_intent(state, intent));
}

pub(crate) fn reduce_entries_page_intent(state: &mut EntriesPageState, intent: EntriesPageIntent) {
    match intent {
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
            state.preferences_loaded = true;
        }
        EntriesPageIntent::SetFeeds(feeds) => state.feeds = feeds,
        EntriesPageIntent::SetEntries(entries) => {
            state.status = format!("共 {} 篇文章。", entries.len());
            state.status_tone = "info".to_string();
            state.entries = entries;
            clamp_current_page(state);
        }
        EntriesPageIntent::PatchEntryFlags { entry_id, is_read, is_starred } => {
            if let Some(entry) = state.entries.iter_mut().find(|entry| entry.id == entry_id) {
                if let Some(is_read) = is_read {
                    entry.is_read = is_read;
                }
                if let Some(is_starred) = is_starred {
                    entry.is_starred = is_starred;
                }
            }
            let read_filter = state.read_filter;
            let starred_filter = state.starred_filter;
            state
                .entries
                .retain(|entry| matches_current_filters(read_filter, starred_filter, entry));
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
        EntriesPageIntent::SetCurrentPage(page) => state.current_page = page.max(FIRST_PAGE_NUMBER),
        EntriesPageIntent::GoToNextPage => state.current_page = state.current_page.saturating_add(1),
        EntriesPageIntent::GoToPreviousPage => {
            state.current_page = state.current_page.saturating_sub(1).max(FIRST_PAGE_NUMBER);
        }
        EntriesPageIntent::SetControlsHidden(hidden) => state.controls_hidden = hidden,
        EntriesPageIntent::ToggleDirectorySource(anchor_id) => {
            if !state.expanded_directory_sources.insert(anchor_id.clone()) {
                state.expanded_directory_sources.remove(&anchor_id);
            }
        }
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
    fn patch_entry_flags_removes_entry_that_no_longer_matches_filter() {
        let mut state = EntriesPageState::new(true);
        state.read_filter = ReadFilter::UnreadOnly;
        state.entries = vec![entry(1, false)];

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
    fn set_entries_resets_render_limit() {
        let mut state = EntriesPageState::new(true);
        state.current_page = 9;

        reduce_entries_page_intent(&mut state, EntriesPageIntent::SetEntries(vec![entry(1, false)]));

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
}
