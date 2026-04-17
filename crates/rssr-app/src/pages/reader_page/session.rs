use dioxus::prelude::*;

use crate::bootstrap::ReaderNavigation;
use crate::ui::{ReaderCommand, UiCommand, UiIntent, spawn_projected_ui_command};

use super::{reducer::dispatch_reader_page_intent, state::ReaderPageState};

#[derive(Clone, Copy)]
pub(crate) struct ReaderPageSession {
    entry_id: i64,
    state: Signal<ReaderPageState>,
}

impl ReaderPageSession {
    pub(crate) fn new(entry_id: i64, state: Signal<ReaderPageState>) -> Self {
        Self { entry_id, state }
    }

    pub(crate) fn snapshot(self) -> ReaderPageState {
        (self.state)()
    }

    pub(crate) fn reload_tick(self) -> u64 {
        (self.state)().reload_tick
    }

    pub(crate) fn previous_entry_target(self) -> Option<i64> {
        previous_entry_target((self.state)().navigation_state)
    }

    pub(crate) fn next_entry_target(self) -> Option<i64> {
        next_entry_target((self.state)().navigation_state)
    }

    pub(crate) fn load(self) {
        self.spawn_ui_command(UiCommand::Reader(ReaderCommand::LoadEntry {
            entry_id: self.entry_id,
        }));
    }

    pub(crate) fn localize_entry_assets(self) {
        self.spawn_ui_command(UiCommand::Reader(ReaderCommand::LocalizeEntryAssets {
            entry_id: self.entry_id,
        }));
    }

    pub(crate) fn toggle_read(self, via_shortcut: bool) {
        self.spawn_ui_command(UiCommand::Reader(ReaderCommand::ToggleRead {
            entry_id: self.entry_id,
            currently_read: (self.state)().is_read,
            via_shortcut,
        }));
    }

    pub(crate) fn toggle_starred(self, via_shortcut: bool) {
        self.spawn_ui_command(UiCommand::Reader(ReaderCommand::ToggleStarred {
            entry_id: self.entry_id,
            currently_starred: (self.state)().is_starred,
            via_shortcut,
        }));
    }

    fn spawn_ui_command(self, command: UiCommand) {
        spawn_projected_ui_command(command, UiIntent::into_reader_page_intent, move |intent| {
            dispatch_reader_page_intent(self.state, intent);
        });
    }
}

fn previous_entry_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.previous_unread_entry_id.or(navigation.previous_feed_entry_id)
}

fn next_entry_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.next_unread_entry_id.or(navigation.next_feed_entry_id)
}
