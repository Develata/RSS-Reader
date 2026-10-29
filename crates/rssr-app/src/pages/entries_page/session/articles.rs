//! Admission, execution and receipts for single-article and explicit batch writes.
use super::super::{
    intent::EntriesPageIntent,
    write_feedback,
    write_lifecycle::{BulkWrite, EntryWrite, WriteEffect},
};
use super::EntriesPageSession;
use crate::ui::{EntriesCommand, UiCommand, UiIntent, run_ui_command};
use dioxus::prelude::*;
use std::future::Future;

impl EntriesPageSession {
    pub(crate) fn apply_selection(self, action: rssr_domain::EntryBatchAction) {
        let request = self.state.with_peek(|state| {
            BulkWrite::can_start(state).then(|| BulkWrite::selection(state, action)).flatten()
        });
        let Some(request) = request else { return };
        let command = EntriesCommand::ApplySelection { request };
        self.dispatch(EntriesPageIntent::BeginBulk { writing: true });
        let request = self.state.with_peek(BulkWrite::capture_started);
        self.spawn_article_mutation(run_ui_command(UiCommand::Entries(command)), move |intents| {
            let effects = self.state.with_peek(|state| request.complete(state, intents));
            self.apply_write_effects(effects);
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
        let Some(request) = self.state.with_peek(|state| EntryWrite::prepare(state, entry_id))
        else {
            return;
        };
        self.dispatch(request.started());
        self.spawn_article_mutation(async move { write().await }, move |intents| {
            self.apply_write_effects(request.complete(intents));
        });
    }

    fn apply_write_effects(self, effects: impl Iterator<Item = WriteEffect>) {
        for effect in effects {
            match effect {
                WriteEffect::Publish(intent) => self.dispatch(intent),
                WriteEffect::Reread => self.reread(),
            }
        }
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
