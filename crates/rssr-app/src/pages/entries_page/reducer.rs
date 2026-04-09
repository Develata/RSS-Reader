use dioxus::prelude::*;

use super::{
    intent::EntriesPageIntent, state::EntriesPageState, state::entry_grouping_mode_from_preference,
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
            state.read_filter = settings.entry_read_filter;
            state.starred_filter = settings.entry_starred_filter;
            state.selected_feed_urls = settings.entry_filtered_feed_urls;
            state.show_archived = settings.show_archived_entries;
            state.grouping_mode = entry_grouping_mode_from_preference(settings.entry_grouping_mode);
            state.preferences_loaded = true;
        }
        EntriesPageIntent::SetFeeds(feeds) => state.feeds = feeds,
        EntriesPageIntent::SetEntries(entries) => {
            state.status = format!("共 {} 篇文章。", entries.len());
            state.status_tone = "info".to_string();
            state.entries = entries;
        }
        EntriesPageIntent::SetStatus { message, tone } => {
            state.status = message;
            state.status_tone = tone;
        }
        EntriesPageIntent::SetGroupingMode(grouping_mode) => state.grouping_mode = grouping_mode,
        EntriesPageIntent::SetShowArchived(show_archived) => state.show_archived = show_archived,
        EntriesPageIntent::SetReadFilter(read_filter) => state.read_filter = read_filter,
        EntriesPageIntent::SetStarredFilter(starred_filter) => {
            state.starred_filter = starred_filter;
        }
        EntriesPageIntent::SetSelectedFeedUrls(selected_feed_urls) => {
            state.selected_feed_urls = selected_feed_urls;
        }
        EntriesPageIntent::SetControlsHidden(hidden) => state.controls_hidden = hidden,
        EntriesPageIntent::ToggleDirectorySource(anchor_id) => {
            if !state.expanded_directory_sources.insert(anchor_id.clone()) {
                state.expanded_directory_sources.remove(&anchor_id);
            }
        }
        EntriesPageIntent::BumpReload => state.reload_tick += 1,
    }
}
