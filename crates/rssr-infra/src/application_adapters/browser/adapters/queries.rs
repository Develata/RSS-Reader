//! [`LibraryQueryPort`] over the browser state: each query reads the state once, under the
//! store's Web Lock, so all its parts come from the same committed version.

use rssr_application::{
    ConfigSnapshot, EntriesPage, EntriesStart, LibraryQueryPort, ReaderSnapshot, StartupState,
};
use rssr_domain::EntryQuery;

use super::feed::persisted_feed_to_domain;
use super::shared::map_store_error;
use crate::application_adapters::browser::{
    query,
    state::{BrowserState, BrowserStore},
};

#[derive(Clone)]
pub struct BrowserLibraryQueries {
    store: BrowserStore,
}

impl BrowserLibraryQueries {
    pub fn new(store: BrowserStore) -> Self {
        Self { store }
    }
}

fn active_feed(
    state: &BrowserState,
    feed_id: i64,
) -> rssr_domain::Result<Option<rssr_domain::Feed>> {
    state
        .core
        .feeds
        .iter()
        .find(|feed| feed.id == feed_id && !feed.is_deleted)
        .map(persisted_feed_to_domain)
        .transpose()
}

#[async_trait::async_trait]
impl LibraryQueryPort for BrowserLibraryQueries {
    async fn entries_page(
        &self,
        query: &EntryQuery,
        count: Option<&EntryQuery>,
    ) -> rssr_domain::Result<EntriesPage> {
        let query = query.clone();
        let count = count.cloned();
        self.store
            .read(move |state| {
                Ok(EntriesPage {
                    entries: query::list_entries(state, &query),
                    counted: count.map(|count| query::count_entries(state, &count)),
                })
            })
            .await
            .map_err(map_store_error)
    }

    async fn reader_snapshot(&self, entry_id: i64) -> rssr_domain::Result<Option<ReaderSnapshot>> {
        self.store
            .read(move |state| {
                let Some(record) = query::get_entry_record(state, entry_id)? else {
                    return Ok(None);
                };
                Ok(Some(ReaderSnapshot {
                    content: query::get_entry_content(state, entry_id)?,
                    navigation: query::reader_navigation(state, entry_id),
                    feed: active_feed(state, record.feed_id)?,
                    record,
                }))
            })
            .await
            .map_err(map_store_error)
    }

    async fn entries_start(
        &self,
        preferences: bool,
        feeds: bool,
    ) -> rssr_domain::Result<EntriesStart> {
        self.store
            .read(move |state| {
                Ok(EntriesStart {
                    settings: preferences.then(|| state.core.settings.clone()),
                    workspace: preferences.then(|| state.app_state.entries_workspace.clone()),
                    feeds: feeds.then(|| query::list_feeds(state)),
                })
            })
            .await
            .map_err(map_store_error)
    }

    async fn startup_state(&self) -> rssr_domain::Result<StartupState> {
        self.store
            .read(|state| {
                let last_opened_feed_id = state.app_state.last_opened_feed_id.filter(|feed_id| {
                    state.core.feeds.iter().any(|feed| feed.id == *feed_id && !feed.is_deleted)
                });
                Ok(StartupState { settings: state.core.settings.clone(), last_opened_feed_id })
            })
            .await
            .map_err(map_store_error)
    }

    async fn config_snapshot(&self) -> rssr_domain::Result<ConfigSnapshot> {
        self.store
            .read(|state| {
                let feeds = state
                    .core
                    .feeds
                    .iter()
                    .filter(|feed| !feed.is_deleted)
                    .map(persisted_feed_to_domain)
                    .collect::<rssr_domain::Result<Vec<_>>>()?;
                Ok(ConfigSnapshot { feeds, settings: state.core.settings.clone() })
            })
            .await
            .map_err(map_store_error)
    }
}
