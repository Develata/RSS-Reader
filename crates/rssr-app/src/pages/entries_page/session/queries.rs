//! Page-owned bootstrap and article reads; request policy is framework-independent.
use super::super::{intent::EntriesPageIntent, request_lifecycle::QueryCompletion};
use super::EntriesPageSession;
use crate::ui::{EntriesCommand, UiCommand, UiIntent, run_ui_command};
use dioxus::prelude::*;
use rssr_domain::EntryQuery;
use std::future::Future;

impl EntriesPageSession {
    pub(crate) fn bootstrap(self) {
        self.spawn_bootstrap(move |_| run_ui_command(self.bootstrap_command()));
    }

    pub(super) fn spawn_bootstrap<F: Future<Output = Vec<UiIntent>> + 'static>(
        mut self,
        bootstrap: impl FnOnce(bool) -> F,
    ) {
        let request = self
            .state
            .with_peek(|state| self.requests.with_mut(|requests| requests.begin_bootstrap(state)));
        let bootstrap = bootstrap(request.load_preferences);
        self.dispatch(EntriesPageIntent::BootstrapStarted);
        let order = self.shell.workspace_write_order();
        spawn(async move {
            // A page opened during an earlier save/URL edit must read its committed
            // baseline before it can save filters. Do not hold the boundary during
            // this read: generations still allow a newer read to supersede it.
            drop(order.lock().await);
            let intents = bootstrap.await;
            if !self.requests.peek().accepts_bootstrap(request) {
                return;
            }
            self.dispatch(EntriesPageIntent::BootstrapFinished);
            for intent in intents.into_iter().filter_map(UiIntent::into_entries_page_intent) {
                self.dispatch(intent);
            }
        });
    }

    /// The page's read-only refresh (C4b item 2): the preferences and subscriptions again,
    /// superseding a start's read still on its way (the reducer then takes it as the start's).
    pub(crate) fn reread(self) {
        self.spawn_bootstrap(|_| run_ui_command(UiCommand::Entries(EntriesCommand::Reread)));
    }

    /// The first run reads the page's start; later ones are its read-only refresh.
    pub(crate) fn bootstrap_or_reread(self) {
        if !self.requests.peek().has_started() {
            self.bootstrap();
        } else {
            self.reread();
        }
    }

    fn bootstrap_command(self) -> UiCommand {
        // Reading readiness must not subscribe the bootstrap effect to its own
        // completion and start a second feed-summary query.
        let load_preferences = !self.state.peek().preferences_load.is_loaded();
        UiCommand::Entries(EntriesCommand::Bootstrap { load_preferences, load_feeds: true })
    }

    /// Opening a subscription's page remembers it for the startup view; a failure is shown.
    pub(crate) fn remember_opened_feed(self, feed_id: i64) {
        self.spawn_ui_command(UiCommand::Entries(EntriesCommand::RememberOpenedFeed { feed_id }));
    }

    pub(crate) fn load_entries_query(self, query: EntryQuery) {
        let key = self.state.peek().position_key(self.feed_id, query.search_title.as_deref());
        let return_page = (!self.state.peek().entries_loaded)
            .then(|| crate::ui::reading_position::remembered_page(&key))
            .flatten();
        self.spawn_entries_query_with_return(
            run_ui_command(UiCommand::Entries(EntriesCommand::LoadEntries { query })),
            return_page,
        );
    }

    #[cfg(test)]
    pub(super) fn spawn_entries_query(self, query: impl Future<Output = Vec<UiIntent>> + 'static) {
        self.spawn_entries_query_with_return(query, None);
    }

    pub(super) fn spawn_entries_query_with_return(
        mut self,
        query: impl Future<Output = Vec<UiIntent>> + 'static,
        return_page: Option<u32>,
    ) {
        let request = self.state.with_peek(|state| {
            self.requests.with_mut(|requests| requests.begin_query(state, return_page))
        });
        let Some(request) = request else {
            return;
        };
        self.dispatch(EntriesPageIntent::EntriesQueryStarted);
        spawn(async move {
            let intents = query.await;
            let completion =
                self.state.with_peek(|state| self.requests.peek().complete_query(request, state));
            if completion == QueryCompletion::Discard {
                return;
            }
            for intent in completion
                .intents(intents.into_iter().filter_map(UiIntent::into_entries_page_intent))
            {
                self.dispatch(intent);
            }
            self.dispatch(EntriesPageIntent::EntriesQueryFinished);
        });
    }
}
