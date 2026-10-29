//! Page-local write decisions. The existing reducer owns busy state and revisions;
//! the session executes tasks and publishes these ordered effects.

use rssr_domain::{EntryBatchAction, EntryBatchRequest};

use super::{intent::EntriesPageIntent, state::EntriesPageState};

pub(super) enum WriteEffect {
    Publish(EntriesPageIntent),
    Reread,
}

pub(super) struct EntryWrite {
    entry_id: i64,
}

impl EntryWrite {
    pub(super) fn prepare(state: &EntriesPageState, entry_id: i64) -> Option<Self> {
        (!state.bulk_busy && !state.pending_entry_writes.contains(&entry_id))
            .then_some(Self { entry_id })
    }

    pub(super) fn started(&self) -> EntriesPageIntent {
        EntriesPageIntent::EntryWriteStarted(self.entry_id)
    }

    pub(super) fn complete(
        self,
        intents: impl IntoIterator<Item = EntriesPageIntent>,
    ) -> impl Iterator<Item = WriteEffect> {
        let mut intents = intents.into_iter().fuse();
        let mut finished = Some(EntriesPageIntent::EntryWriteFinished(self.entry_id));
        let mut refresh_sources = false;
        std::iter::from_fn(move || {
            if let Some(intent) = intents.next() {
                refresh_sources |=
                    matches!(&intent, EntriesPageIntent::PatchEntryFlags { is_read: Some(_), .. });
                return Some(WriteEffect::Publish(intent));
            }
            if let Some(finished) = finished.take() {
                return Some(WriteEffect::Publish(finished));
            }
            // Both success and failure unlock the row before any read recovery.
            std::mem::take(&mut refresh_sources).then_some(WriteEffect::Reread)
        })
    }
}

pub(super) struct BulkWrite {
    generation: u64,
}

impl BulkWrite {
    pub(super) fn selection(
        state: &EntriesPageState,
        action: EntryBatchAction,
    ) -> Option<EntryBatchRequest> {
        (!state.selected_entry_ids.is_empty()).then(|| EntryBatchRequest {
            entry_ids: state.selected_entry_ids.iter().copied().collect(),
            action,
        })
    }

    pub(super) fn can_start(state: &EntriesPageState) -> bool {
        !state.bulk_busy && state.pending_entry_writes.is_empty()
    }

    /// Capture after the existing BeginBulk reducer has assigned the generation.
    pub(super) fn capture_started(state: &EntriesPageState) -> Self {
        Self { generation: state.bulk_generation }
    }

