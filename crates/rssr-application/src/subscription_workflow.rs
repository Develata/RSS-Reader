use std::sync::Arc;

use anyhow::Result;
use rssr_domain::Feed;

use crate::{
    AddSubscriptionInput, FeedService, RefreshFeedOutcome, RefreshService, RemoveSubscriptionInput,
};

#[async_trait::async_trait]
pub trait AppStatePort: Send + Sync {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddSubscriptionAndRefreshOutcome {
    pub feed: Feed,
    pub refresh: RefreshFeedOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddSubscriptionLifecycleInput {
    pub subscription: AddSubscriptionInput,
    pub refresh_after_add: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddSubscriptionLifecycleOutcome {
    pub feed: Feed,
    pub first_refresh: Option<RefreshFeedOutcome>,
}

#[derive(Clone)]
pub struct SubscriptionWorkflow {
    feed_service: FeedService,
    refresh_service: RefreshService,
    app_state: Arc<dyn AppStatePort>,
}

impl SubscriptionWorkflow {
    pub fn new(
        feed_service: FeedService,
        refresh_service: RefreshService,
        app_state: Arc<dyn AppStatePort>,
    ) -> Self {
        Self { feed_service, refresh_service, app_state }
    }

    pub async fn add_subscription(&self, input: &AddSubscriptionInput) -> Result<Feed> {
        Ok(self
            .add_subscription_lifecycle(AddSubscriptionLifecycleInput {
                subscription: input.clone(),
                refresh_after_add: false,
            })
            .await?
            .feed)
    }

    pub async fn add_subscription_and_refresh(
        &self,
        input: &AddSubscriptionInput,
    ) -> Result<AddSubscriptionAndRefreshOutcome> {
        let outcome = self
            .add_subscription_lifecycle(AddSubscriptionLifecycleInput {
                subscription: input.clone(),
                refresh_after_add: true,
            })
            .await?;
        let refresh = outcome.first_refresh.expect("refresh_after_add produces refresh outcome");
        Ok(AddSubscriptionAndRefreshOutcome { feed: outcome.feed, refresh })
    }

    pub async fn add_subscription_lifecycle(
        &self,
        input: AddSubscriptionLifecycleInput,
    ) -> Result<AddSubscriptionLifecycleOutcome> {
        let feed = self.feed_service.add_subscription(&input.subscription).await?;
        let first_refresh = if input.refresh_after_add {
            Some(self.refresh_service.refresh_feed(feed.id).await?)
        } else {
            None
        };
        Ok(AddSubscriptionLifecycleOutcome { feed, first_refresh })
    }

    pub async fn remove_subscription(&self, input: RemoveSubscriptionInput) -> Result<()> {
        self.feed_service.remove_subscription(input).await?;
        self.app_state.clear_last_opened_feed_if_matches(input.feed_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::Result;
    use rssr_domain::{
        EntryContent, EntryContentRepository, EntryIndexRepository, EntryNavigation, EntryQuery,
        EntryRecord, Feed, FeedRepository, FeedSummary, NewFeedSubscription,
    };
    use time::OffsetDateTime;
    use url::Url;

    use crate::{
        FeedRefreshSourceOutput, FeedRefreshUpdate, ParsedFeedUpdate, RefreshHttpMetadata,
        RefreshStorePort, RefreshTarget,
    };

    use super::{
        AddSubscriptionAndRefreshOutcome, AddSubscriptionLifecycleInput, AppStatePort,
        SubscriptionWorkflow,
    };

    struct FeedRepositoryStub {
        next_id: Mutex<i64>,
        deleted_feed_ids: Mutex<Vec<i64>>,
    }

    #[async_trait::async_trait]
    impl FeedRepository for FeedRepositoryStub {
        async fn upsert_subscription(
            &self,
            new_feed: &NewFeedSubscription,
        ) -> rssr_domain::Result<Feed> {
            let mut next_id = self.next_id.lock().expect("lock next id");
            let id = *next_id;
            *next_id += 1;
            Ok(Feed {
                id,
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
            if is_deleted {
                self.deleted_feed_ids.lock().expect("lock deleted ids").push(feed_id);
            }
            Ok(())
        }

        async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
            Ok(Vec::new())
        }

        async fn get_feed(&self, _feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
            Ok(None)
        }

        async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
            Ok(Vec::new())
        }
    }

    struct EntryRepositoryStub {
        deleted_feed_ids: Mutex<Vec<i64>>,
    }

    #[async_trait::async_trait]
    impl EntryIndexRepository for EntryRepositoryStub {
        async fn list_entries(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<Vec<rssr_domain::EntrySummary>> {
            Ok(Vec::new())
        }

        async fn count_entries(&self, _query: &EntryQuery) -> rssr_domain::Result<u64> {
            Ok(0)
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

        async fn set_read(&self, _entry_id: i64, _is_read: bool) -> rssr_domain::Result<()> {
            Ok(())
        }

        async fn set_starred(&self, _entry_id: i64, _is_starred: bool) -> rssr_domain::Result<()> {
            Ok(())
        }

        async fn delete_for_feed(&self, feed_id: i64) -> rssr_domain::Result<()> {
            self.deleted_feed_ids.lock().expect("lock deleted feed ids").push(feed_id);
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl EntryContentRepository for EntryRepositoryStub {
        async fn get_content(&self, _entry_id: i64) -> rssr_domain::Result<Option<EntryContent>> {
            Ok(None)
        }

        async fn delete_for_feed(&self, _feed_id: i64) -> rssr_domain::Result<()> {
            Ok(())
        }

        async fn delete_for_entry_ids(&self, _entry_ids: &[i64]) -> rssr_domain::Result<()> {
            Ok(())
        }
    }

    struct SourceStub;

    #[async_trait::async_trait]
    impl crate::FeedRefreshSourcePort for SourceStub {
        async fn refresh(&self, _target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
            Ok(FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
                metadata: RefreshHttpMetadata::default(),
                feed: ParsedFeedUpdate {
                    title: Some("Example".to_string()),
                    site_url: None,
                    description: None,
                    entries: Vec::new(),
                },
            }))
        }
    }

    struct StoreStub {
        targets: Vec<RefreshTarget>,
    }

    #[async_trait::async_trait]
    impl RefreshStorePort for StoreStub {
        async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
            Ok(self.targets.clone())
        }

        async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
            Ok(self.targets.iter().find(|target| target.feed_id == feed_id).cloned())
        }

        async fn commit(&self, _feed_id: i64, _commit: crate::RefreshCommit) -> Result<()> {
            Ok(())
        }
    }

    struct AppStateStub {
        cleared_feed_ids: Mutex<Vec<i64>>,
    }

    #[async_trait::async_trait]
    impl AppStatePort for AppStateStub {
        async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
            self.cleared_feed_ids.lock().expect("lock cleared ids").push(feed_id);
            Ok(())
        }
    }

    #[tokio::test]
    async fn add_and_refresh_combines_feed_and_refresh_use_cases() {
        let entry_repository =
            Arc::new(EntryRepositoryStub { deleted_feed_ids: Mutex::new(Vec::new()) });
        let feed_service = crate::FeedService::new(
            Arc::new(FeedRepositoryStub {
                next_id: Mutex::new(1),
                deleted_feed_ids: Mutex::new(Vec::new()),
            }),
            entry_repository.clone(),
            entry_repository,
        );
        let refresh_service = crate::RefreshService::new(
            Arc::new(SourceStub),
            Arc::new(StoreStub {
                targets: vec![RefreshTarget {
                    feed_id: 1,
                    url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                    etag: None,
                    last_modified: None,
                }],
            }),
        );
        let workflow = SubscriptionWorkflow::new(
            feed_service,
            refresh_service,
            Arc::new(AppStateStub { cleared_feed_ids: Mutex::new(Vec::new()) }),
        );

        let outcome: AddSubscriptionAndRefreshOutcome = workflow
            .add_subscription_and_refresh(&crate::AddSubscriptionInput {
                url: "https://example.com/feed.xml".to_string(),
                title: None,
                folder: None,
            })
            .await
            .expect("add and refresh");

        assert_eq!(outcome.feed.id, 1);
        assert!(matches!(outcome.refresh.result, crate::RefreshFeedResult::Updated { .. }));
    }

    #[tokio::test]
    async fn add_lifecycle_can_skip_first_refresh() {
        let entry_repository =
            Arc::new(EntryRepositoryStub { deleted_feed_ids: Mutex::new(Vec::new()) });
        let workflow = SubscriptionWorkflow::new(
            crate::FeedService::new(
                Arc::new(FeedRepositoryStub {
                    next_id: Mutex::new(7),
                    deleted_feed_ids: Mutex::new(Vec::new()),
                }),
                entry_repository.clone(),
                entry_repository,
            ),
            crate::RefreshService::new(
                Arc::new(SourceStub),
                Arc::new(StoreStub { targets: Vec::new() }),
            ),
            Arc::new(AppStateStub { cleared_feed_ids: Mutex::new(Vec::new()) }),
        );

        let outcome = workflow
            .add_subscription_lifecycle(AddSubscriptionLifecycleInput {
                subscription: crate::AddSubscriptionInput {
                    url: "https://example.com/feed.xml".to_string(),
                    title: None,
                    folder: None,
                },
                refresh_after_add: false,
            })
            .await
            .expect("add without refresh");

        assert_eq!(outcome.feed.id, 7);
        assert_eq!(outcome.first_refresh, None);
    }

    #[tokio::test]
    async fn add_lifecycle_can_run_first_refresh() {
        let entry_repository =
            Arc::new(EntryRepositoryStub { deleted_feed_ids: Mutex::new(Vec::new()) });
        let workflow = SubscriptionWorkflow::new(
            crate::FeedService::new(
                Arc::new(FeedRepositoryStub {
                    next_id: Mutex::new(3),
                    deleted_feed_ids: Mutex::new(Vec::new()),
                }),
                entry_repository.clone(),
                entry_repository,
            ),
            crate::RefreshService::new(
                Arc::new(SourceStub),
                Arc::new(StoreStub {
                    targets: vec![RefreshTarget {
                        feed_id: 3,
                        url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                        etag: None,
                        last_modified: None,
                    }],
                }),
            ),
            Arc::new(AppStateStub { cleared_feed_ids: Mutex::new(Vec::new()) }),
        );

        let outcome = workflow
            .add_subscription_lifecycle(AddSubscriptionLifecycleInput {
                subscription: crate::AddSubscriptionInput {
                    url: "https://example.com/feed.xml".to_string(),
                    title: None,
                    folder: None,
                },
                refresh_after_add: true,
            })
            .await
            .expect("add with first refresh");

        assert_eq!(outcome.feed.id, 3);
        let refresh = outcome.first_refresh.expect("first refresh outcome");
        assert!(matches!(refresh.result, crate::RefreshFeedResult::Updated { .. }));
    }

    #[tokio::test]
    async fn remove_subscription_clears_last_opened_state_after_feed_removal() {
        let app_state = Arc::new(AppStateStub { cleared_feed_ids: Mutex::new(Vec::new()) });
        let feed_repository = Arc::new(FeedRepositoryStub {
            next_id: Mutex::new(1),
            deleted_feed_ids: Mutex::new(Vec::new()),
        });
        let entry_repository =
            Arc::new(EntryRepositoryStub { deleted_feed_ids: Mutex::new(Vec::new()) });
        let workflow = SubscriptionWorkflow::new(
            crate::FeedService::new(
                feed_repository.clone(),
                entry_repository.clone(),
                entry_repository,
            ),
            crate::RefreshService::new(
                Arc::new(SourceStub),
                Arc::new(StoreStub { targets: Vec::new() }),
            ),
            app_state.clone(),
        );

        workflow
            .remove_subscription(crate::RemoveSubscriptionInput { feed_id: 9, purge_entries: true })
            .await
            .expect("remove subscription");

        assert_eq!(
            feed_repository.deleted_feed_ids.lock().expect("lock deleted ids").as_slice(),
            &[9]
        );
        assert_eq!(app_state.cleared_feed_ids.lock().expect("lock cleared ids").as_slice(), &[9]);
    }
}
