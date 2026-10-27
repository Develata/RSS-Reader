use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{ConfigFingerprint, FeedSummary};

use crate::library_queries::{FeedsOverview, LibraryQueryPort};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FeedsSnapshotOutcome {
    pub feeds: Vec<FeedSummary>,
    pub feed_count: u64,
    pub entry_count: u64,
    pub unread_count: u64,
    pub archived_count: u64,
    /// What a configuration import from this page expects to replace.
    pub fingerprint: ConfigFingerprint,
}

#[derive(Clone)]
pub struct FeedsSnapshotService {
    queries: Arc<dyn LibraryQueryPort>,
}

impl FeedsSnapshotService {
    pub fn new(queries: Arc<dyn LibraryQueryPort>) -> Self {
        Self { queries }
    }

    /// The subscriptions and the configuration fingerprint from one read snapshot.
    pub async fn load_snapshot(&self) -> anyhow::Result<FeedsSnapshotOutcome> {
        let FeedsOverview { feeds, fingerprint, archived_count } =
            self.queries.feeds_overview().await.context("读取订阅失败")?;
        let feed_count = feeds.len() as u64;
        let entry_count = feeds.iter().map(|feed| feed.entry_count).sum();
        let unread_count = feeds.iter().map(|feed| feed.unread_count).sum();
        Ok(FeedsSnapshotOutcome {
            feeds,
            feed_count,
            entry_count,
            unread_count,
            archived_count,
            fingerprint,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{Feed, FeedRepository, FeedSummary, NewFeedSubscription};

    use super::FeedsSnapshotService;
    use crate::library_queries::test_support::RepositoryQueries;

    struct FeedRepositoryStub {
        summaries: Vec<FeedSummary>,
    }

    #[async_trait::async_trait]
    impl FeedRepository for FeedRepositoryStub {
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
        let queries = RepositoryQueries {
            feeds: Some(Arc::new(FeedRepositoryStub { summaries: feeds })),
            settings: Some(Arc::new(crate::settings_service::tests::MemorySettings::default())),
            ..RepositoryQueries::default()
        };
        FeedsSnapshotService::new(Arc::new(queries))
    }

    fn feed(id: i64, entry_count: u64) -> FeedSummary {
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
