//! Admission, execution and receipts for single-article and explicit batch writes.
use super::super::{intent::EntriesPageIntent, write_feedback};
use super::EntriesPageSession;
use crate::ui::{EntriesCommand, UiCommand, UiIntent, run_ui_command};
use dioxus::prelude::*;
use std::future::Future;

impl EntriesPageSession {
    pub(crate) fn apply_selection(self, action: rssr_domain::EntryBatchAction) {
        if self.state.with_peek(|state| state.bulk_busy || !state.pending_entry_writes.is_empty()) {
            return;
        }
        let entry_ids = self.state.peek().selected_entry_ids.iter().copied().collect::<Vec<_>>();
        if entry_ids.is_empty() {
            return;
        }
        let command = EntriesCommand::ApplySelection {
            request: rssr_domain::EntryBatchRequest { entry_ids, action },
        };
        self.dispatch(EntriesPageIntent::BeginBulk { writing: true });
        let generation = self.state.peek().bulk_generation;
        self.spawn_article_mutation(run_ui_command(UiCommand::Entries(command)), move |intents| {
            if self.state.peek().bulk_generation != generation {
                return;
            }
            for intent in intents {
                let applied = matches!(&intent, EntriesPageIntent::SelectionApplied { .. });
                self.dispatch(intent);
                if applied {
                    self.bootstrap();
                }
            }
        });
    }

    /// The card's button toggles: it asks for the opposite of what the card shows.
    pub(crate) fn toggle_read(self, entry_id: i64, entry_title: String, currently_read: bool) {
        self.spawn_entry_write(entry_id, move || {
            run_ui_command(UiCommand::Entries(EntriesCommand::SetRead {
                entry_id,
                entry_title,
                read: !currently_read,
            }))
        });
    }

    pub(crate) fn toggle_starred(
        self,
        entry_id: i64,
        entry_title: String,
        currently_starred: bool,
    ) {
        self.spawn_entry_write(entry_id, move || {
            run_ui_command(UiCommand::Entries(EntriesCommand::SetStarred {
                entry_id,
                entry_title,
                starred: !currently_starred,
            }))
        });
    }

    pub(super) fn spawn_entry_write<F: Future<Output = Vec<UiIntent>> + 'static>(
        self,
        entry_id: i64,
        write: impl FnOnce() -> F + 'static,
    ) {
        // Check live Rust state as well as disabling buttons: a second event
        // can already be queued before the DOM has rendered the pending state.
        if self
            .state
            .with_peek(|state| state.bulk_busy || state.pending_entry_writes.contains(&entry_id))
        {
            return;
        }
        self.dispatch(EntriesPageIntent::EntryWriteStarted(entry_id));
        self.spawn_article_mutation(async move { write().await }, move |intents| {
            let mut read_changed = false;
            for intent in intents {
                read_changed |=
                    matches!(&intent, EntriesPageIntent::PatchEntryFlags { is_read: Some(_), .. });
                self.dispatch(intent);
            }
            self.dispatch(EntriesPageIntent::EntryWriteFinished(entry_id));
            if read_changed {
                self.bootstrap();
            }
        });
    }

    /// Accepted article writes belong to the app, like preference saves. Only
    /// their presentation belongs to this page. Re-enter its scope so follow-up
    /// reads are cancelled on navigation rather than inheriting the app lifetime.
    fn spawn_article_mutation(
        self,
        write: impl Future<Output = Vec<UiIntent>> + 'static,
        complete: impl FnOnce(Vec<EntriesPageIntent>) + 'static,
    ) {
        self.shell.spawn_in_shell(async move {
            let intents: Vec<_> =
                write.await.into_iter().filter_map(UiIntent::into_entries_page_intent).collect();
            if self.state.try_peek().is_ok() {
                dioxus::core::Runtime::current().in_scope(self.owner, || complete(intents));
            } else {
                for intent in intents {
                    if let Some((message, tone)) = write_feedback::notice(intent) {
                        self.shell.report_job(message, tone);
                    }
                }
                self.shell.reread_now();
            }
        });
    }
}
