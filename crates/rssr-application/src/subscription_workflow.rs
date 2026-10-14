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

/// What adding a subscription led to, as the user interface receives it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AddSubscriptionOutcome {
    /// The address is a page listing several feeds; the user picks one.
    NeedsSelection {
        page_url: url::Url,
        candidates: Vec<crate::FeedDiscoveryCandidate>,
    },
    SavedAndRefreshed,
    /// Saved, but its first refresh failed: the subscription exists and later refreshes may
    /// succeed.
    SavedRefreshFailed {
        failure: crate::RefreshFailureItem,
    },
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
        input.subscription.url = prepared.url.to_string();
        // 激活之后订阅就对后台刷新可见；序号必须在此之前领取，之后开始的刷新才算更新。
        let ticket = self.refresh_service.prepared_update_ticket();
        let activated = self
            .feed_service
            .activate_subscription_with_site(
                &input.subscription,
                prepared.update.feed.site_url.clone(),
                // 探测到的标题是源标题，不能当作用户指定的名称，否则以后刷新就跟不上源改名。
                prepared.update.feed.title.clone(),
            )
            .await?;
        let target = RefreshTarget {
            feed_id: activated.feed.id,
            generation: activated.generation,
            url: activated.feed.url.clone(),
            etag: activated.feed.etag.clone(),
            last_modified: activated.feed.last_modified.clone(),
            ticket: activated.ticket,
        };
        let first_refresh = if input.refresh_after_add {
            Some(self.refresh_service.apply_prepared_update(target, prepared.update, ticket).await)
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
        ActivatedSubscription, FeedRefreshSourceOutput, FeedRefreshUpdate, ParsedFeedUpdate,
        RefreshHttpMetadata, RefreshStorePort, RefreshTarget, SubscriptionActivationPort,
        SubscriptionRemovalPort,
    };

    use super::{
        AddSubscriptionAndRefreshOutcome, AddSubscriptionLifecycleInput, SubscriptionWorkflow,
    };

    struct FeedRepositoryStub {
        next_id: Mutex<i64>,
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
            new_feed: &NewFeedSubscription,
        ) -> rssr_domain::Result<Feed> {
            let mut next_id = self.next_id.lock().expect("lock next id");
            let id = *next_id;
            *next_id += 1;
            Ok(Feed {
                id,
                url: new_feed.url.clone(),
                title: new_feed.title.clone(),
                source_title: new_feed.source_title.clone(),
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

    struct ActivationStub {
        next_id: Mutex<i64>,
        generation: i64,
        received: Mutex<Vec<NewFeedSubscription>>,
    }

    impl ActivationStub {
        fn new(next_id: i64) -> Self {
            Self { next_id: Mutex::new(next_id), generation: 0, received: Mutex::new(Vec::new()) }
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl SubscriptionActivationPort for ActivationStub {
        async fn activate_subscription(
            &self,
            new_feed: NewFeedSubscription,
        ) -> Result<ActivatedSubscription> {
            self.received.lock().expect("lock received").push(new_feed.clone());
            let mut next_id = self.next_id.lock().expect("lock activation id");
            let id = *next_id;
            *next_id += 1;
            Ok(ActivatedSubscription {
                feed: Feed {
                    id,
                    url: new_feed.url,
                    // 与真实适配器一样返回显示名称：指定名称优先，否则源标题。
                    title: new_feed.title.or_else(|| new_feed.source_title.clone()),
                    source_title: new_feed.source_title.clone(),
                    site_url: new_feed.site_url,
                    description: None,
                    icon_url: None,
                    folder: new_feed.folder,
                    etag: None,
                    last_modified: None,
                    last_fetched_at: None,
                    last_success_at: None,
                    fetch_error: None,
                    is_deleted: false,
                    created_at: OffsetDateTime::UNIX_EPOCH,
                    updated_at: OffsetDateTime::UNIX_EPOCH,
                },
                generation: self.generation,
                ticket: Default::default(),
            })
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
        /// Fails every content write, as a full disk would.
        fail_commits: bool,
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
            if self.fail_commits {
                return Err(rssr_domain::DomainError::Persistence("disk full".into()).into());
            }
            Ok(Default::default())
        }
    }

    fn workflow(
        next_id: i64,
        targets: Vec<RefreshTarget>,
        removal: Arc<RemovalStub>,
    ) -> SubscriptionWorkflow {
        workflow_with_activation(Arc::new(ActivationStub::new(next_id)), targets, removal)
    }

    fn workflow_with_activation(
        activation: Arc<ActivationStub>,
        targets: Vec<RefreshTarget>,
        removal: Arc<RemovalStub>,
    ) -> SubscriptionWorkflow {
        workflow_over(activation, StoreStub { targets, fail_commits: false }, removal)
    }

    fn workflow_over(
        activation: Arc<ActivationStub>,
        store: StoreStub,
        removal: Arc<RemovalStub>,
    ) -> SubscriptionWorkflow {
        let next_id = *activation.next_id.lock().expect("lock activation id");
        SubscriptionWorkflow::new(
            crate::FeedService::new(
                Arc::new(FeedRepositoryStub { next_id: Mutex::new(next_id) }),
                activation,
                removal,
            ),
            crate::RefreshService::new(Arc::new(SourceStub), Arc::new(store)),
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
                ticket: Default::default(),
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

    /// 探测到的标题是源标题，不能填进用户指定的名称，否则以后刷新就跟不上源改名（审计 B6）。
    #[tokio::test]
    async fn add_keeps_the_probed_title_apart_from_the_chosen_name() {
        for chosen in [None, Some("我的名称")] {
            let activation = Arc::new(ActivationStub::new(7));
            let workflow = workflow_with_activation(
                activation.clone(),
                Vec::new(),
                Arc::new(RemovalStub::default()),
            );
            workflow
                .add_subscription_lifecycle(AddSubscriptionLifecycleInput {
                    subscription: crate::AddSubscriptionInput {
                        url: "https://example.com/feed.xml".to_string(),
                        title: chosen.map(str::to_string),
                        folder: None,
                    },
                    refresh_after_add: false,
                })
                .await
                .expect("add without refresh");

            let received = activation.received.lock().expect("lock received");
            assert_eq!(received.len(), 1);
            assert_eq!(received[0].title.as_deref(), chosen, "only the chosen name");
            assert_eq!(received[0].source_title.as_deref(), Some("Example"), "probed title");
        }
    }

    /// Saved but its first write failed: the add succeeds with the saved subscription and a
    /// failed first refresh, not an error that would hide that the subscription exists.
    #[tokio::test]
    async fn a_failed_first_write_keeps_the_saved_subscription() {
        let workflow = workflow_over(
            Arc::new(ActivationStub::new(4)),
            StoreStub { targets: Vec::new(), fail_commits: true },
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
            .expect("the subscription is saved");

        assert_eq!(outcome.feed.id, 4);
        let refresh = outcome.first_refresh.expect("first refresh outcome");
        assert_eq!(refresh.feed_id, 4);
        assert_eq!(
            refresh.result,
            crate::RefreshFeedResult::Failed {
                kind: crate::ErrorKind::Storage,
                message: crate::ErrorKind::Storage.generic_message().to_string(),
            }
        );
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
                ticket: Default::default(),
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

    struct RecordingStore {
        misleading_current_target: RefreshTarget,
        committed_generations: Arc<Mutex<Vec<i64>>>,
    }

    #[async_trait::async_trait]
    impl RefreshStorePort for RecordingStore {
        async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
            Ok(vec![self.misleading_current_target.clone()])
        }

        async fn get_target(&self, _feed_id: i64) -> Result<Option<RefreshTarget>> {
            Ok(Some(self.misleading_current_target.clone()))
        }

        async fn commit(
            &self,
            target: &RefreshTarget,
            _commit: crate::RefreshCommit,
        ) -> Result<crate::RefreshCommitOutcome> {
            self.committed_generations
                .lock()
                .expect("lock committed generations")
                .push(target.generation);
            Ok(Default::default())
        }
    }

    #[tokio::test]
    async fn prepared_refresh_uses_generation_returned_by_activation() {
        let committed_generations = Arc::new(Mutex::new(Vec::new()));
        let misleading_current_target = RefreshTarget {
            feed_id: 5,
            generation: 99,
            url: Url::parse("https://example.com/prepared.xml").unwrap(),
            etag: None,
            last_modified: None,
            ticket: Default::default(),
        };
        let workflow = SubscriptionWorkflow::new(
            crate::FeedService::new(
                Arc::new(FeedRepositoryStub { next_id: Mutex::new(5) }),
                Arc::new(ActivationStub { generation: 7, ..ActivationStub::new(5) }),
                Arc::new(RemovalStub::default()),
            ),
            crate::RefreshService::new(
                Arc::new(SourceStub),
                Arc::new(RecordingStore {
                    misleading_current_target,
                    committed_generations: committed_generations.clone(),
                }),
            ),
            Arc::new(SourceStub),
        );

        let outcome = workflow
            .add_prepared_subscription(
                AddSubscriptionLifecycleInput {
                    subscription: crate::AddSubscriptionInput {
                        url: "https://example.com/prepared.xml".into(),
                        title: None,
                        folder: None,
                    },
                    refresh_after_add: true,
                },
                crate::PreparedSubscription {
                    url: Url::parse("https://example.com/prepared.xml").unwrap(),
                    update: FeedRefreshUpdate {
                        metadata: RefreshHttpMetadata::default(),
                        feed: ParsedFeedUpdate {
                            title: Some("Prepared".into()),
                            site_url: None,
                            description: None,
                            entries: Vec::new(),
                        },
                    },
                },
            )
            .await
            .expect("prepared refresh");

        assert_eq!(outcome.feed.id, 5);
        assert_eq!(
            committed_generations.lock().expect("lock committed generations").as_slice(),
            &[7],
            "prepared response must stay bound to the generation returned by activation"
        );
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