    pub(super) fn complete<I: IntoIterator<Item = EntriesPageIntent>>(
        self,
        state: &EntriesPageState,
        intents: I,
    ) -> impl Iterator<Item = WriteEffect> + use<I> {
        let accepted = (self.generation == state.bulk_generation).then_some(intents);
        accepted.into_iter().flatten().flat_map(|intent| {
            let refresh = matches!(&intent, EntriesPageIntent::SelectionApplied { .. });
            [Some(WriteEffect::Publish(intent)), refresh.then_some(WriteEffect::Reread)]
                .into_iter()
                .flatten()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::entries_page::reducer::reduce_entries_page_intent as reduce;
    use rssr_domain::{EntrySummary, ReadFilter};

    fn state() -> EntriesPageState {
        let mut state = EntriesPageState::new();
        reduce(&mut state, EntriesPageIntent::PreferencesLoaded);
        reduce(
            &mut state,
            EntriesPageIntent::SetEntries {
                entries: (1..=3)
                    .map(|id| EntrySummary {
                        id,
                        feed_id: 1,
                        title: format!("Entry {id}"),
                        feed_title: "Feed".into(),
                        published_at: None,
                        is_read: false,
                        is_starred: false,
                    })
                    .collect(),
                archived_count: 0,
                truncated: false,
            },
        );
        state
    }

    fn start_bulk(state: &mut EntriesPageState, writing: bool) -> BulkWrite {
        assert!(BulkWrite::can_start(state));
        reduce(state, EntriesPageIntent::BeginBulk { writing });
        BulkWrite::capture_started(state)
    }

    fn apply(state: &mut EntriesPageState, effects: impl Iterator<Item = WriteEffect>) -> usize {
        let mut refreshes = 0;
        for effect in effects {
            match effect {
                WriteEffect::Publish(intent) => reduce(state, intent),
                WriteEffect::Reread => refreshes += 1,
            }
        }
        refreshes
    }

    #[test]
    fn separate_rows_can_write_while_duplicates_and_batches_wait() {
        let mut state = state();
        let first = EntryWrite::prepare(&state, 1).unwrap();
        reduce(&mut state, first.started());
        assert!(EntryWrite::prepare(&state, 1).is_none());
        assert!(!BulkWrite::can_start(&state));
        let second = EntryWrite::prepare(&state, 2).unwrap();
        reduce(&mut state, second.started());
        apply(&mut state, first.complete([]));
        assert!(EntryWrite::prepare(&state, 1).is_some());
        assert!(EntryWrite::prepare(&state, 2).is_none());
        assert!(!BulkWrite::can_start(&state));
        apply(&mut state, second.complete([]));
        assert!(BulkWrite::can_start(&state));
        let _batch = start_bulk(&mut state, true);
        assert!(EntryWrite::prepare(&state, 3).is_none());
        assert!(!BulkWrite::can_start(&state));
    }

    #[test]
    fn read_success_publishes_results_then_unlocks_then_refreshes_once() {
        let mut state = state();
        let request = EntryWrite::prepare(&state, 1).unwrap();
        reduce(&mut state, request.started());
        let effects = request
            .complete([
                EntriesPageIntent::SetStatus { message: "saved".into(), tone: "success".into() },
                EntriesPageIntent::PatchEntryFlags {
                    entry_id: 1,
                    is_read: Some(true),
                    is_starred: None,
                },
            ])
            .collect::<Vec<_>>();
        assert!(matches!(
            effects.as_slice(),
            [
                WriteEffect::Publish(EntriesPageIntent::SetStatus { .. }),
                WriteEffect::Publish(EntriesPageIntent::PatchEntryFlags { .. }),
                WriteEffect::Publish(EntriesPageIntent::EntryWriteFinished(1)),
                WriteEffect::Reread,
            ]
        ));
        assert_eq!(apply(&mut state, effects.into_iter()), 1);
        assert!(state.entries[0].is_read);
        assert!(state.pending_entry_writes.is_empty());
    }

    #[test]
    fn failed_and_starred_writes_unlock_without_refreshing_sources() {
        for failed in [false, true] {
            let mut state = state();
            let request = EntryWrite::prepare(&state, 1).unwrap();
            reduce(&mut state, request.started());
            let result = if failed {
                EntriesPageIntent::SetStatus { message: "failed".into(), tone: "error".into() }
            } else {
                EntriesPageIntent::PatchEntryFlags {
                    entry_id: 1,
                    is_read: None,
                    is_starred: Some(true),
                }
            };
            assert_eq!(apply(&mut state, request.complete([result])), 0);
            assert_eq!(state.entries[0].is_starred, !failed);
            assert!(!state.entries[0].is_read);
            assert!(EntryWrite::prepare(&state, 1).is_some());
        }
    }

    #[test]
    fn batch_selection_is_an_owned_snapshot_of_only_the_selected_ids() {
        let mut state = state();
        assert!(BulkWrite::selection(&state, EntryBatchAction::Archive).is_none());
        reduce(&mut state, EntriesPageIntent::ToggleSelected(1));
        reduce(&mut state, EntriesPageIntent::ToggleSelected(3));
        let request = BulkWrite::selection(&state, EntryBatchAction::Archive).unwrap();
        reduce(&mut state, EntriesPageIntent::ClearSelection);
        reduce(&mut state, EntriesPageIntent::ToggleSelected(2));
        assert_eq!(request.entry_ids, [1, 3]);
        assert_eq!(request.action, EntryBatchAction::Archive);
    }

    #[test]
    fn filter_changes_do_not_cancel_a_committing_batch() {
        let mut state = state();
        reduce(&mut state, EntriesPageIntent::ToggleSelected(1));
        let request = start_bulk(&mut state, true);
        reduce(&mut state, EntriesPageIntent::SetReadFilter(ReadFilter::UnreadOnly));
        reduce(&mut state, EntriesPageIntent::CancelBulk);
        assert!(state.bulk_busy);
        let effects = request.complete(
            &state,
            [EntriesPageIntent::SelectionApplied {
                ids: vec![1],
                action: EntryBatchAction::MarkRead,
                count: 1,
            }],
        );
        assert_eq!(apply(&mut state, effects), 1);
        assert!(!state.bulk_busy);
        assert_eq!(state.reload_revision, 1);
        assert_eq!(state.flags_revision, 1);
        assert_eq!(state.read_filter, ReadFilter::UnreadOnly);
    }

    #[test]
    fn retired_batch_success_and_failure_cannot_unlock_a_later_batch() {
        for failed in [false, true] {
            let mut state = state();
            let old = start_bulk(&mut state, false);
            reduce(&mut state, EntriesPageIntent::CancelBulk);
            let _current = start_bulk(&mut state, true);
            let before = state.clone();
            let result = if failed {
                EntriesPageIntent::BulkFailed(crate::ui::failure::plain("old failure"))
            } else {
                EntriesPageIntent::SelectionApplied {
                    ids: vec![1],
                    action: EntryBatchAction::Archive,
                    count: 1,
                }
            };
            let effects = old.complete(&state, [result]);
            assert_eq!(apply(&mut state, effects), 0);
            assert_eq!(state, before);
        }
    }

    #[test]
    fn failed_batch_retains_selection_and_unlocks_retry_without_refresh() {
        let mut state = state();
        reduce(&mut state, EntriesPageIntent::ToggleSelected(1));
        let request = start_bulk(&mut state, true);
        let effects = request
            .complete(&state, [EntriesPageIntent::BulkFailed(crate::ui::failure::plain("failed"))]);
        assert_eq!(apply(&mut state, effects), 0);
        assert_eq!(state.selected_entry_ids.iter().copied().collect::<Vec<_>>(), [1]);
        assert!(BulkWrite::can_start(&state));
        assert_eq!(state.status, "测试失败：failed");
        assert_eq!(state.flags_revision, 0);
        assert_eq!(state.reload_revision, 0);
    }

    #[test]
    fn batch_success_refreshes_after_publication_before_following_intents() {
        let mut state = state();
        let request = start_bulk(&mut state, true);
        let effects = request
            .complete(
                &state,
                [
                    EntriesPageIntent::SelectionApplied {
                        ids: vec![1],
                        action: EntryBatchAction::Archive,
                        count: 1,
                    },
                    EntriesPageIntent::SetStatus { message: "next".into(), tone: "info".into() },
                ],
            )
            .collect::<Vec<_>>();
        assert!(matches!(
            effects.as_slice(),
            [
                WriteEffect::Publish(EntriesPageIntent::SelectionApplied { .. }),
                WriteEffect::Reread,
                WriteEffect::Publish(EntriesPageIntent::SetStatus { .. }),
            ]
        ));
    }

    #[test]
    fn rejected_batch_does_not_traverse_its_payload() {
        let mut state = state();
        let request = start_bulk(&mut state, false);
        reduce(&mut state, EntriesPageIntent::CancelBulk);
        let never_read = std::iter::from_fn(|| panic!("obsolete batch payload was read"));
        assert_eq!(request.complete(&state, never_read).count(), 0);
    }
}
