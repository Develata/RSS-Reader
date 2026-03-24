use std::sync::Arc;

use anyhow::Result;
use rssr_domain::{Entry, EntryQuery, EntryRepository, EntrySummary};

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
}
