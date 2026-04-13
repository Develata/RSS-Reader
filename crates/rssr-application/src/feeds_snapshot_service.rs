use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{FeedRepository, FeedSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedsSnapshotOutcome {
    pub feeds: Vec<FeedSummary>,
    pub feed_count: usize,
    pub entry_count: usize,
}

#[derive(Clone)]
pub struct FeedsSnapshotService {
    feed_repository: Arc<dyn FeedRepository>,
}

impl FeedsSnapshotService {
    pub fn new(feed_repository: Arc<dyn FeedRepository>) -> Self {
        Self { feed_repository }
    }

    pub async fn load_snapshot(&self) -> anyhow::Result<FeedsSnapshotOutcome> {
        let feeds = self.feed_repository.list_summaries().await.context("读取订阅失败")?;
        let feed_count = feeds.len();
        let entry_count = feeds.iter().map(|feed| feed.entry_count as usize).sum();
        Ok(FeedsSnapshotOutcome { feeds, feed_count, entry_count })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{Feed, FeedRepository, FeedSummary, NewFeedSubscription};

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
    fn service(feeds: Vec<FeedSummary>) -> FeedsSnapshotService {
        FeedsSnapshotService::new(Arc::new(FeedRepositoryStub { summaries: feeds }))
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
