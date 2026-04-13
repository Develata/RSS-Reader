use std::sync::Arc;

use anyhow::{Context, Result};
use rssr_domain::{EntryRepository, Feed, FeedRepository, NewFeedSubscription, normalize_feed_url};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddSubscriptionInput {
    pub url: String,
    pub title: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveSubscriptionInput {
    pub feed_id: i64,
    pub purge_entries: bool,
}

#[derive(Clone)]
pub struct FeedService {
    feed_repository: Arc<dyn FeedRepository>,
    entry_repository: Arc<dyn EntryRepository>,
}

impl FeedService {
    pub fn new(
        feed_repository: Arc<dyn FeedRepository>,
        entry_repository: Arc<dyn EntryRepository>,
    ) -> Self {
        Self { feed_repository, entry_repository }
    }

    pub async fn add_subscription(&self, input: &AddSubscriptionInput) -> Result<Feed> {
        let url = normalize_feed_url(&Url::parse(&input.url).context("订阅 URL 不合法")?);
        Ok(self
            .feed_repository
            .upsert_subscription(&NewFeedSubscription {
                url,
                title: input.title.clone(),
                folder: input.folder.clone(),
            })
            .await?)
    }

    pub async fn remove_subscription(&self, input: RemoveSubscriptionInput) -> Result<()> {
        if input.purge_entries {
            self.entry_repository.delete_for_feed(input.feed_id).await?;
        }
        Ok(self.feed_repository.set_deleted(input.feed_id, true).await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{
        Entry, EntryNavigation, EntryQuery, EntryRepository, Feed, FeedRepository,
        NewFeedSubscription,
    };
    use time::OffsetDateTime;

    use super::{AddSubscriptionInput, FeedService, RemoveSubscriptionInput};

    struct FeedRepositoryStub {
        upserted: Mutex<Vec<NewFeedSubscription>>,
        deleted: Mutex<Vec<(i64, bool)>>,
    }

    #[async_trait::async_trait]
    impl FeedRepository for FeedRepositoryStub {
        async fn upsert_subscription(
            &self,
            new_feed: &NewFeedSubscription,
        ) -> rssr_domain::Result<Feed> {
            self.upserted.lock().expect("lock upserted").push(new_feed.clone());
            Ok(Feed {
                id: 1,
                url: new_feed.url.clone(),
                title: new_feed.title.clone(),
                site_url: None,
                description: None,
                icon_url: None,
                folder: new_feed.folder.clone(),
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
            })
        }

        async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> rssr_domain::Result<()> {
            self.deleted.lock().expect("lock deleted").push((feed_id, is_deleted));
            Ok(())
        }

        async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
            Ok(Vec::new())
        }

        async fn get_feed(&self, _feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
            Ok(None)
        }

        async fn list_summaries(&self) -> rssr_domain::Result<Vec<rssr_domain::FeedSummary>> {
            Ok(Vec::new())
        }
    }

    struct EntryRepositoryStub {
        deleted_feed_ids: Mutex<Vec<i64>>,
    }

    #[async_trait::async_trait]
    impl EntryRepository for EntryRepositoryStub {
        async fn list_entries(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<Vec<rssr_domain::EntrySummary>> {
            Ok(Vec::new())
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

        async fn delete_for_feed(&self, feed_id: i64) -> rssr_domain::Result<()> {
            self.deleted_feed_ids.lock().expect("lock deleted feeds").push(feed_id);
            Ok(())
        }
    }

    #[tokio::test]
    async fn add_subscription_normalizes_url_before_persisting() {
        let feed_repository = Arc::new(FeedRepositoryStub {
            upserted: Mutex::new(Vec::new()),
            deleted: Mutex::new(Vec::new()),
        });
        let entry_repository =
            Arc::new(EntryRepositoryStub { deleted_feed_ids: Mutex::new(Vec::new()) });
        let service = FeedService::new(feed_repository.clone(), entry_repository);

        let feed = service
            .add_subscription(&AddSubscriptionInput {
                url: "https://example.com:443/feed.xml#fragment".to_string(),
                title: Some("Example".to_string()),
                folder: Some("Tech".to_string()),
            })
            .await
            .expect("add subscription");

        assert_eq!(feed.url.as_str(), "https://example.com/feed.xml");
        let persisted = feed_repository.upserted.lock().expect("lock upserted");
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].url.as_str(), "https://example.com/feed.xml");
    }

    #[tokio::test]
    async fn remove_subscription_can_purge_entries_before_soft_delete() {
        let feed_repository = Arc::new(FeedRepositoryStub {
            upserted: Mutex::new(Vec::new()),
            deleted: Mutex::new(Vec::new()),
        });
        let entry_repository =
            Arc::new(EntryRepositoryStub { deleted_feed_ids: Mutex::new(Vec::new()) });
        let service = FeedService::new(feed_repository.clone(), entry_repository.clone());

        service
            .remove_subscription(RemoveSubscriptionInput { feed_id: 7, purge_entries: true })
            .await
            .expect("remove subscription");

        assert_eq!(
            entry_repository.deleted_feed_ids.lock().expect("lock deleted feeds").as_slice(),
            &[7]
        );
        assert_eq!(feed_repository.deleted.lock().expect("lock deleted").as_slice(), &[(7, true)]);
    }

    #[tokio::test]
    async fn remove_subscription_can_skip_entry_purge() {
        let feed_repository = Arc::new(FeedRepositoryStub {
            upserted: Mutex::new(Vec::new()),
            deleted: Mutex::new(Vec::new()),
        });
        let entry_repository =
            Arc::new(EntryRepositoryStub { deleted_feed_ids: Mutex::new(Vec::new()) });
        let service = FeedService::new(feed_repository.clone(), entry_repository.clone());

        service
            .remove_subscription(RemoveSubscriptionInput { feed_id: 8, purge_entries: false })
            .await
            .expect("remove subscription");

        assert!(entry_repository.deleted_feed_ids.lock().expect("lock deleted feeds").is_empty());
        assert_eq!(feed_repository.deleted.lock().expect("lock deleted").as_slice(), &[(8, true)]);
    }

    #[tokio::test]
    async fn add_subscription_rejects_invalid_urls() {
        let feed_repository = Arc::new(FeedRepositoryStub {
            upserted: Mutex::new(Vec::new()),
            deleted: Mutex::new(Vec::new()),
        });
        let entry_repository =
            Arc::new(EntryRepositoryStub { deleted_feed_ids: Mutex::new(Vec::new()) });
        let service = FeedService::new(feed_repository, entry_repository);

        let error = service
            .add_subscription(&AddSubscriptionInput {
                url: "not a url".to_string(),
                title: None,
                folder: None,
            })
            .await
            .expect_err("invalid url should fail");

        assert!(error.downcast_ref::<url::ParseError>().is_some());
    }
}
