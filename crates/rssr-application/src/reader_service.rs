use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{
    Entry, EntryContentRepository, EntryIndexRepository, EntryNavigation, FeedRepository,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderEntrySnapshot {
    pub entry: Option<Entry>,
    pub navigation: EntryNavigation,
    pub feed_title: Option<String>,
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
    entry_index_repository: Arc<dyn EntryIndexRepository>,
    entry_content_repository: Arc<dyn EntryContentRepository>,
    feed_repository: Arc<dyn FeedRepository>,
}

impl ReaderService {
    pub fn new(
        entry_index_repository: Arc<dyn EntryIndexRepository>,
        entry_content_repository: Arc<dyn EntryContentRepository>,
        feed_repository: Arc<dyn FeedRepository>,
    ) -> Self {
        Self { entry_index_repository, entry_content_repository, feed_repository }
    }

    pub async fn load_entry(&self, entry_id: i64) -> anyhow::Result<ReaderEntrySnapshot> {
        let record =
            self.entry_index_repository.get_entry_record(entry_id).await.context("读取文章失败")?;
        let navigation = if record.is_some() {
            self.entry_index_repository.reader_navigation(entry_id).await.unwrap_or_default()
        } else {
            EntryNavigation::default()
        };
        let entry = match record {
            Some(record) => Some(
                record.into_entry(
                    self.entry_content_repository
                        .get_content(entry_id)
                        .await
                        .context("读取文章正文失败")?,
                ),
            ),
            None => None,
        };

        let feed_title = match entry.as_ref() {
            Some(entry) => match self.feed_repository.get_feed(entry.feed_id).await {
                Ok(Some(feed)) => Some(
                    feed.title
                        .filter(|title| !title.trim().is_empty())
                        .unwrap_or_else(|| feed.url.to_string()),
                ),
                Ok(None) => None,
                Err(error) => {
                    tracing::warn!(feed_id = entry.feed_id, %error, "读取订阅元信息失败，保留正文");
                    None
                }
            },
            None => None,
        };
        Ok(ReaderEntrySnapshot { entry, navigation, feed_title })
    }

    pub async fn toggle_read(&self, input: ToggleReadInput) -> anyhow::Result<ToggleReadOutcome> {
        let is_read = !input.currently_read;
        self.entry_index_repository
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
        self.entry_index_repository
            .set_starred(input.entry_id, is_starred)
            .await
            .context("更新收藏状态失败")?;
        Ok(ToggleStarredOutcome { is_starred })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{
        EntryContent, EntryContentRepository, EntryIndexRepository, EntryNavigation, EntryQuery,
        EntryRecord, EntrySummary,
    };
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};
    use url::Url;

    use super::{ReaderService, ToggleReadInput, ToggleStarredInput};

    #[derive(Debug, Default)]
    struct EntryIndexRepositoryStub {
        entry: Option<EntryRecord>,
        navigation: EntryNavigation,
        fail_navigation: bool,
        read_calls: Mutex<Vec<(i64, bool)>>,
        starred_calls: Mutex<Vec<(i64, bool)>>,
    }

    #[async_trait::async_trait]
    impl EntryIndexRepository for EntryIndexRepositoryStub {
        async fn preview_mark_read(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<rssr_domain::MarkReadPreview> {
            Err(rssr_domain::DomainError::InvalidInput("此测试替身不支持批量操作".into()))
        }
        async fn mark_read_if_unchanged(
            &self,
            _preview: &rssr_domain::MarkReadPreview,
        ) -> rssr_domain::Result<rssr_domain::MarkReadOutcome> {
            Err(rssr_domain::DomainError::InvalidInput("此测试替身不支持批量操作".into()))
        }

        async fn list_entries(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<Vec<EntrySummary>> {
            Ok(Vec::new())
        }

        async fn count_entries(&self, _query: &EntryQuery) -> rssr_domain::Result<u64> {
            Ok(0)
        }

        async fn get_entry_record(
            &self,
            _entry_id: i64,
        ) -> rssr_domain::Result<Option<EntryRecord>> {
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

    #[derive(Debug, Default)]
    struct EntryContentRepositoryStub {
        content: Option<EntryContent>,
    }

    #[async_trait::async_trait]
    impl EntryContentRepository for EntryContentRepositoryStub {
        async fn get_content(&self, _entry_id: i64) -> rssr_domain::Result<Option<EntryContent>> {
            Ok(self.content.clone())
        }

        async fn delete_for_feed(&self, _feed_id: i64) -> rssr_domain::Result<()> {
            Ok(())
        }

        async fn delete_for_entry_ids(&self, _entry_ids: &[i64]) -> rssr_domain::Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct FeedRepositoryStub {
        feed: Option<rssr_domain::Feed>,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl rssr_domain::FeedRepository for FeedRepositoryStub {
        async fn upsert_subscription(
            &self,
            _: &rssr_domain::NewFeedSubscription,
        ) -> rssr_domain::Result<rssr_domain::Feed> {
            unreachable!()
        }
        async fn set_deleted(&self, _: i64, _: bool) -> rssr_domain::Result<()> {
            unreachable!()
        }
        async fn list_feeds(&self) -> rssr_domain::Result<Vec<rssr_domain::Feed>> {
            unreachable!()
        }
        async fn list_summaries(&self) -> rssr_domain::Result<Vec<rssr_domain::FeedSummary>> {
            unreachable!()
        }
        async fn get_feed(&self, feed_id: i64) -> rssr_domain::Result<Option<rssr_domain::Feed>> {
            assert_eq!(feed_id, 7);
            if self.fail {
                return Err(rssr_domain::DomainError::Persistence("feed unavailable".into()));
            }
            Ok(self.feed.clone())
        }
    }

    fn feed(title: Option<&str>) -> rssr_domain::Feed {
        rssr_domain::Feed {
            id: 7,
            url: Url::parse("https://example.com/feed").unwrap(),
            title: title.map(str::to_string),
            site_url: None,
            description: None,
            icon_url: None,
            folder: None,
            etag: None,
            last_modified: None,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
            is_deleted: false,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[tokio::test]
    async fn metadata_is_independent_and_best_effort() {
        for (feed, fail, expected) in [
            (Some(feed(Some("Source"))), false, Some("Source")),
            (Some(feed(None)), false, Some("https://example.com/feed")),
            (Some(feed(Some("  "))), false, Some("https://example.com/feed")),
            (None, false, None),
            (None, true, None),
        ] {
            let mut record = entry_record();
            record.author = Some("Writer".into());
            let service = ReaderService::new(
                Arc::new(EntryIndexRepositoryStub { entry: Some(record), ..Default::default() }),
                Arc::new(EntryContentRepositoryStub { content: Some(entry_content()) }),
                Arc::new(FeedRepositoryStub { feed, fail }),
            );
            let snapshot = service.load_entry(42).await.unwrap();
            assert_eq!(snapshot.feed_title.as_deref(), expected);
            let entry = snapshot.entry.unwrap();
            assert_eq!(entry.author.as_deref(), Some("Writer"));
            assert_eq!(entry.content_html.as_deref(), Some("<p>Body</p>"));
        }
    }

    fn entry_record() -> EntryRecord {
        let now = OffsetDateTime::parse("2026-04-12T00:00:00Z", &Rfc3339).expect("parse test time");
        EntryRecord {
            id: 42,
            feed_id: 7,
            external_id: "external-42".to_string(),
            dedup_key: "dedup-42".to_string(),
            url: Some(Url::parse("https://example.com/post").expect("parse url")),
            title: "Reader item".to_string(),
            author: None,
            summary: Some("Summary".to_string()),
            published_at: Some(now),
            updated_at_source: Some(now),
            first_seen_at: now,
            has_content: true,
            is_read: false,
            is_starred: true,
            read_at: None,
            starred_at: Some(now),
            created_at: now,
            updated_at: now,
        }
    }

    fn entry_content() -> EntryContent {
        let now = OffsetDateTime::parse("2026-04-12T00:00:00Z", &Rfc3339).expect("parse test time");
        EntryContent {
            entry_id: 42,
            content_html: Some("<p>Body</p>".to_string()),
            content_text: None,
            content_hash: None,
            updated_at: now,
        }
    }

    fn service(
        index_repository: Arc<EntryIndexRepositoryStub>,
        content_repository: Arc<EntryContentRepositoryStub>,
    ) -> ReaderService {
        ReaderService::new(
            index_repository,
            content_repository,
            Arc::new(FeedRepositoryStub::default()),
        )
    }

    #[tokio::test]
    async fn load_entry_returns_entry_and_navigation_snapshot() {
        let repository = Arc::new(EntryIndexRepositoryStub {
            entry: Some(entry_record()),
            navigation: EntryNavigation {
                previous_unread_entry_id: Some(1),
                next_unread_entry_id: Some(2),
                previous_feed_entry_id: Some(3),
                next_feed_entry_id: Some(4),
            },
            ..EntryIndexRepositoryStub::default()
        });
        let content_repository =
            Arc::new(EntryContentRepositoryStub { content: Some(entry_content()) });

        let snapshot = service(repository, content_repository)
            .load_entry(42)
            .await
            .expect("load reader entry");

        assert_eq!(snapshot.entry.expect("entry").id, 42);
        assert_eq!(snapshot.navigation.next_unread_entry_id, Some(2));
        assert_eq!(snapshot.navigation.next_feed_entry_id, Some(4));
    }

    #[tokio::test]
    async fn load_entry_keeps_navigation_best_effort() {
        let repository = Arc::new(EntryIndexRepositoryStub {
            entry: Some(entry_record()),
            fail_navigation: true,
            ..EntryIndexRepositoryStub::default()
        });
        let content_repository =
            Arc::new(EntryContentRepositoryStub { content: Some(entry_content()) });

        let snapshot = service(repository, content_repository)
            .load_entry(42)
            .await
            .expect("load reader entry");

        assert!(snapshot.entry.is_some());
        assert_eq!(snapshot.navigation, EntryNavigation::default());
    }

    #[tokio::test]
    async fn load_entry_without_entry_does_not_require_navigation() {
        let repository = Arc::new(EntryIndexRepositoryStub {
            entry: None,
            fail_navigation: true,
            ..EntryIndexRepositoryStub::default()
        });
        let content_repository = Arc::new(EntryContentRepositoryStub::default());

        let snapshot = service(repository, content_repository)
            .load_entry(42)
            .await
            .expect("load missing reader entry");

        assert!(snapshot.entry.is_none());
        assert!(snapshot.feed_title.is_none());
        assert_eq!(snapshot.navigation, EntryNavigation::default());
    }

    #[tokio::test]
    async fn load_entry_allows_missing_content() {
        let repository = Arc::new(EntryIndexRepositoryStub {
            entry: Some(entry_record()),
            ..EntryIndexRepositoryStub::default()
        });
        let content_repository = Arc::new(EntryContentRepositoryStub::default());

        let snapshot =
            service(repository, content_repository).load_entry(42).await.expect("load entry");

        let entry = snapshot.entry.expect("entry");
        assert_eq!(entry.summary.as_deref(), Some("Summary"));
        assert!(entry.content_html.is_none());
        assert!(entry.content_text.is_none());
    }

    #[tokio::test]
    async fn toggle_read_persists_inverted_state() {
        let repository = Arc::new(EntryIndexRepositoryStub::default());
        let content_repository = Arc::new(EntryContentRepositoryStub::default());

        let outcome = service(repository.clone(), content_repository)
            .toggle_read(ToggleReadInput { entry_id: 42, currently_read: false })
            .await
            .expect("toggle read");

        assert!(outcome.is_read);
        assert_eq!(*repository.read_calls.lock().expect("lock read calls"), vec![(42, true)]);
    }

    #[tokio::test]
    async fn toggle_starred_persists_inverted_state() {
        let repository = Arc::new(EntryIndexRepositoryStub::default());
        let content_repository = Arc::new(EntryContentRepositoryStub::default());

        let outcome = service(repository.clone(), content_repository)
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
