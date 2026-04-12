use anyhow::Context;
use rssr_domain::FeedSummary;

use crate::FeedService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedsSnapshotOutcome {
    pub feeds: Vec<FeedSummary>,
    pub feed_count: usize,
    pub entry_count: usize,
}

#[derive(Clone)]
pub struct FeedsSnapshotService {
    feed_service: FeedService,
}

impl FeedsSnapshotService {
    pub fn new(feed_service: FeedService) -> Self {
        Self { feed_service }
    }

    pub async fn load_snapshot(&self) -> anyhow::Result<FeedsSnapshotOutcome> {
        let feeds = self.feed_service.list_feeds().await.context("读取订阅失败")?;
        let feed_count = feeds.len();
        let entry_count = feeds.iter().map(|feed| feed.entry_count as usize).sum();
        Ok(FeedsSnapshotOutcome { feeds, feed_count, entry_count })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{
        Entry, EntryNavigation, EntryQuery, EntryRepository, EntrySummary, Feed, FeedRepository,
        FeedSummary, NewFeedSubscription,
    };

    use super::FeedsSnapshotService;

    struct FeedRepositoryStub {
        summaries: Vec<FeedSummary>,
    }

    #[async_trait::async_trait]
    impl FeedRepository for FeedRepositoryStub {
        async fn upsert_subscription(
            &self,
            _new_feed: &NewFeedSubscription,
        ) -> rssr_domain::Result<Feed> {
            Err(rssr_domain::DomainError::InvalidInput(
                "upsert not used in feeds snapshot tests".to_string(),
            ))
        }

        async fn set_deleted(&self, _feed_id: i64, _is_deleted: bool) -> rssr_domain::Result<()> {
            Ok(())
        }

        async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
            Ok(Vec::new())
        }

        async fn get_feed(&self, _feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
            Ok(None)
        }

        async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
            Ok(self.summaries.clone())
        }
    }

    struct EntryRepositoryStub;

    #[async_trait::async_trait]
    impl EntryRepository for EntryRepositoryStub {
        async fn list_entries(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<Vec<EntrySummary>> {
            Err(rssr_domain::DomainError::InvalidInput(
                "entry list should not be used for feeds snapshot".to_string(),
            ))
        }

        async fn get_entry(&self, _entry_id: i64) -> rssr_domain::Result<Option<Entry>> {
            Ok(None)
        }

        async fn reader_navigation(
            &self,
            _current_entry_id: i64,
        ) -> rssr_domain::Result<EntryNavigation> {
            Ok(EntryNavigation::default())
        }

        async fn set_read(&self, _entry_id: i64, _is_read: bool) -> rssr_domain::Result<()> {
            Ok(())
        }

        async fn set_starred(&self, _entry_id: i64, _is_starred: bool) -> rssr_domain::Result<()> {
            Ok(())
        }

        async fn delete_for_feed(&self, _feed_id: i64) -> rssr_domain::Result<()> {
            Ok(())
        }
    }

    fn service(feeds: Vec<FeedSummary>) -> FeedsSnapshotService {
        FeedsSnapshotService::new(crate::FeedService::new(
            Arc::new(FeedRepositoryStub { summaries: feeds }),
            Arc::new(EntryRepositoryStub),
        ))
    }

    fn feed(id: i64, entry_count: u32) -> FeedSummary {
        FeedSummary {
            id,
            title: format!("Feed {id}"),
            url: format!("https://example.com/{id}.xml"),
            unread_count: 0,
            entry_count,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
        }
    }

    #[tokio::test]
    async fn load_snapshot_counts_feeds_and_entries_from_feed_summaries() {
        let outcome = service(vec![feed(1, 3), feed(2, 5)])
            .load_snapshot()
            .await
            .expect("load feeds snapshot");

        assert_eq!(outcome.feed_count, 2);
        assert_eq!(outcome.entry_count, 8);
        assert_eq!(outcome.feeds.len(), 2);
    }
}
