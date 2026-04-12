use std::sync::{Arc, Mutex};

use rssr_domain::{DomainError, Entry, EntryNavigation, EntryQuery, EntryRepository, EntrySummary};

use crate::application_adapters::browser::{
    now_utc,
    query::{
        get_entry as query_get_entry, list_entries as query_list_entries,
        reader_navigation as query_reader_navigation,
    },
    state::{BrowserState, PersistedEntryFlag, save_entry_flag_patch, save_state_snapshot},
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
impl EntryRepository for BrowserEntryRepository {
    async fn list_entries(&self, query: &EntryQuery) -> rssr_domain::Result<Vec<EntrySummary>> {
        let state = self.state.lock().expect("lock state");
        Ok(query_list_entries(&state, query))
    }

    async fn get_entry(&self, entry_id: i64) -> rssr_domain::Result<Option<Entry>> {
        let state = self.state.lock().expect("lock state");
        query_get_entry(&state, entry_id).map_err(map_persistence_error)
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
            state.clone()
        };

        save_state_snapshot(snapshot).map_err(map_persistence_error)
    }
}
