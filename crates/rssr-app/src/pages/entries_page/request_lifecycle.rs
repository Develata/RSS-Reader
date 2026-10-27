//! Page-local initialization/query rules, independent of the UI executor.
//!
//! The adapter owns task lifetimes and publishes the returned intents. This model
//! owns only request identities; revisions and article data stay in the existing
//! page state. Bootstrap and queries deliberately have separate generations.

use super::{intent::EntriesPageIntent, state::EntriesPageState};

#[derive(Debug, Default)]
pub(super) struct EntriesRequestLifecycle {
    bootstrap_generation: u64,
    query_generation: u64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct BootstrapRequest {
    generation: u64,
    pub(super) load_preferences: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct QueryRequest {
    generation: u64,
    flags_revision: u64,
    reload_revision: u64,
    return_page: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QueryCompletion {
    Discard,
    Reload,
    Accept { return_page: Option<u32> },
}

impl EntriesRequestLifecycle {
    pub(super) fn has_started(&self) -> bool {
        self.bootstrap_generation != 0
    }
    pub(super) fn invalidate_query(&mut self) {
        self.query_generation = self.query_generation.wrapping_add(1);
    }

    pub(super) fn begin_bootstrap(&mut self, state: &EntriesPageState) -> BootstrapRequest {
        self.bootstrap_generation = self.bootstrap_generation.wrapping_add(1);
        BootstrapRequest {
            generation: self.bootstrap_generation,
            load_preferences: !state.preferences_load.is_loaded(),
        }
    }

    pub(super) fn accepts_bootstrap(&self, request: BootstrapRequest) -> bool {
        request.generation == self.bootstrap_generation
    }

    pub(super) fn begin_query(
        &mut self,
        state: &EntriesPageState,
        return_page: Option<u32>,
    ) -> Option<QueryRequest> {
        if !state.preferences_load.can_load_entries() {
            return None;
        }
        self.query_generation = self.query_generation.wrapping_add(1);
        Some(QueryRequest {
            generation: self.query_generation,
            flags_revision: state.flags_revision,
            reload_revision: state.reload_revision,
            return_page,
        })
    }

    pub(super) fn complete_query(
        &self,
        request: QueryRequest,
        state: &EntriesPageState,
    ) -> QueryCompletion {
        // Check identity first: an obsolete query must not schedule another reload.
        if request.generation != self.query_generation {
            return QueryCompletion::Discard;
        }
        if request.flags_revision != state.flags_revision {
            // A completed write may have invalidated the storage snapshot. An
            // existing reload already covers it; otherwise ask the adapter to retry.
            return if request.reload_revision == state.reload_revision {
                QueryCompletion::Reload
            } else {
                QueryCompletion::Discard
            };
        }
        QueryCompletion::Accept { return_page: request.return_page }
    }
}

impl QueryCompletion {
    /// Restore the page only after a successful list has arrived, so the existing
    /// reducer clamps it against the new list. Preserve every other intent's order.
    /// Rejected results are dropped without traversing or copying their payload.
    pub(super) fn intents(
        self,
        intents: impl IntoIterator<Item = EntriesPageIntent>,
    ) -> impl Iterator<Item = EntriesPageIntent> {
        let reload = matches!(self, Self::Reload).then_some(EntriesPageIntent::ReloadEntries);
        let (accepted, return_page) = match self {
            Self::Accept { return_page } => (Some(intents), return_page),
            Self::Discard | Self::Reload => (None, None),
        };
        reload.into_iter().chain(accepted.into_iter().flatten().flat_map(move |intent| {
            let restore = if matches!(&intent, EntriesPageIntent::SetEntries { .. }) {
                return_page.map(EntriesPageIntent::SetCurrentPage)
            } else {
                None
            };
            [Some(intent), restore].into_iter().flatten()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::entries_page::{
        reducer::reduce_entries_page_intent, state::PreferencesLoadState,
    };
    use rssr_domain::{EntryBatchAction, EntrySummary, ReadFilter};

    fn loaded(ids: &[i64]) -> EntriesPageIntent {
        EntriesPageIntent::SetEntries {
            entries: ids
                .iter()
                .map(|&id| EntrySummary {
                    id,
                    feed_id: 1,
                    title: format!("Entry {id}"),
                    feed_title: "Feed".into(),
                    published_at: None,
                    is_read: false,
                    is_starred: false,
                })
                .collect(),
            archived_count: 3,
            truncated: false,
        }
    }

    fn ready_state() -> EntriesPageState {
        let mut state = EntriesPageState::new();
        reduce_entries_page_intent(&mut state, EntriesPageIntent::PreferencesLoaded);
        reduce_entries_page_intent(&mut state, loaded(&[1, 2]));
        state
    }

    fn publish(
        lifecycle: &EntriesRequestLifecycle,
        request: QueryRequest,
        state: &mut EntriesPageState,
        intents: impl IntoIterator<Item = EntriesPageIntent>,
    ) {
        for intent in lifecycle.complete_query(request, state).intents(intents) {
            reduce_entries_page_intent(state, intent);
        }
    }

    #[test]
    fn bootstrap_retries_unavailable_preferences_and_retires_older_requests() {
        let mut lifecycle = EntriesRequestLifecycle::default();
        let mut state = EntriesPageState::new();
        let first = lifecycle.begin_bootstrap(&state);
        assert!(first.load_preferences);
        state.preferences_load =
            PreferencesLoadState::Unavailable { message: "read failed".into(), tone: "error" };
        let retry = lifecycle.begin_bootstrap(&state);
        assert!(retry.load_preferences);
        assert!(!lifecycle.accepts_bootstrap(first));
        assert!(lifecycle.accepts_bootstrap(retry));
        state.preferences_load = PreferencesLoadState::Loaded;
        let refresh = lifecycle.begin_bootstrap(&state);
        assert!(!refresh.load_preferences);
        assert!(!lifecycle.accepts_bootstrap(retry));
        assert!(lifecycle.accepts_bootstrap(refresh));
    }

    #[test]
    fn pending_queries_do_not_start_and_unavailable_preferences_still_allow_reading() {
        let mut lifecycle = EntriesRequestLifecycle::default();
        let mut state = EntriesPageState::new();
        assert!(lifecycle.begin_query(&state, Some(2)).is_none());
        state.preferences_load =
            PreferencesLoadState::Unavailable { message: "read failed".into(), tone: "error" };
        let fallback = lifecycle.begin_query(&state, None).unwrap();
        publish(&lifecycle, fallback, &mut state, [loaded(&[1])]);
        assert_eq!(state.entries.len(), 1);
        assert!(!state.preferences_load.is_loaded());
        assert_eq!(state.status_tone, "error");
        assert!(state.status.contains("read failed"));
        // Refusing a new request must not invalidate a previously admitted one.
        state.preferences_load = PreferencesLoadState::Pending;
        assert!(lifecycle.begin_query(&state, None).is_none());
        assert_eq!(
            lifecycle.complete_query(fallback, &state),
            QueryCompletion::Accept { return_page: None }
        );
    }

    #[test]
    fn bootstrap_and_queries_keep_independent_identities() {
        let mut lifecycle = EntriesRequestLifecycle::default();
        let state = ready_state();
        let bootstrap = lifecycle.begin_bootstrap(&state);
        let query = lifecycle.begin_query(&state, None).unwrap();
        assert!(lifecycle.accepts_bootstrap(bootstrap));
        let next_bootstrap = lifecycle.begin_bootstrap(&state);
        assert_eq!(
            lifecycle.complete_query(query, &state),
            QueryCompletion::Accept { return_page: None }
        );
        let next_query = lifecycle.begin_query(&state, None).unwrap();
        assert!(lifecycle.accepts_bootstrap(next_bootstrap));
        assert!(!lifecycle.accepts_bootstrap(bootstrap));
        assert_eq!(lifecycle.complete_query(query, &state), QueryCompletion::Discard);
        assert_eq!(
            lifecycle.complete_query(next_query, &state),
            QueryCompletion::Accept { return_page: None }
        );
    }

    #[test]
    fn old_success_and_error_cannot_replace_latest_list_or_restore_its_page() {
        for old_result in [
            loaded(&[99]),
            EntriesPageIntent::SetStatus { message: "old error".into(), tone: "error".into() },
        ] {
            let mut lifecycle = EntriesRequestLifecycle::default();
            let mut state = ready_state();
            let old = lifecycle.begin_query(&state, Some(9)).unwrap();
            let latest = lifecycle.begin_query(&state, None).unwrap();
            publish(&lifecycle, latest, &mut state, [loaded(&[3, 4])]);
            let before = state.clone();
            publish(&lifecycle, old, &mut state, [old_result]);
            assert_eq!(state, before);
        }
    }

    #[test]
    fn completed_flag_write_rejects_stale_rows_and_errors_and_requests_one_reload() {
        for read_filter in [ReadFilter::All, ReadFilter::UnreadOnly] {
            for failed in [false, true] {
                let mut lifecycle = EntriesRequestLifecycle::default();
                let mut state = ready_state();
                reduce_entries_page_intent(
                    &mut state,
                    EntriesPageIntent::SetReadFilter(read_filter),
                );
                let request = lifecycle.begin_query(&state, Some(8)).unwrap();
                reduce_entries_page_intent(
                    &mut state,
                    EntriesPageIntent::PatchEntryFlags {
                        entry_id: 1,
                        is_read: Some(true),
                        is_starred: Some(true),
                    },
                );
                let before = state.clone();
                let result = if failed {
                    EntriesPageIntent::SetStatus {
                        message: "stale error".into(),
                        tone: "error".into(),
                    }
                } else {
                    loaded(&[1, 2])
                };
                publish(&lifecycle, request, &mut state, [result]);
                assert_eq!(state.entries, before.entries);
                assert_eq!(state.current_page, before.current_page);
                assert_eq!(state.status, before.status);
                assert_eq!(state.reload_revision, before.reload_revision + 1);
                assert_eq!(lifecycle.complete_query(request, &state), QueryCompletion::Discard);
                let retry = lifecycle.begin_query(&state, None).unwrap();
                publish(&lifecycle, retry, &mut state, [loaded(&[3])]);
                assert_eq!(state.entries[0].id, 3);
            }
        }
    }

    #[test]
    fn bulk_write_reload_is_not_duplicated_and_old_rows_do_not_reappear() {
        let mut lifecycle = EntriesRequestLifecycle::default();
        let mut state = ready_state();
        let request = lifecycle.begin_query(&state, None).unwrap();
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::SelectionApplied {
                ids: vec![1],
                action: EntryBatchAction::Archive,
                count: 1,
            },
        );
        let before = state.clone();
        publish(&lifecycle, request, &mut state, [loaded(&[1, 2])]);
        assert_eq!(state, before);
    }

    #[test]
    fn obsolete_query_cannot_request_reload_even_after_a_write() {
        let mut lifecycle = EntriesRequestLifecycle::default();
        let mut state = ready_state();
        let old = lifecycle.begin_query(&state, None).unwrap();
        lifecycle.begin_query(&state, None).unwrap();
        reduce_entries_page_intent(
            &mut state,
            EntriesPageIntent::PatchEntryFlags {
                entry_id: 1,
                is_read: None,
                is_starred: Some(true),
            },
        );
        assert_eq!(lifecycle.complete_query(old, &state), QueryCompletion::Discard);
        assert_eq!(state.reload_revision, 0);
    }

    #[test]
    fn reload_revision_alone_does_not_invalidate_an_otherwise_current_query() {
        let mut lifecycle = EntriesRequestLifecycle::default();
        let mut state = ready_state();
        let request = lifecycle.begin_query(&state, None).unwrap();
        reduce_entries_page_intent(&mut state, EntriesPageIntent::ReloadEntries);
        publish(&lifecycle, request, &mut state, [loaded(&[3])]);
        assert_eq!(state.entries[0].id, 3);
    }

    #[test]
    fn successful_list_restores_page_before_following_intents_and_clamps_to_new_rows() {
        let mut lifecycle = EntriesRequestLifecycle::default();
        let mut state = ready_state();
        state.entries_page_size = 1;
        let request = lifecycle.begin_query(&state, Some(9)).unwrap();
        let completion = lifecycle.complete_query(request, &state);
        let intents = completion
            .intents([
                loaded(&[3, 4]),
                EntriesPageIntent::SetStatus {
                    message: "latest status".into(),
                    tone: "info".into(),
                },
            ])
            .collect::<Vec<_>>();
        assert!(matches!(
            intents.as_slice(),
            [
                EntriesPageIntent::SetEntries { .. },
                EntriesPageIntent::SetCurrentPage(9),
                EntriesPageIntent::SetStatus { .. },
            ]
        ));
        for intent in intents {
            reduce_entries_page_intent(&mut state, intent);
        }
        assert_eq!(state.current_page, 2);
        assert_eq!(state.archived_count, 3);
        assert_eq!(state.status, "latest status");
        let empty = lifecycle.begin_query(&state, Some(9)).unwrap();
        publish(&lifecycle, empty, &mut state, [loaded(&[])]);
        assert_eq!(state.current_page, 1);
    }

    #[test]
    fn failed_query_does_not_restore_page_and_rejected_payload_is_not_traversed() {
        let mut lifecycle = EntriesRequestLifecycle::default();
        let mut state = ready_state();
        let request = lifecycle.begin_query(&state, Some(9)).unwrap();
        publish(
            &lifecycle,
            request,
            &mut state,
            [EntriesPageIntent::SetStatus { message: "latest error".into(), tone: "error".into() }],
        );
        assert_eq!(state.current_page, 1);
        assert_eq!(state.status, "latest error");
        for completion in [QueryCompletion::Discard, QueryCompletion::Reload] {
            let never_read = std::iter::from_fn(|| panic!("discarded payload was traversed"));
            let intents = completion.intents(never_read).collect::<Vec<_>>();
            assert_eq!(intents.len(), usize::from(completion == QueryCompletion::Reload));
            assert!(
                intents.iter().all(|intent| matches!(intent, EntriesPageIntent::ReloadEntries))
            );
        }
    }
}
