use crate::bootstrap::ReaderNavigation;

use super::{
    session::ReaderPageSession,
    state::{ReaderFlag, ReaderPageState},
};

#[derive(Clone)]
pub(crate) struct ReaderPageFacade {
    session: ReaderPageSession,
    snapshot: ReaderPageState,
}

impl ReaderPageFacade {
    pub(crate) fn positions_ready(&self, entry_id: i64) -> bool {
        self.load_state(entry_id) == "loaded"
    }

    pub(crate) fn load_state(&self, entry_id: i64) -> &'static str {
        self.snapshot.load_state(entry_id)
    }

    pub(super) fn load_generation(&self) -> u64 {
        self.snapshot.load_generation
    }
    pub(crate) fn new(session: ReaderPageSession, snapshot: ReaderPageState) -> Self {
        Self { session, snapshot }
    }

    pub(crate) fn can_write_flag(&self, entry_id: i64, flag: ReaderFlag) -> bool {
        self.snapshot.can_write_flag(entry_id, flag)
    }

    pub(crate) fn flag_write_pending(&self, flag: ReaderFlag) -> bool {
        self.snapshot.flag_write_pending(flag)
    }

    pub(crate) fn title(&self) -> &str {
        &self.snapshot.title
    }

    pub(crate) fn author(&self) -> Option<&str> {
        self.snapshot.author.as_deref()
    }

    pub(crate) fn original_url(&self) -> Option<&str> {
        self.snapshot.original_url.as_deref()
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

    pub(crate) fn error(&self) -> Option<&str> {
        self.snapshot.error.as_deref()
    }

    pub(crate) fn error_tone(&self) -> &'static str {
        self.snapshot.error_tone
    }

    /// Whether the status is shown on screen (screen readers hear every status): an error, a
    /// failure or an outcome that may have happened; a plain result is only announced.
    pub(crate) fn status_shown(&self) -> bool {
        status_shown(&self.snapshot)
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

    pub(crate) fn read_toggle_text(&self) -> &'static str {
        if self.is_read() { "未读" } else { "已读" }
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

/// See [`ReaderPageFacade::status_shown`] (C4b item 1: an unknown outcome must be seen).
pub(crate) fn status_shown(state: &super::state::ReaderPageState) -> bool {
    let tone = state.status_tone.as_str();
    state.error.is_some() || tone == "error" || tone == crate::ui::failure::TONE_UNKNOWN
}
