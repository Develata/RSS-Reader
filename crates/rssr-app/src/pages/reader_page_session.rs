use dioxus::prelude::*;

use crate::bootstrap::ReaderNavigation;

use super::{
    reader_page_bindings::ReaderPageBindings, reader_page_effect::ReaderPageEffect,
    reader_page_runtime::execute_reader_page_effect, reader_page_state::ReaderPageState,
};

#[derive(Clone, Copy)]
pub(crate) struct ReaderPageSession {
    entry_id: i64,
    state: Signal<ReaderPageState>,
    bindings: ReaderPageBindings,
}

impl ReaderPageSession {
    pub(crate) fn new(entry_id: i64, state: Signal<ReaderPageState>) -> Self {
        Self { entry_id, state, bindings: ReaderPageBindings::new(state) }
    }

    pub(crate) fn snapshot(self) -> ReaderPageState {
        (self.state)()
    }

    pub(crate) fn reload_tick(self) -> u64 {
        (self.state)().reload_tick
    }

    pub(crate) fn previous_action_target(self) -> Option<i64> {
        previous_action_target((self.state)().navigation_state)
    }

    pub(crate) fn next_action_target(self) -> Option<i64> {
        next_action_target((self.state)().navigation_state)
    }

    pub(crate) fn load(self) {
        let entry_id = self.entry_id;
        let bindings = self.bindings;
        spawn(async move {
            let outcome = execute_reader_page_effect(ReaderPageEffect::LoadEntry(entry_id)).await;
            bindings.apply_runtime_outcome(outcome);
        });
    }

    pub(crate) fn toggle_read(self, via_shortcut: bool) {
        let entry_id = self.entry_id;
        let currently_read = (self.state)().is_read;
        let bindings = self.bindings;
        spawn(async move {
            let outcome = execute_reader_page_effect(ReaderPageEffect::ToggleRead {
                entry_id,
                currently_read,
                via_shortcut,
            })
            .await;
            bindings.apply_runtime_outcome(outcome);
        });
    }

    pub(crate) fn toggle_starred(self, via_shortcut: bool) {
        let entry_id = self.entry_id;
        let currently_starred = (self.state)().is_starred;
        let bindings = self.bindings;
        spawn(async move {
            let outcome = execute_reader_page_effect(ReaderPageEffect::ToggleStarred {
                entry_id,
                currently_starred,
                via_shortcut,
            })
            .await;
            bindings.apply_runtime_outcome(outcome);
        });
    }
}

fn previous_action_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.previous_unread_entry_id.or(navigation.previous_feed_entry_id)
}

fn next_action_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.next_unread_entry_id.or(navigation.next_feed_entry_id)
}
