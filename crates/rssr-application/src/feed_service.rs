use std::sync::Arc;

use anyhow::Result;
use rssr_domain::{FeedRepository, FeedSummary, NewFeedSubscription};

pub struct FeedService {
    repository: Arc<dyn FeedRepository>,
}

impl FeedService {
    pub fn new(repository: Arc<dyn FeedRepository>) -> Self {
        Self { repository }
    }

    pub async fn add_subscription(&self, new_feed: &NewFeedSubscription) -> Result<()> {
        self.repository.upsert_subscription(new_feed).await?;
        Ok(())
    }

    pub async fn list_feeds(&self) -> Result<Vec<FeedSummary>> {
        Ok(self.repository.list_summaries().await?)
    }
}
