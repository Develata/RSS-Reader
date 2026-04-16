use std::sync::{Arc, Mutex};

use rssr_domain::{
    DomainError, EntryContent, EntryContentRepository, EntryIndexRepository, EntryNavigation,
    EntryQuery, EntryRecord, EntrySummary,
};

use crate::application_adapters::browser::{
    now_utc,
    query::{
        count_entries as query_count_entries, get_entry_content as query_get_entry_content,
        get_entry_record as query_get_entry_record, list_entries as query_list_entries,
        reader_navigation as query_reader_navigation,
    },
    state::{
        BrowserState, PersistedEntryContent, PersistedEntryFlag, save_entry_content_patch,
        save_entry_flag_patch, save_state_snapshot,
    },
};

use super::shared::map_persistence_error;

#[derive(Clone)]
pub struct BrowserEntryRepository {
    state: Arc<Mutex<BrowserState>>,
}

impl BrowserEntryRepository {
    pub fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl EntryIndexRepository for BrowserEntryRepository {
    async fn list_entries(&self, query: &EntryQuery) -> rssr_domain::Result<Vec<EntrySummary>> {
        let state = self.state.lock().expect("lock state");
        Ok(query_list_entries(&state, query))
    }

    async fn count_entries(&self, query: &EntryQuery) -> rssr_domain::Result<u64> {
        let state = self.state.lock().expect("lock state");
        Ok(query_count_entries(&state, query))
    }

    async fn get_entry_record(&self, entry_id: i64) -> rssr_domain::Result<Option<EntryRecord>> {
        let state = self.state.lock().expect("lock state");
        query_get_entry_record(&state, entry_id).map_err(map_persistence_error)
    }

    async fn reader_navigation(
        &self,
        current_entry_id: i64,
    ) -> rssr_domain::Result<EntryNavigation> {
        let state = self.state.lock().expect("lock state");
        Ok(query_reader_navigation(&state, current_entry_id))
    }

    async fn set_read(&self, entry_id: i64, is_read: bool) -> rssr_domain::Result<()> {
        let entry = {
            let mut state = self.state.lock().expect("lock state");
            let now = now_utc();
            let entry = state.entry_flags.entries.iter_mut().find(|entry| entry.id == entry_id);

            if let Some(entry) = entry {
                entry.is_read = is_read;
                entry.read_at = is_read.then_some(now);
                entry.clone()
            } else {
                if !state.core.entries.iter().any(|entry| entry.id == entry_id) {
                    return Err(DomainError::NotFound);
                }
                let flag = PersistedEntryFlag {
                    id: entry_id,
                    is_read,
                    is_starred: false,
                    read_at: is_read.then_some(now),
                    starred_at: None,
                };
                state.entry_flags.entries.push(flag.clone());
                flag
            }
        };

        save_entry_flag_patch(entry).map_err(map_persistence_error)
    }

    async fn set_starred(&self, entry_id: i64, is_starred: bool) -> rssr_domain::Result<()> {
        let entry = {
            let mut state = self.state.lock().expect("lock state");
            let now = now_utc();
            let entry = state.entry_flags.entries.iter_mut().find(|entry| entry.id == entry_id);

            if let Some(entry) = entry {
                entry.is_starred = is_starred;
                entry.starred_at = is_starred.then_some(now);
                entry.clone()
            } else {
                if !state.core.entries.iter().any(|entry| entry.id == entry_id) {
                    return Err(DomainError::NotFound);
                }
                let flag = PersistedEntryFlag {
                    id: entry_id,
                    is_read: false,
                    is_starred,
                    read_at: None,
                    starred_at: is_starred.then_some(now),
                };
                state.entry_flags.entries.push(flag.clone());
                flag
            }
        };

        save_entry_flag_patch(entry).map_err(map_persistence_error)
    }

    async fn delete_for_feed(&self, feed_id: i64) -> rssr_domain::Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let removed_entry_ids = state
                .core
                .entries
                .iter()
                .filter(|entry| entry.feed_id == feed_id)
                .map(|entry| entry.id)
                .collect::<Vec<_>>();
            state.core.entries.retain(|entry| entry.feed_id != feed_id);
            state.entry_flags.entries.retain(|entry| !removed_entry_ids.contains(&entry.id));
            state
                .entry_content
                .entries
                .retain(|entry| !removed_entry_ids.contains(&entry.entry_id));
            state.clone()
        };

        save_state_snapshot(snapshot).map_err(map_persistence_error)
    }
}

#[async_trait::async_trait]
impl EntryContentRepository for BrowserEntryRepository {
    async fn get_content(&self, entry_id: i64) -> rssr_domain::Result<Option<EntryContent>> {
        let state = self.state.lock().expect("lock state");
        query_get_entry_content(&state, entry_id).map_err(map_persistence_error)
    }

    async fn delete_for_feed(&self, feed_id: i64) -> rssr_domain::Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            state.entry_content.entries.retain(|entry| entry.feed_id != feed_id);
            for entry in state.core.entries.iter_mut().filter(|entry| entry.feed_id == feed_id) {
                entry.has_content = false;
            }
            state.clone()
        };

        save_state_snapshot(snapshot).map_err(map_persistence_error)
    }

    async fn delete_for_entry_ids(&self, entry_ids: &[i64]) -> rssr_domain::Result<()> {
        if entry_ids.is_empty() {
            return Ok(());
        }

        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            state.entry_content.entries.retain(|entry| !entry_ids.contains(&entry.entry_id));
            for entry in state.core.entries.iter_mut().filter(|entry| entry_ids.contains(&entry.id))
            {
                entry.has_content = false;
            }
            state.clone()
        };

        save_state_snapshot(snapshot).map_err(map_persistence_error)
    }
}
