use dioxus::prelude::*;

use super::{
    reader_page_reducer::dispatch_reader_page_intent,
    reader_page_runtime::ReaderPageRuntimeOutcome, reader_page_state::ReaderPageState,
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ReaderPageBindings {
    state: Signal<ReaderPageState>,
}

impl ReaderPageBindings {
    pub(crate) fn new(state: Signal<ReaderPageState>) -> Self {
        Self { state }
    }

    pub(crate) fn apply_runtime_outcome(self, outcome: ReaderPageRuntimeOutcome) {
        for intent in outcome.intents {
            dispatch_reader_page_intent(self.state, intent);
        }
    }
}
