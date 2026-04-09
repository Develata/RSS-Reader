use dioxus::prelude::*;

use super::{
    reducer::dispatch_entries_page_intent, runtime::EntriesPageRuntimeOutcome,
    state::EntriesPageState,
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct EntriesPageBindings {
    state: Signal<EntriesPageState>,
}

impl EntriesPageBindings {
    pub(crate) fn new(state: Signal<EntriesPageState>) -> Self {
        Self { state }
    }

    pub(crate) fn apply_runtime_outcome(self, outcome: EntriesPageRuntimeOutcome) {
        for intent in outcome.intents {
            dispatch_entries_page_intent(self.state, intent);
        }
    }
}
