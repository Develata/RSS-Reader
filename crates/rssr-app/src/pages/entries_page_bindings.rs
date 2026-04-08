use dioxus::prelude::*;

use super::entries_page_commands::EntriesPageCommandOutcome;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct EntriesPageBindings {
    reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
}

impl EntriesPageBindings {
    pub(crate) fn new(
        reload_tick: Signal<u64>,
        status: Signal<String>,
        status_tone: Signal<String>,
    ) -> Self {
        Self { reload_tick, status, status_tone }
    }

    pub(crate) fn apply_command_outcome(mut self, outcome: EntriesPageCommandOutcome) {
        if let Some(message) = outcome.status_message {
            self.status.set(message);
        }
        if let Some(tone) = outcome.status_tone {
            self.status_tone.set(tone.to_string());
        }
        if outcome.reload {
            let mut reload_tick = self.reload_tick;
            reload_tick += 1;
        }
    }
}
