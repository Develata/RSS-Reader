use super::shared::map_store_error;
use crate::application_adapters::browser::{
    now_utc, query,
    state::{BrowserStore, Changes, PersistedEntryFlag},
};
use rssr_domain::{
    DomainError, EntryContent, EntryContentRepository, EntryIndexRepository, EntryNavigation,
    EntryQuery, EntryRecord, EntrySummary,
};
use std::collections::HashSet;

#[derive(Clone)]
pub struct BrowserEntryRepository {
    store: BrowserStore,
}
impl BrowserEntryRepository {
    pub fn new(store: BrowserStore) -> Self {
        Self { store }
    }

    async fn update_flags(
        &self,
        entry_id: i64,
        update: impl FnOnce(&mut PersistedEntryFlag) + Send + 'static,
    ) -> rssr_domain::Result<()> {
        self.store
            .update(move |state| {
                // The operation is applied to the latest committed flag, preserving the other bit.
                if !state.core.entries.iter().any(|entry| entry.id == entry_id) {
                    return Err(DomainError::NotFound.into());
                }
                let index =
                    match state.entry_flags.entries.iter().position(|entry| entry.id == entry_id) {
                        Some(index) => index,
                        None => {
                            state.entry_flags.entries.push(PersistedEntryFlag {
                                id: entry_id,
                                is_read: false,
                                is_starred: false,
                                read_at: None,
                                starred_at: None,
                            });
                            state.entry_flags.entries.len() - 1
                        }
                    };
                update(&mut state.entry_flags.entries[index]);
                Ok(((), Changes::FLAGS))
            })
            .await
            .map_err(map_store_error)
    }
}
#[async_trait::async_trait]
impl EntryIndexRepository for BrowserEntryRepository {
    async fn preview_mark_read(
        &self,
        query: &EntryQuery,
    ) -> rssr_domain::Result<rssr_domain::MarkReadPreview> {
        let query = EntryQuery { limit: None, ..query.clone() };
        self.store
            .read(move |state| {
                let unread_entry_ids = query::unread_selection(state, &query);
                Ok(rssr_domain::MarkReadPreview { query, unread_entry_ids })
            })
            .await
            .map_err(map_store_error)
    }
    async fn mark_read_if_unchanged(
        &self,
        preview: &rssr_domain::MarkReadPreview,
    ) -> rssr_domain::Result<rssr_domain::MarkReadOutcome> {
        let preview = preview.clone();
        self.store
            .update(move |state| {
                let query = EntryQuery { limit: None, ..preview.query };
                let ids = query::unread_selection(state, &query);
                if ids != preview.unread_entry_ids {
                    return Ok((
                        rssr_domain::MarkReadOutcome::SelectionChanged {
                            preview: rssr_domain::MarkReadPreview { query, unread_entry_ids: ids },
                        },
                        Changes::NONE,
                    ));
                }
                if ids.is_empty() {
                    return Ok((
                        rssr_domain::MarkReadOutcome::Applied { changed_count: 0 },
                        Changes::NONE,
                    ));
                }
                let now = now_utc();
                let mut missing = ids.iter().copied().collect::<HashSet<_>>();
                for flag in &mut state.entry_flags.entries {
                    if missing.remove(&flag.id) {
                        flag.is_read = true;
                        flag.read_at = Some(now);
                    }
                }
                state.entry_flags.entries.extend(missing.into_iter().map(|id| {
                    PersistedEntryFlag {
                        id,
                        is_read: true,
                        is_starred: false,
                        read_at: Some(now),
                        starred_at: None,
                    }
                }));
                Ok((
                    rssr_domain::MarkReadOutcome::Applied { changed_count: ids.len() as u64 },
                    Changes::FLAGS,
                ))
            })
            .await
            .map_err(map_store_error)
    }
    async fn list_entries(&self, query: &EntryQuery) -> rssr_domain::Result<Vec<EntrySummary>> {
        let query = query.clone();
        self.store
            .read(move |state| Ok(query::list_entries(state, &query)))
            .await
            .map_err(map_store_error)
    }
    async fn count_entries(&self, query: &EntryQuery) -> rssr_domain::Result<u64> {
        let query = query.clone();
        self.store
            .read(move |state| Ok(query::count_entries(state, &query)))
            .await
            .map_err(map_store_error)
    }
    async fn get_entry_record(&self, entry_id: i64) -> rssr_domain::Result<Option<EntryRecord>> {
        self.store
            .read(move |state| query::get_entry_record(state, entry_id))
            .await
            .map_err(map_store_error)
    }
    async fn reader_navigation(
        &self,
        current_entry_id: i64,
    ) -> rssr_domain::Result<EntryNavigation> {
        self.store
            .read(move |state| Ok(query::reader_navigation(state, current_entry_id)))
            .await
            .map_err(map_store_error)
    }
    async fn set_read(&self, entry_id: i64, is_read: bool) -> rssr_domain::Result<()> {
        self.update_flags(entry_id, move |entry| {
            entry.is_read = is_read;
            entry.read_at = is_read.then(now_utc);
        })
        .await
    }
    async fn set_starred(&self, entry_id: i64, is_starred: bool) -> rssr_domain::Result<()> {
        self.update_flags(entry_id, move |entry| {
            entry.is_starred = is_starred;
            entry.starred_at = is_starred.then(now_utc);
        })
        .await
    }
    async fn delete_for_feed(&self, feed_id: i64) -> rssr_domain::Result<()> {
        self.store
            .update(move |state| {
                let removed = state
                    .core
                    .entries
                    .iter()
                    .filter(|entry| entry.feed_id == feed_id)
                    .map(|entry| entry.id)
                    .collect::<HashSet<_>>();
                state.core.entries.retain(|entry| entry.feed_id != feed_id);
                state.entry_flags.entries.retain(|entry| !removed.contains(&entry.id));
                state.entry_content.entries.retain(|entry| entry.feed_id != feed_id);
                Ok(((), Changes::CORE | Changes::FLAGS | Changes::CONTENT))
            })
            .await
            .map_err(map_store_error)
    }
}
#[async_trait::async_trait]
impl EntryContentRepository for BrowserEntryRepository {
    async fn get_content(&self, entry_id: i64) -> rssr_domain::Result<Option<EntryContent>> {
        self.store
            .read(move |state| query::get_entry_content(state, entry_id))
            .await
            .map_err(map_store_error)
    }
    async fn delete_for_feed(&self, feed_id: i64) -> rssr_domain::Result<()> {
        self.store
            .update(move |state| {
                state.entry_content.entries.retain(|entry| entry.feed_id != feed_id);
                for entry in state.core.entries.iter_mut().filter(|entry| entry.feed_id == feed_id)
                {
                    entry.has_content = false;
                }
                Ok(((), Changes::CORE | Changes::CONTENT))
            })
            .await
            .map_err(map_store_error)
    }
    async fn delete_for_entry_ids(&self, entry_ids: &[i64]) -> rssr_domain::Result<()> {
        if entry_ids.is_empty() {
            return Ok(());
        }
        let entry_ids = entry_ids.iter().copied().collect::<HashSet<_>>();
        self.store
            .update(move |state| {
                state.entry_content.entries.retain(|entry| !entry_ids.contains(&entry.entry_id));
                for entry in
                    state.core.entries.iter_mut().filter(|entry| entry_ids.contains(&entry.id))
                {
                    entry.has_content = false;
                }
                Ok(((), Changes::CORE | Changes::CONTENT))
            })
            .await
            .map_err(map_store_error)
    }
}
