use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{Entry, EntryNavigation};

use crate::library_queries::{LibraryQueryPort, ReaderSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderEntrySnapshot {
    pub entry: Option<Entry>,
    pub navigation: EntryNavigation,
    pub feed_title: Option<String>,
}

#[derive(Clone)]
pub struct ReaderService {
    queries: Arc<dyn LibraryQueryPort>,
}

impl ReaderService {
    pub fn new(queries: Arc<dyn LibraryQueryPort>) -> Self {
        Self { queries }
    }

    /// 文章、正文、导航与订阅名称来自同一个读快照：要么整体读到，要么整体报错，不会把
    /// 正文与另一时刻的导航拼在一起。
    pub async fn load_entry(&self, entry_id: i64) -> anyhow::Result<ReaderEntrySnapshot> {
        let snapshot = self.queries.reader_snapshot(entry_id).await.context("读取文章失败")?;
        let Some(ReaderSnapshot { record, content, navigation, feed }) = snapshot else {
            return Ok(ReaderEntrySnapshot {
                entry: None,
                navigation: EntryNavigation::default(),
                feed_title: None,
            });
        };
        let feed_title = feed.map(|feed| {
            feed.title.filter(|title| !title.trim().is_empty()).unwrap_or_else(|| feed.url.into())
        });
        Ok(ReaderEntrySnapshot { entry: Some(record.into_entry(content)), navigation, feed_title })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{
        EntryContent, EntryContentRepository, EntryIndexRepository, EntryNavigation, EntryQuery,
        EntryRecord, EntrySummary,
    };
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};
    use url::Url;

    use super::ReaderService;
    use crate::library_queries::test_support::RepositoryQueries;

    #[derive(Debug, Default)]
    struct EntryIndexRepositoryStub {
        entry: Option<EntryRecord>,
        navigation: EntryNavigation,
        fail_navigation: bool,
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

        async fn set_read(&self, _entry_id: i64, _is_read: bool) -> rssr_domain::Result<bool> {
            unreachable!("the reader does not set flags")
        }

        async fn set_starred(&self, _entry_id: i64, _starred: bool) -> rssr_domain::Result<bool> {
            unreachable!("the reader does not set flags")
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
        async fn upsert_subscriptions(
            &self,
            new_feeds: &[rssr_domain::NewFeedSubscription],
        ) -> rssr_domain::Result<Vec<rssr_domain::Feed>> {
            let mut feeds = Vec::with_capacity(new_feeds.len());
            for new_feed in new_feeds {
                feeds.push(self.upsert_subscription(new_feed).await?);
            }
            Ok(feeds)
        }

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
            source_title: None,
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

    /// The feed name is the custom title when it has text, else the address; none for a removed
    /// subscription.
    #[tokio::test]
    async fn the_feed_name_falls_back_to_the_address() {
        for (feed, expected) in [
            (Some(feed(Some("Source"))), Some("Source")),
            (Some(feed(None)), Some("https://example.com/feed")),
            (Some(feed(Some("  "))), Some("https://example.com/feed")),
            (None, None),
        ] {
            let mut record = entry_record();
            record.author = Some("Writer".into());
            let service = service_with(
                Arc::new(EntryIndexRepositoryStub { entry: Some(record), ..Default::default() }),
                Arc::new(EntryContentRepositoryStub { content: Some(entry_content()) }),
                Arc::new(FeedRepositoryStub { feed, fail: false }),
            );
            let snapshot = service.load_entry(42).await.unwrap();
            assert_eq!(snapshot.feed_title.as_deref(), expected);
            let entry = snapshot.entry.unwrap();
            assert_eq!(entry.author.as_deref(), Some("Writer"));
            assert_eq!(entry.content_html.as_deref(), Some("<p>Body</p>"));
        }
    }

    /// The snapshot is read whole or not at all: a failing part fails the load.
    #[tokio::test]
    async fn a_failing_part_fails_the_whole_snapshot() {
        let failing_feed = service_with(
            Arc::new(EntryIndexRepositoryStub {
                entry: Some(entry_record()),
                ..Default::default()
            }),
            Arc::new(EntryContentRepositoryStub { content: Some(entry_content()) }),
            Arc::new(FeedRepositoryStub { feed: None, fail: true }),
        );
        assert!(failing_feed.load_entry(42).await.is_err());

        let failing_navigation = service(
            Arc::new(EntryIndexRepositoryStub {
                entry: Some(entry_record()),
                fail_navigation: true,
                ..EntryIndexRepositoryStub::default()
            }),
            Arc::new(EntryContentRepositoryStub { content: Some(entry_content()) }),
        );
        assert!(failing_navigation.load_entry(42).await.is_err());
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

    fn service_with(
        index_repository: Arc<EntryIndexRepositoryStub>,
        content_repository: Arc<EntryContentRepositoryStub>,
        feed_repository: Arc<FeedRepositoryStub>,
    ) -> ReaderService {
        let queries = RepositoryQueries {
            feeds: Some(feed_repository),
            entries: Some(index_repository),
            contents: Some(content_repository),
            ..RepositoryQueries::default()
        };
        ReaderService::new(Arc::new(queries))
    }

    fn service(
        index_repository: Arc<EntryIndexRepositoryStub>,
        content_repository: Arc<EntryContentRepositoryStub>,
    ) -> ReaderService {
        service_with(index_repository, content_repository, Arc::new(FeedRepositoryStub::default()))
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
}
