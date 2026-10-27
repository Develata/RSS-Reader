//! [`LibraryQueryPort`] over the library: each query runs its statements in one read
//! transaction, so they all see the same snapshot.

use rssr_application::{
    ConfigSnapshot, EntriesPage, EntriesStart, FeedsOverview, LibraryQueryPort, ReaderSnapshot,
    StartupState,
};
use rssr_domain::{
    AppStateSnapshot, ConfigFingerprint, EntryQuery, Result as DomainResult, UserSettings,
};

use super::documents::{APP_STATE_KEY, USER_SETTINGS_KEY, load};
use super::entries::{
    count_entries, get_content, get_entry_record, list_entries, reader_navigation,
};
use super::executor::LibraryStore;
use super::feeds::{config_fingerprint, get_active, list_active, list_summaries};

#[derive(Clone)]
pub struct LibraryQueries {
    store: LibraryStore,
}

impl LibraryQueries {
    pub fn new(store: LibraryStore) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl LibraryQueryPort for LibraryQueries {
    async fn entries_page(
        &self,
        query: &EntryQuery,
        count: Option<&EntryQuery>,
    ) -> DomainResult<EntriesPage> {
        let query = query.clone();
        let count = count.cloned();
        Ok(self
            .store
            .read(move |conn| {
                let entries = list_entries(conn, &query)?;
                let counted = count.map(|count| count_entries(conn, &count)).transpose()?;
                Ok(EntriesPage { entries, counted })
            })
            .await?)
    }

    async fn reader_snapshot(&self, entry_id: i64) -> DomainResult<Option<ReaderSnapshot>> {
        Ok(self
            .store
            .read(move |conn| {
                let Some(record) = get_entry_record(conn, entry_id)? else { return Ok(None) };
                let content = get_content(conn, entry_id)?;
                let navigation = reader_navigation(conn, entry_id)?;
                let feed = get_active(conn, record.feed_id)?.map(|row| row.feed);
                Ok(Some(ReaderSnapshot { record, content, navigation, feed }))
            })
            .await?)
    }

    async fn entries_start(&self, preferences: bool, feeds: bool) -> DomainResult<EntriesStart> {
        Ok(self
            .store
            .read(move |conn| {
                let (settings, workspace) = if preferences {
                    let settings: UserSettings = load(conn, USER_SETTINGS_KEY)?;
                    let state: AppStateSnapshot = load(conn, APP_STATE_KEY)?;
                    (Some(settings), Some(state.entries_workspace))
                } else {
                    (None, None)
                };
                let feeds = if feeds { Some(list_summaries(conn)?) } else { None };
                Ok(EntriesStart { settings, workspace, feeds })
            })
            .await?)
    }

    async fn startup_state(&self) -> DomainResult<StartupState> {
        Ok(self
            .store
            .read(|conn| {
                let settings = load(conn, USER_SETTINGS_KEY)?;
                let state: AppStateSnapshot = load(conn, APP_STATE_KEY)?;
                let last_opened_feed_id = match state.last_opened_feed_id {
                    Some(feed_id) => get_active(conn, feed_id)?.map(|row| row.feed.id),
                    None => None,
                };
                Ok(StartupState { settings, last_opened_feed_id })
            })
            .await?)
    }

    async fn feeds_overview(&self) -> DomainResult<FeedsOverview> {
        Ok(self
            .store
            .read(|conn| {
                let settings: UserSettings = load(conn, USER_SETTINGS_KEY)?;
                let archive_filter = match rssr_domain::archive_cutoff_at(
                    crate::platform_clock::now_utc(),
                    settings.archive_after_months,
                ) {
                    Some(cutoff) => rssr_domain::ArchiveFilter::OnlyArchived { cutoff },
                    None => rssr_domain::ArchiveFilter::OnlyManual,
                };
                let archived_count =
                    count_entries(conn, &EntryQuery { archive_filter, ..Default::default() })?;
                Ok(FeedsOverview {
                    archived_count,
                    feeds: list_summaries(conn)?,
                    fingerprint: config_fingerprint(conn)?,
                })
            })
            .await?)
    }

    async fn config_fingerprint(&self) -> DomainResult<ConfigFingerprint> {
        Ok(self.store.read(config_fingerprint).await?)
    }

    async fn config_snapshot(&self) -> DomainResult<ConfigSnapshot> {
        Ok(self
            .store
            .read(|conn| {
                let feeds = list_active(conn)?.into_iter().map(|row| row.feed).collect();
                Ok(ConfigSnapshot { feeds, settings: load(conn, USER_SETTINGS_KEY)? })
            })
            .await?)
    }
}
