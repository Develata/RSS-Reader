use std::sync::Arc;

use anyhow::Result;
use rssr_domain::Feed;

use crate::{
    AddSubscriptionInput, FeedService, RefreshFeedOutcome, RefreshService, RefreshTarget,
    RemoveSubscriptionInput,
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
    pub(crate) feed_service: FeedService,
    pub(crate) probe: Arc<dyn crate::SubscriptionProbePort>,
    refresh_service: RefreshService,
}

impl SubscriptionWorkflow {
    pub fn new(
        feed_service: FeedService,
        refresh_service: RefreshService,
        probe: Arc<dyn crate::SubscriptionProbePort>,
    ) -> Self {
        Self { feed_service, refresh_service, probe }
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
        let prepared = match self.prepare_subscription(&input.subscription.url).await? {
            crate::PrepareSubscriptionOutcome::Ready(prepared) => prepared,
            crate::PrepareSubscriptionOutcome::NeedsSelection { candidates, .. } => {
                anyhow::bail!(
                    "发现多个订阅，请选择 feed URL 后重新添加：\n{}",
                    candidates
                        .iter()
                        .map(|candidate| format!(
                            "{} {}",
                            candidate.url,
                            candidate.title.as_deref().unwrap_or("")
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        };
        self.add_prepared_subscription(input, prepared).await
    }

    pub async fn add_prepared_subscription(
        &self,
        mut input: AddSubscriptionLifecycleInput,
        prepared: crate::PreparedSubscription,
    ) -> Result<AddSubscriptionLifecycleOutcome> {
        self.feed_service.ensure_not_subscribed(&prepared.url).await?;
        input.subscription.url = prepared.url.to_string();
        if input.subscription.title.is_none() {
            input.subscription.title = prepared.update.feed.title.clone();
        }
        let activated = self
            .feed_service
            .activate_subscription_with_site(
                &input.subscription,
                prepared.update.feed.site_url.clone(),
            )
            .await?;
        let target = RefreshTarget {
            feed_id: activated.feed.id,
            generation: activated.generation,
            url: activated.feed.url.clone(),
            etag: activated.feed.etag.clone(),
            last_modified: activated.feed.last_modified.clone(),
        };
        let first_refresh = if input.refresh_after_add {
            Some(self.refresh_service.apply_prepared_update(target, prepared.update).await?)
        } else {
            None
        };
        Ok(AddSubscriptionLifecycleOutcome { feed: activated.feed, first_refresh })
    }

    pub async fn remove_subscription(&self, input: RemoveSubscriptionInput) -> Result<()> {
        self.feed_service.remove_subscription(input).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::Result;
    use rssr_domain::{Feed, FeedRepository, FeedSummary, NewFeedSubscription};
    use time::OffsetDateTime;
    use url::Url;

    use crate::{
        FeedRefreshSourceOutput, FeedRefreshUpdate, ParsedFeedUpdate, RefreshHttpMetadata,
        RefreshStorePort, RefreshTarget, SubscriptionRemovalPort,
    };

    use super::{
        AddSubscriptionAndRefreshOutcome, AddSubscriptionLifecycleInput, SubscriptionWorkflow,
    };

    struct FeedRepositoryStub {
        next_id: Mutex<i64>,
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
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct RemovalStub {
        calls: Mutex<Vec<(i64, bool)>>,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl SubscriptionRemovalPort for RemovalStub {
        async fn remove_subscription(&self, feed_id: i64, purge_entries: bool) -> Result<()> {
            self.calls.lock().expect("lock removal calls").push((feed_id, purge_entries));
            Ok(())
        }
    }

    struct SourceStub;

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl crate::SubscriptionProbePort for SourceStub {
        async fn probe(&self, url: &Url) -> Result<crate::SubscriptionProbeOutcome> {
            Ok(crate::SubscriptionProbeOutcome::Feed {
                url: url.clone(),
                update: FeedRefreshUpdate {
                    metadata: RefreshHttpMetadata::default(),
                    feed: ParsedFeedUpdate {
                        title: Some("Example".into()),
                        site_url: None,
                        description: None,
                        entries: Vec::new(),
                    },
                },
            })
        }
    }

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

        async fn commit(
            &self,
            _target: &RefreshTarget,
            _commit: crate::RefreshCommit,
        ) -> Result<crate::RefreshCommitOutcome> {
            Ok(Default::default())
        }
    }

    fn workflow(
        next_id: i64,
        targets: Vec<RefreshTarget>,
        removal: Arc<RemovalStub>,
    ) -> SubscriptionWorkflow {
        SubscriptionWorkflow::new(
            crate::FeedService::new(
                Arc::new(FeedRepositoryStub { next_id: Mutex::new(next_id) }),
                removal,
            ),
            crate::RefreshService::new(Arc::new(SourceStub), Arc::new(StoreStub { targets })),
            Arc::new(SourceStub),
        )
    }

    #[tokio::test]
    async fn add_and_refresh_combines_feed_and_refresh_use_cases() {
        let workflow = workflow(
            1,
            vec![RefreshTarget {
                feed_id: 1,
                generation: 0,
                url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                etag: None,
                last_modified: None,
            }],
            Arc::new(RemovalStub::default()),
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
        let workflow = workflow(7, Vec::new(), Arc::new(RemovalStub::default()));

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
        let workflow = workflow(
            3,
            vec![RefreshTarget {
                feed_id: 3,
                generation: 0,
                url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                etag: None,
                last_modified: None,
            }],
            Arc::new(RemovalStub::default()),
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
    async fn remove_subscription_delegates_exactly_once() {
        let removal = Arc::new(RemovalStub::default());
        let workflow = workflow(1, Vec::new(), removal.clone());

        workflow
            .remove_subscription(crate::RemoveSubscriptionInput { feed_id: 9, purge_entries: true })
            .await
            .expect("remove subscription");

        assert_eq!(removal.calls.lock().expect("lock removal calls").as_slice(), &[(9, true)]);
    }
}
