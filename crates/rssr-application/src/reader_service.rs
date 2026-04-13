use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{Entry, EntryNavigation, EntryRepository};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderEntrySnapshot {
    pub entry: Option<Entry>,
    pub navigation: EntryNavigation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleReadInput {
    pub entry_id: i64,
    pub currently_read: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleReadOutcome {
    pub is_read: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleStarredInput {
    pub entry_id: i64,
    pub currently_starred: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleStarredOutcome {
    pub is_starred: bool,
}

#[derive(Clone)]
pub struct ReaderService {
    entry_repository: Arc<dyn EntryRepository>,
}

impl ReaderService {
    pub fn new(entry_repository: Arc<dyn EntryRepository>) -> Self {
        Self { entry_repository }
    }

    pub async fn load_entry(&self, entry_id: i64) -> anyhow::Result<ReaderEntrySnapshot> {
        let entry = self.entry_repository.get_entry(entry_id).await.context("读取文章失败")?;
        let navigation = if entry.is_some() {
            self.entry_repository.reader_navigation(entry_id).await.unwrap_or_default()
        } else {
            EntryNavigation::default()
        };

        Ok(ReaderEntrySnapshot { entry, navigation })
    }

    pub async fn toggle_read(&self, input: ToggleReadInput) -> anyhow::Result<ToggleReadOutcome> {
        let is_read = !input.currently_read;
        self.entry_repository
            .set_read(input.entry_id, is_read)
            .await
            .context("更新已读状态失败")?;
        Ok(ToggleReadOutcome { is_read })
    }

    pub async fn toggle_starred(
        &self,
        input: ToggleStarredInput,
    ) -> anyhow::Result<ToggleStarredOutcome> {
        let is_starred = !input.currently_starred;
        self.entry_repository
            .set_starred(input.entry_id, is_starred)
            .await
            .context("更新收藏状态失败")?;
        Ok(ToggleStarredOutcome { is_starred })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{Entry, EntryNavigation, EntryQuery, EntryRepository, EntrySummary};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};
    use url::Url;

    use super::{ReaderService, ToggleReadInput, ToggleStarredInput};

    #[derive(Debug, Default)]
    struct EntryRepositoryStub {
        entry: Option<Entry>,
        navigation: EntryNavigation,
        fail_navigation: bool,
        read_calls: Mutex<Vec<(i64, bool)>>,
        starred_calls: Mutex<Vec<(i64, bool)>>,
    }

    #[async_trait::async_trait]
    impl EntryRepository for EntryRepositoryStub {
        async fn list_entries(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<Vec<EntrySummary>> {
            Ok(Vec::new())
        }

        async fn get_entry(&self, _entry_id: i64) -> rssr_domain::Result<Option<Entry>> {
            Ok(self.entry.clone())
        }

        async fn reader_navigation(
            &self,
            _current_entry_id: i64,
        ) -> rssr_domain::Result<EntryNavigation> {
            if self.fail_navigation {
                return Err(rssr_domain::DomainError::Persistence("navigation failed".to_string()));
            }
            Ok(self.navigation)
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

    fn entry() -> Entry {
        let now = OffsetDateTime::parse("2026-04-12T00:00:00Z", &Rfc3339).expect("parse test time");
        Entry {
            id: 42,
            feed_id: 7,
            external_id: "external-42".to_string(),
            dedup_key: "dedup-42".to_string(),
            url: Some(Url::parse("https://example.com/post").expect("parse url")),
            title: "Reader item".to_string(),
            author: None,
            summary: Some("Summary".to_string()),
            content_html: Some("<p>Body</p>".to_string()),
            content_text: None,
            published_at: Some(now),
            updated_at_source: Some(now),
            first_seen_at: now,
            content_hash: None,
            is_read: false,
            is_starred: true,
            read_at: None,
            starred_at: Some(now),
            created_at: now,
            updated_at: now,
        }
    }

    fn service(repository: Arc<EntryRepositoryStub>) -> ReaderService {
        ReaderService::new(repository)
    }

    #[tokio::test]
    async fn load_entry_returns_entry_and_navigation_snapshot() {
        let repository = Arc::new(EntryRepositoryStub {
            entry: Some(entry()),
            navigation: EntryNavigation {
                previous_unread_entry_id: Some(1),
                next_unread_entry_id: Some(2),
                previous_feed_entry_id: Some(3),
                next_feed_entry_id: Some(4),
            },
            ..EntryRepositoryStub::default()
        });

        let snapshot = service(repository).load_entry(42).await.expect("load reader entry");

        assert_eq!(snapshot.entry.expect("entry").id, 42);
        assert_eq!(snapshot.navigation.next_unread_entry_id, Some(2));
        assert_eq!(snapshot.navigation.next_feed_entry_id, Some(4));
    }

    #[tokio::test]
    async fn load_entry_keeps_navigation_best_effort() {
        let repository = Arc::new(EntryRepositoryStub {
            entry: Some(entry()),
            fail_navigation: true,
            ..EntryRepositoryStub::default()
        });

        let snapshot = service(repository).load_entry(42).await.expect("load reader entry");

        assert!(snapshot.entry.is_some());
        assert_eq!(snapshot.navigation, EntryNavigation::default());
    }

    #[tokio::test]
    async fn load_entry_without_entry_does_not_require_navigation() {
        let repository = Arc::new(EntryRepositoryStub {
            entry: None,
            fail_navigation: true,
            ..EntryRepositoryStub::default()
        });

        let snapshot = service(repository).load_entry(42).await.expect("load missing reader entry");

        assert!(snapshot.entry.is_none());
        assert_eq!(snapshot.navigation, EntryNavigation::default());
    }

    #[tokio::test]
    async fn toggle_read_persists_inverted_state() {
        let repository = Arc::new(EntryRepositoryStub::default());

        let outcome = service(repository.clone())
            .toggle_read(ToggleReadInput { entry_id: 42, currently_read: false })
            .await
            .expect("toggle read");

        assert!(outcome.is_read);
        assert_eq!(*repository.read_calls.lock().expect("lock read calls"), vec![(42, true)]);
    }

    #[tokio::test]
    async fn toggle_starred_persists_inverted_state() {
        let repository = Arc::new(EntryRepositoryStub::default());

        let outcome = service(repository.clone())
            .toggle_starred(ToggleStarredInput { entry_id: 42, currently_starred: true })
            .await
            .expect("toggle starred");

        assert!(!outcome.is_starred);
        assert_eq!(
            *repository.starred_calls.lock().expect("lock starred calls"),
            vec![(42, false)]
        );
    }
}
