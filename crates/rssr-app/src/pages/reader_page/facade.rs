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

    pub(crate) fn status_message(&self) -> &str {
        &self.snapshot.status
    }

    pub(crate) fn status_tone(&self) -> &str {
        &self.snapshot.status_tone
    }

    pub(crate) fn has_status_message(&self) -> bool {
        !self.status_message().is_empty()
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

    pub(crate) fn previous_entry_target(&self) -> Option<i64> {
        self.session.previous_entry_target()
    }

    pub(crate) fn next_entry_target(&self) -> Option<i64> {
        self.session.next_entry_target()
    }

    pub(crate) fn has_previous_entry_target(&self) -> bool {
        self.previous_entry_target().is_some()
    }

    pub(crate) fn has_next_entry_target(&self) -> bool {
        self.next_entry_target().is_some()
    }

    pub(crate) fn read_toggle_icon(&self) -> &'static str {
        if self.is_read() { "○" } else { "✓" }
    }

    pub(crate) fn read_toggle_label(&self) -> &'static str {
        if self.is_read() { "未读（M）" } else { "已读（M）" }
    }

    pub(crate) fn read_state(&self) -> &'static str {
        if self.is_read() { "read" } else { "unread" }
    }

    pub(crate) fn starred_toggle_icon(&self) -> &'static str {
        if self.is_starred() { "★" } else { "☆" }
    }

    pub(crate) fn starred_state(&self) -> &'static str {
        if self.is_starred() { "starred" } else { "unstarred" }
    }

    pub(crate) fn previous_entry_state(&self) -> &'static str {
        if self.has_previous_entry_target() { "available" } else { "unavailable" }
    }

    pub(crate) fn next_entry_state(&self) -> &'static str {
        if self.has_next_entry_target() { "available" } else { "unavailable" }
    }

    pub(crate) fn body_state(&self) -> &'static str {
        if self.body_html().is_some() { "html" } else { "text" }
    }

    pub(crate) fn toggle_read(&self, via_shortcut: bool) {
        self.session.toggle_read(via_shortcut);
    }

    pub(crate) fn toggle_starred(&self, via_shortcut: bool) {
        self.session.toggle_starred(via_shortcut);
    }
}
