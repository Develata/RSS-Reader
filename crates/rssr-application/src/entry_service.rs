use std::sync::Arc;

use anyhow::Result;
use rssr_domain::{Entry, EntryNavigation, EntryQuery, EntryRepository, EntrySummary};

pub struct EntryService {
    repository: Arc<dyn EntryRepository>,
}

impl EntryService {
    pub fn new(repository: Arc<dyn EntryRepository>) -> Self {
        Self { repository }
    }

    pub async fn list_entries(&self, query: &EntryQuery) -> Result<Vec<EntrySummary>> {
        Ok(self.repository.list_entries(query).await?)
    }

    pub async fn get_entry(&self, entry_id: i64) -> Result<Option<Entry>> {
        Ok(self.repository.get_entry(entry_id).await?)
    }

    pub async fn reader_navigation(&self, current_entry_id: i64) -> Result<EntryNavigation> {
        Ok(self.repository.reader_navigation(current_entry_id).await?)
    }

    pub async fn set_read(&self, entry_id: i64, is_read: bool) -> Result<()> {
        Ok(self.repository.set_read(entry_id, is_read).await?)
    }

    pub async fn set_starred(&self, entry_id: i64, is_starred: bool) -> Result<()> {
        Ok(self.repository.set_starred(entry_id, is_starred).await?)
    }
}
