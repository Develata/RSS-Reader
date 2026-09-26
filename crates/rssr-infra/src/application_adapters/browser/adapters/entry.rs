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
    state::{BrowserState, PersistedEntryFlag, save_entry_flags_slice, save_state_snapshot},
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

    /// 标记操作共用持久化边界；写失败只回滚本次条目，避免内存状态领先于存储。
    fn update_flags(
        &self,
        entry_id: i64,
        update: impl FnOnce(&mut PersistedEntryFlag),
    ) -> rssr_domain::Result<()> {
        let mut state =
            self.state.lock().map_err(|error| DomainError::Persistence(error.to_string()))?;
        let index = state.entry_flags.entries.iter().position(|entry| entry.id == entry_id);
        let previous = index.map(|index| state.entry_flags.entries[index].clone());
        let index = match index {
            Some(index) => index,
            None => {
                if !state.core.entries.iter().any(|entry| entry.id == entry_id) {
                    return Err(DomainError::NotFound);
                }
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
        if let Err(error) = save_entry_flags_slice(&state.entry_flags) {
            if let Some(previous) = previous {
                state.entry_flags.entries[index] = previous;
            } else {
                state.entry_flags.entries.pop();
            }
            return Err(map_persistence_error(error));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl EntryIndexRepository for BrowserEntryRepository {
    async fn preview_mark_read(
        &self,
        query: &EntryQuery,
    ) -> rssr_domain::Result<rssr_domain::MarkReadPreview> {
        let state = self.state.lock().map_err(|e| DomainError::Persistence(e.to_string()))?;
        let query = EntryQuery { limit: None, ..query.clone() };
        let unread_entry_ids =
            crate::application_adapters::browser::query::unread_selection(&state, &query);
        Ok(rssr_domain::MarkReadPreview { query, unread_entry_ids })
    }
    async fn mark_read_if_unchanged(
        &self,
        preview: &rssr_domain::MarkReadPreview,
    ) -> rssr_domain::Result<rssr_domain::MarkReadOutcome> {
        let mut state = self.state.lock().map_err(|e| DomainError::Persistence(e.to_string()))?;
        let query = EntryQuery { limit: None, ..preview.query.clone() };
        let ids = crate::application_adapters::browser::query::unread_selection(&state, &query);
        if ids != preview.unread_entry_ids {
            return Ok(rssr_domain::MarkReadOutcome::SelectionChanged {
                preview: rssr_domain::MarkReadPreview { query, unread_entry_ids: ids },
            });
        }
        if ids.is_empty() {
            return Ok(rssr_domain::MarkReadOutcome::Applied { changed_count: 0 });
        }
        let previous = state.entry_flags.clone();
        let now = now_utc();
        let mut missing = ids.iter().copied().collect::<std::collections::HashSet<_>>();
        for flag in &mut state.entry_flags.entries {
            if missing.remove(&flag.id) {
                flag.is_read = true;
                flag.read_at = Some(now);
            }
        }
        state.entry_flags.entries.extend(missing.into_iter().map(|id| PersistedEntryFlag {
            id,
            is_read: true,
            is_starred: false,
            read_at: Some(now),
            starred_at: None,
        }));
        if let Err(error) = save_entry_flags_slice(&state.entry_flags) {
            state.entry_flags = previous;
            return Err(map_persistence_error(error));
        }
        Ok(rssr_domain::MarkReadOutcome::Applied { changed_count: ids.len() as u64 })
    }

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
        self.update_flags(entry_id, |entry| {
            entry.is_read = is_read;
            entry.read_at = is_read.then(now_utc);
        })
    }

    async fn set_starred(&self, entry_id: i64, is_starred: bool) -> rssr_domain::Result<()> {
        self.update_flags(entry_id, |entry| {
            entry.is_starred = is_starred;
            entry.starred_at = is_starred.then(now_utc);
        })
    }

    async fn delete_for_feed(&self, feed_id: i64) -> rssr_domain::Result<()> {
        {
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
            save_state_snapshot(&state).map_err(map_persistence_error)
        }
    }
}

#[async_trait::async_trait]
impl EntryContentRepository for BrowserEntryRepository {
    async fn get_content(&self, entry_id: i64) -> rssr_domain::Result<Option<EntryContent>> {
        let state = self.state.lock().expect("lock state");
        query_get_entry_content(&state, entry_id).map_err(map_persistence_error)
    }

    async fn delete_for_feed(&self, feed_id: i64) -> rssr_domain::Result<()> {
        {
            let mut state = self.state.lock().expect("lock state");
            state.entry_content.entries.retain(|entry| entry.feed_id != feed_id);
            for entry in state.core.entries.iter_mut().filter(|entry| entry.feed_id == feed_id) {
                entry.has_content = false;
            }
            save_state_snapshot(&state).map_err(map_persistence_error)
        }
    }

    async fn delete_for_entry_ids(&self, entry_ids: &[i64]) -> rssr_domain::Result<()> {
        if entry_ids.is_empty() {
            return Ok(());
        }

        {
            let mut state = self.state.lock().expect("lock state");
            state.entry_content.entries.retain(|entry| !entry_ids.contains(&entry.entry_id));
            for entry in state.core.entries.iter_mut().filter(|entry| entry_ids.contains(&entry.id))
            {
                entry.has_content = false;
            }
            save_state_snapshot(&state).map_err(map_persistence_error)
        }
    }
}
