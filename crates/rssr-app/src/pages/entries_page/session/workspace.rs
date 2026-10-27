//! App-owned preference writer: coalesces choices and orders them with URL edits.
use super::super::intent::EntriesPageIntent;
use super::EntriesPageSession;
use crate::ui::{EntriesCommand, UiCommand, UiIntent, run_ui_command};
use dioxus::prelude::*;
use rssr_domain::EntriesWorkspaceState;
use std::{cell::RefCell, rc::Rc};

/// Only the latest waiting choice survives. Cloning retains this mailbox after
/// the page is gone without retaining its signals or article snapshots.
#[derive(Clone, Default)]
pub(super) struct WorkspaceSaveQueue(Rc<RefCell<Option<EntriesWorkspaceState>>>);

impl WorkspaceSaveQueue {
    fn replace(&self, next: EntriesWorkspaceState) {
        *self.0.borrow_mut() = Some(next);
    }

    fn take_changed(
        &self,
        stored: &Option<EntriesWorkspaceState>,
    ) -> Option<EntriesWorkspaceState> {
        self.0.borrow_mut().take().filter(|wanted| Some(wanted) != stored.as_ref())
    }
}

impl EntriesPageSession {
    pub(crate) fn save_browsing_preferences(
        self,
        preferences_loaded: bool,
        next: EntriesWorkspaceState,
    ) {
        if !self.can_save_browsing_preferences(preferences_loaded) {
            return;
        }
        // Record the choice synchronously. The serialized writer owns the only
        // async boundary, so a later read cannot overtake an unregistered edit.
        self.save_workspace(next);
    }

    /// Saves the browsing preferences the user chose, one save at a time. Only what differs from
    /// the stored ones is sent: preferences that were read (at start or again) are the library's
    /// already, and a read never writes (C4b item 2). A choice made while a save is on its way
    /// waits for it, then is saved if it still differs, so the last choice is the one kept
    /// (review C4B-B-05).
    fn save_workspace(self, next: rssr_domain::EntriesWorkspaceState) {
        let state = self.state.peek();
        if state.workspace_saving {
            drop(state);
            self.workspace_saves.peek().replace(next.clone());
            self.dispatch(EntriesPageIntent::WorkspaceWanted(next));
            return;
        }
        // The filters a read recovered: the effect fires for them once, when the page becomes
        // ready, and that is not the user's change; only that firing is skipped (reviews
        // C4B-B-R2-06, C4B-B-R3-01).
        if let Some(recovered) = state.recovered_edits.clone() {
            drop(state);
            self.dispatch(EntriesPageIntent::RecoveredEditsHandled);
            if recovered == next {
                return;
            }
            return self.save_workspace(next);
        }
        if state.stored_workspace.as_ref() == Some(&next) {
            return;
        }
        let mut stored = state.stored_workspace.clone();
        drop(state);
        self.dispatch(EntriesPageIntent::WorkspaceSaveStarted);
        let pending = self.workspace_saves.peek().clone();
        let order = self.shell.workspace_write_order();
        self.shell.spawn_in_shell(async move {
            // Hold the order through the final queued choice. A feed URL edit, or
            // a newer page's save, cannot overtake the choices from this page.
            let _order = order.lock().await;
            let mut next = next;
            loop {
                let command =
                    UiCommand::Entries(EntriesCommand::SaveBrowsingPreferences { workspace: next });
                let intents = run_ui_command(command).await;
                let failure = intents.iter().find_map(UiIntent::failure).cloned();
                let unknown =
                    failure.as_ref().is_some_and(crate::ui::failure::UiFailure::is_unknown);
                for intent in &intents {
                    if let UiIntent::EntriesPage(EntriesPageIntent::WorkspaceSaved(saved)) = intent
                    {
                        stored = Some(saved.clone());
                    }
                }
                if unknown {
                    stored = None;
                }
                let wanted = pending.take_changed(&stored);
                if self.state.try_peek().is_ok() {
                    // Follow-up reads still belong to this page, not the writer's
                    // root scope. Never deliver a receipt to a replacement page.
                    dioxus::core::Runtime::current().in_scope(self.owner, || {
                        for intent in
                            intents.into_iter().filter_map(UiIntent::into_entries_page_intent)
                        {
                            self.dispatch(intent);
                        }
                        if unknown {
                            self.dispatch(EntriesPageIntent::WorkspaceSaveUnknown);
                        }
                        self.dispatch(EntriesPageIntent::WorkspaceSaveEnded);
                        if wanted.is_some() {
                            self.dispatch(EntriesPageIntent::WorkspaceSaveStarted);
                        }
                    });
                } else if let Some(failure) = failure {
                    self.shell.report_job(failure.message(), failure.tone().to_owned());
                }
                match wanted {
                    Some(wanted) => next = wanted,
                    None => break,
                }
            }
            if self.state.try_peek().is_err() {
                self.shell.reread_now();
            }
        });
    }

    pub(super) fn can_save_browsing_preferences(self, preferences_loaded: bool) -> bool {
        // The captured flag also matters: an effect queued before bootstrap
        // finished still contains default values, even if the live state is ready.
        preferences_loaded && self.state.peek().preferences_load.is_loaded()
    }
}
