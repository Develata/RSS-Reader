use dioxus::prelude::Callback;

use crate::bootstrap::ReaderNavigation;

use super::{session::ReaderPageSession, state::ReaderPageState};

#[derive(Clone)]
pub(crate) struct ReaderPageFacade {
    session: ReaderPageSession,
    snapshot: ReaderPageState,
    shortcuts: Callback<dioxus::events::KeyboardEvent>,
}

impl ReaderPageFacade {
    pub(crate) fn new(
        session: ReaderPageSession,
        snapshot: ReaderPageState,
        shortcuts: Callback<dioxus::events::KeyboardEvent>,
    ) -> Self {
        Self { session, snapshot, shortcuts }
    }

    pub(crate) fn shortcuts(&self) -> Callback<dioxus::events::KeyboardEvent> {
        self.shortcuts
    }

    pub(crate) fn title(&self) -> &str {
        &self.snapshot.title
    }

    pub(crate) fn source(&self) -> &str {
        &self.snapshot.source
    }

    pub(crate) fn published_at(&self) -> &str {
        &self.snapshot.published_at
    }

    pub(crate) fn body_text(&self) -> &str {
        &self.snapshot.body_text
    }

    pub(crate) fn body_html(&self) -> Option<&str> {
        self.snapshot.body_html.as_deref()
    }

    pub(crate) fn status(&self) -> &str {
        &self.snapshot.status
    }

    pub(crate) fn status_tone(&self) -> &str {
        &self.snapshot.status_tone
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.snapshot.error.as_deref()
    }

    pub(crate) fn is_read(&self) -> bool {
        self.snapshot.is_read
    }

    pub(crate) fn is_starred(&self) -> bool {
        self.snapshot.is_starred
    }

    pub(crate) fn navigation_state(&self) -> &ReaderNavigation {
        &self.snapshot.navigation_state
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
