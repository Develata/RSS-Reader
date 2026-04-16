use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{EntryIndexRepository, EntryQuery, EntrySummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntriesListOutcome {
    pub entries: Vec<EntrySummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleEntryReadInput {
    pub entry_id: i64,
    pub currently_read: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleEntryReadOutcome {
    pub is_read: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleEntryStarredInput {
    pub entry_id: i64,
    pub currently_starred: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleEntryStarredOutcome {
    pub is_starred: bool,
}

#[derive(Clone)]
pub struct EntriesListService {
    entry_repository: Arc<dyn EntryIndexRepository>,
}

impl EntriesListService {
    pub fn new(entry_repository: Arc<dyn EntryIndexRepository>) -> Self {
        Self { entry_repository }
    }

    pub async fn list_entries(&self, query: &EntryQuery) -> anyhow::Result<EntriesListOutcome> {
        let entries = self.entry_repository.list_entries(query).await.context("读取文章失败")?;
        Ok(EntriesListOutcome { entries })
    }

    pub async fn toggle_read(
        &self,
        input: ToggleEntryReadInput,
    ) -> anyhow::Result<ToggleEntryReadOutcome> {
        let is_read = !input.currently_read;
        self.entry_repository
            .set_read(input.entry_id, is_read)
            .await
            .context("更新已读状态失败")?;
        Ok(ToggleEntryReadOutcome { is_read })
    }

    pub async fn toggle_starred(
        &self,
        input: ToggleEntryStarredInput,
    ) -> anyhow::Result<ToggleEntryStarredOutcome> {
        let is_starred = !input.currently_starred;
        self.entry_repository
            .set_starred(input.entry_id, is_starred)
            .await
            .context("更新收藏状态失败")?;
        Ok(ToggleEntryStarredOutcome { is_starred })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{
        EntryIndexRepository, EntryNavigation, EntryQuery, EntryRecord, EntrySummary,
    };

    use super::{EntriesListService, ToggleEntryReadInput, ToggleEntryStarredInput};

    #[derive(Debug, Default)]
    struct EntryRepositoryStub {
        entries: Vec<EntrySummary>,
        read_calls: Mutex<Vec<(i64, bool)>>,
        starred_calls: Mutex<Vec<(i64, bool)>>,
    }

    #[async_trait::async_trait]
    impl EntryIndexRepository for EntryRepositoryStub {
        async fn list_entries(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<Vec<EntrySummary>> {
            Ok(self.entries.clone())
        }

        async fn count_entries(&self, _query: &EntryQuery) -> rssr_domain::Result<u64> {
            Ok(self.entries.len() as u64)
        }

        async fn get_entry_record(
            &self,
            _entry_id: i64,
        ) -> rssr_domain::Result<Option<EntryRecord>> {
            Ok(None)
        }

        async fn reader_navigation(
            &self,
            _current_entry_id: i64,
        ) -> rssr_domain::Result<EntryNavigation> {
            Ok(EntryNavigation::default())
        }

        async fn set_read(&self, entry_id: i64, is_read: bool) -> rssr_domain::Result<()> {
            self.read_calls.lock().expect("lock read calls").push((entry_id, is_read));
            Ok(())
        }

        async fn set_starred(&self, entry_id: i64, is_starred: bool) -> rssr_domain::Result<()> {
            self.starred_calls.lock().expect("lock starred calls").push((entry_id, is_starred));
            Ok(())
        }

        async fn delete_for_feed(&self, _feed_id: i64) -> rssr_domain::Result<()> {
            Ok(())
        }
    }

    fn service(repository: Arc<EntryRepositoryStub>) -> EntriesListService {
        EntriesListService::new(repository)
    }

    #[tokio::test]
    async fn list_entries_returns_entry_summaries() {
        let repository = Arc::new(EntryRepositoryStub {
            entries: vec![EntrySummary {
                id: 42,
                feed_id: 7,
                title: "Entry 42".to_string(),
                feed_title: "Feed".to_string(),
                published_at: None,
                is_read: false,
                is_starred: false,
            }],
            ..EntryRepositoryStub::default()
        });

        let outcome =
            service(repository).list_entries(&EntryQuery::default()).await.expect("list entries");

        assert_eq!(outcome.entries.len(), 1);
        assert_eq!(outcome.entries[0].id, 42);
    }

    #[tokio::test]
    async fn toggle_read_persists_inverted_state() {
        let repository = Arc::new(EntryRepositoryStub::default());

        let outcome = service(repository.clone())
            .toggle_read(ToggleEntryReadInput { entry_id: 42, currently_read: false })
            .await
            .expect("toggle read");

        assert!(outcome.is_read);
        assert_eq!(*repository.read_calls.lock().expect("lock read calls"), vec![(42, true)]);
    }

    #[tokio::test]
    async fn toggle_starred_persists_inverted_state() {
        let repository = Arc::new(EntryRepositoryStub::default());

        let outcome = service(repository.clone())
            .toggle_starred(ToggleEntryStarredInput { entry_id: 42, currently_starred: true })
            .await
            .expect("toggle starred");

        assert!(!outcome.is_starred);
        assert_eq!(
            *repository.starred_calls.lock().expect("lock starred calls"),
            vec![(42, false)]
        );
    }
}
