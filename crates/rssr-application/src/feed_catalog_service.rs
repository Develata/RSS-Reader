use std::sync::Arc;

use rssr_domain::{Feed, FeedRepository};

#[derive(Clone)]
pub struct FeedCatalogService {
    feed_repository: Arc<dyn FeedRepository>,
}

impl FeedCatalogService {
    pub fn new(feed_repository: Arc<dyn FeedRepository>) -> Self {
        Self { feed_repository }
    }

    pub async fn list_feeds(&self) -> anyhow::Result<Vec<Feed>> {
        Ok(self.feed_repository.list_feeds().await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{Feed, FeedRepository, FeedSummary, NewFeedSubscription};
    use time::OffsetDateTime;
    use url::Url;

    use super::FeedCatalogService;

    struct FeedRepositoryStub {
        feeds: Vec<Feed>,
    }

    #[async_trait::async_trait]
    impl FeedRepository for FeedRepositoryStub {
        async fn upsert_subscription(
            &self,
            _new_feed: &NewFeedSubscription,
        ) -> rssr_domain::Result<Feed> {
            Err(rssr_domain::DomainError::InvalidInput(
                "upsert not used in feed catalog tests".to_string(),
            ))
        }

        async fn set_deleted(&self, _feed_id: i64, _is_deleted: bool) -> rssr_domain::Result<()> {
            Ok(())
        }

        async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
            Ok(self.feeds.clone())
        }

        async fn get_feed(&self, _feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
            Ok(None)
        }

        async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn list_feeds_returns_full_feed_entities() {
        let service = FeedCatalogService::new(Arc::new(FeedRepositoryStub {
            feeds: vec![Feed {
                id: 7,
                url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                title: Some("Example".to_string()),
                site_url: None,
                description: None,
                icon_url: None,
                folder: Some("Tech".to_string()),
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
            }],
        }));

        let feeds = service.list_feeds().await.expect("list feeds");

        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].id, 7);
        assert_eq!(feeds[0].folder.as_deref(), Some("Tech"));
    }
}
