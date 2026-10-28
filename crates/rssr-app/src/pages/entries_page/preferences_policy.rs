//! Eligibility for the existing preference save task. Persistence serialization
//! belongs to EntriesWorkspaceService; this policy owns no queue or state copy.

use rssr_domain::EntriesWorkspaceState;

use super::state::{EntriesPageState, grouping_mode_preference};

pub(super) fn can_schedule_save(state: &EntriesPageState, captured_loaded: bool) -> bool {
    captured_loaded && state.preferences_load.is_loaded()
}

pub(super) fn is_current_save(state: &EntriesPageState, next: &EntriesWorkspaceState) -> bool {
    state.preferences_load.is_loaded()
        && grouping_mode_preference(state.grouping_mode) == next.grouping_mode
        && state.show_archived == next.show_archived
        && state.read_filter == next.read_filter
        && state.starred_filter == next.starred_filter
        && state.selected_feed_urls == next.selected_feed_urls
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::entries_page::{
        intent::EntriesPageIntent,
        reducer::reduce_entries_page_intent as reduce,
        state::{EntryGroupingMode, PreferencesLoadState},
    };
    use rssr_domain::{ReadFilter, StarredFilter};

    #[test]
    fn scheduling_requires_both_captured_and_live_readiness() {
        let mut state = EntriesPageState::new();
        for readiness in [
            PreferencesLoadState::Pending,
            PreferencesLoadState::Unavailable("failed".into()),
            PreferencesLoadState::Loaded,
        ] {
            state.preferences_load = readiness;
            assert!(!can_schedule_save(&state, false));
            assert_eq!(can_schedule_save(&state, true), state.preferences_load.is_loaded());
        }
    }

    #[test]
    fn readiness_is_rechecked_when_the_queued_task_runs() {
        let mut state = EntriesPageState::new();
        state.preferences_load = PreferencesLoadState::Loaded;
        assert!(can_schedule_save(&state, true));
        let next = EntriesWorkspaceState::default();
        for readiness in
            [PreferencesLoadState::Pending, PreferencesLoadState::Unavailable("failed".into())]
        {
            state.preferences_load = readiness;
            assert!(!is_current_save(&state, &next));
        }
    }

    #[test]
    fn every_browsing_preference_participates_in_the_staleness_check() {
        let mut current = EntriesPageState::new();
        current.preferences_load = PreferencesLoadState::Loaded;
        let captured = EntriesWorkspaceState::default();
        assert!(is_current_save(&current, &captured));
        for change in [
            EntriesPageIntent::SetGroupingMode(EntryGroupingMode::Source),
            EntriesPageIntent::SetShowArchived(!current.show_archived),
            EntriesPageIntent::SetReadFilter(ReadFilter::UnreadOnly),
            EntriesPageIntent::SetStarredFilter(StarredFilter::StarredOnly),
            EntriesPageIntent::SetSelectedFeedUrls(vec!["https://example.com/feed".into()]),
        ] {
            let mut changed = current.clone();
            reduce(&mut changed, change);
            assert!(!is_current_save(&changed, &captured));
        }
    }

    #[test]
    fn rapid_return_to_all_rejects_the_intermediate_choice() {
        let mut state = EntriesPageState::new();
        state.preferences_load = PreferencesLoadState::Loaded;
        let all = EntriesWorkspaceState::default();
        let unread = EntriesWorkspaceState { read_filter: ReadFilter::UnreadOnly, ..all.clone() };
        reduce(&mut state, EntriesPageIntent::SetReadFilter(ReadFilter::UnreadOnly));
        assert!(is_current_save(&state, &unread));
        reduce(&mut state, EntriesPageIntent::SetReadFilter(ReadFilter::All));
        assert!(!is_current_save(&state, &unread));
        assert!(is_current_save(&state, &all));
    }

    #[test]
    fn source_url_order_and_identity_are_preserved() {
        let mut state = EntriesPageState::new();
        state.preferences_load = PreferencesLoadState::Loaded;
        state.selected_feed_urls =
            vec!["https://example.com/a".into(), "https://example.com/b".into()];
        let mut next = EntriesWorkspaceState {
            selected_feed_urls: state.selected_feed_urls.clone(),
            ..Default::default()
        };
        assert!(is_current_save(&state, &next));
        next.selected_feed_urls.reverse();
        assert!(!is_current_save(&state, &next));
        assert_eq!(state.selected_feed_urls[0], "https://example.com/a");
    }
}
