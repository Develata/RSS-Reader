use dioxus::prelude::Callback;

use super::{session::ReaderPageSession, state::ReaderPageState};

#[derive(Clone)]
pub(crate) struct ReaderPageFacade {
    pub(crate) session: ReaderPageSession,
    pub(crate) snapshot: ReaderPageState,
    pub(crate) shortcuts: Callback<dioxus::events::KeyboardEvent>,
}

impl ReaderPageFacade {
    pub(crate) fn new(
        session: ReaderPageSession,
        snapshot: ReaderPageState,
        shortcuts: Callback<dioxus::events::KeyboardEvent>,
    ) -> Self {
        Self { session, snapshot, shortcuts }
    }

    pub(crate) fn previous_action_target(&self) -> Option<i64> {
        self.session.previous_action_target()
    }

    pub(crate) fn next_action_target(&self) -> Option<i64> {
        self.session.next_action_target()
    }

    pub(crate) fn toggle_read(&self, via_shortcut: bool) {
        self.session.toggle_read(via_shortcut);
    }

    pub(crate) fn toggle_starred(&self, via_shortcut: bool) {
        self.session.toggle_starred(via_shortcut);
    }
}
