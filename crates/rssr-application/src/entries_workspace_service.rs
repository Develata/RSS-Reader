use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{EntriesWorkspaceState, FeedSummary, UserSettings};

use crate::AppStateService;
use crate::library_queries::{EntriesStart, LibraryQueryPort};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntriesBootstrapInput {
    pub load_preferences: bool,
    pub load_feeds: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntriesBootstrapOutcome {
    pub settings: Option<UserSettings>,
    pub workspace: Option<EntriesWorkspaceState>,
    pub feeds: Option<Vec<FeedSummary>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveEntriesWorkspaceOutcome {
    pub changed: bool,
}

#[derive(Clone)]
pub struct EntriesWorkspaceService {
    app_state_service: AppStateService,
    queries: Arc<dyn LibraryQueryPort>,
}

impl EntriesWorkspaceService {
    pub fn new(app_state_service: AppStateService, queries: Arc<dyn LibraryQueryPort>) -> Self {
        Self { app_state_service, queries }
    }

    pub async fn bootstrap(
        &self,
        input: EntriesBootstrapInput,
    ) -> anyhow::Result<EntriesBootstrapOutcome> {
        // 设置、列表偏好与订阅来自同一个读快照。
        let EntriesStart { settings, workspace, feeds } = self
            .queries
            .entries_start(input.load_preferences, input.load_feeds)
            .await
            .context("读取文章页初始数据失败")?;

        Ok(EntriesBootstrapOutcome { settings, workspace, feeds })
    }

    /// Remembers `feed_id` as the last opened subscription, for the startup view. A read never
    /// writes it on the side (B2-D6); the page calls this when it opens a subscription.
    pub async fn remember_opened_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        self.app_state_service
            .save_last_opened_feed_id(Some(feed_id))
            .await
            .context("记住上次打开的订阅失败")
    }

    pub async fn save_workspace_if_changed(
        &self,
        next: EntriesWorkspaceState,
    ) -> anyhow::Result<SaveEntriesWorkspaceOutcome> {
        let current =
            self.app_state_service.load_entries_workspace().await.context("读取文章页偏好失败")?;
        if current == next {
            return Ok(SaveEntriesWorkspaceOutcome { changed: false });
        }

        self.app_state_service.save_entries_workspace(next).await.context("保存文章页偏好失败")?;
        Ok(SaveEntriesWorkspaceOutcome { changed: true })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{
        AppStateRepository, AppStateSnapshot, EntriesWorkspaceState, Feed, FeedRepository,
        FeedSummary, NewFeedSubscription, ReadFilter, SettingsRepository, UserSettings,
    };

    use super::{EntriesBootstrapInput, EntriesWorkspaceService};
    use crate::library_queries::test_support::RepositoryQueries;

    struct SettingsRepositoryStub {
        settings: UserSettings,
    }

    #[async_trait::async_trait]
    impl SettingsRepository for SettingsRepositoryStub {
        async fn load(&self) -> rssr_domain::Result<UserSettings> {
            Ok(self.settings.clone())
        }

        async fn save(&self, _settings: &UserSettings) -> rssr_domain::Result<()> {
            Ok(())
        }
    }

    struct AppStateRepositoryStub {
        state: Mutex<AppStateSnapshot>,
        fail_save: bool,
    }

    #[async_trait::async_trait]
    impl AppStateRepository for AppStateRepositoryStub {
        async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
            Ok(self.state.lock().expect("lock app state").clone())
        }

        async fn update(&self, mutate: rssr_domain::AppStateMutation) -> rssr_domain::Result<()> {
            if self.fail_save {
                return Err(rssr_domain::DomainError::Persistence("save failed".to_string()));
            }
            mutate(&mut self.state.lock().expect("lock app state"));
            Ok(())
        }
    }

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
                "upsert not used in entries workspace tests".to_string(),
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

    fn service(
        settings: UserSettings,
        state: AppStateSnapshot,
        feeds: Vec<i64>,
        fail_save: bool,
    ) -> EntriesWorkspaceService {
        let app_state = Arc::new(AppStateRepositoryStub { state: Mutex::new(state), fail_save });
        let queries = RepositoryQueries {
            settings: Some(Arc::new(SettingsRepositoryStub { settings })),
            app_state: Some(app_state.clone()),
            feeds: Some(Arc::new(FeedRepositoryStub {
                summaries: feeds
                    .into_iter()
                    .map(|id| FeedSummary {
                        id,
                        title: format!("Feed {id}"),
                        url: format!("https://example.com/{id}.xml"),
                        unread_count: 0,
                        entry_count: 0,
                        last_fetched_at: None,
                        last_success_at: None,
                        fetch_error: None,
                    })
                    .collect(),
            })),
            ..RepositoryQueries::default()
        };
        EntriesWorkspaceService::new(crate::AppStateService::new(app_state), Arc::new(queries))
    }

    #[tokio::test]
    async fn bootstrap_loads_requested_preferences_and_feeds() {
        let settings = UserSettings { archive_after_months: 12, ..UserSettings::default() };
        let workspace = EntriesWorkspaceState {
            read_filter: ReadFilter::UnreadOnly,
            ..EntriesWorkspaceState::default()
        };
        let state = AppStateSnapshot {
            entries_workspace: workspace.clone(),
            ..AppStateSnapshot::default()
        };

        let outcome = service(settings.clone(), state, vec![1, 2], false)
            .bootstrap(EntriesBootstrapInput { load_preferences: true, load_feeds: true })
            .await
            .expect("bootstrap entries");

        assert_eq!(outcome.settings, Some(settings));
        assert_eq!(outcome.workspace, Some(workspace));
        assert_eq!(outcome.feeds.expect("feeds").len(), 2);
    }

    /// Reading the page's start data writes nothing; remembering the opened subscription is its
    /// own write, and its failure is reported.
    #[tokio::test]
    async fn remembering_the_opened_feed_is_an_explicit_write() {
        let failing =
            service(UserSettings::default(), AppStateSnapshot::default(), Vec::new(), true);
        let outcome = failing
            .bootstrap(EntriesBootstrapInput { load_preferences: false, load_feeds: false })
            .await
            .expect("a read does not write");
        assert_eq!((outcome.settings, outcome.workspace, outcome.feeds), (None, None, None));
        assert!(failing.remember_opened_feed(7).await.is_err());

        let working =
            service(UserSettings::default(), AppStateSnapshot::default(), Vec::new(), false);
        working.remember_opened_feed(7).await.expect("remember");
    }

    #[tokio::test]
    async fn save_workspace_only_persists_changed_state() {
        let current = EntriesWorkspaceState::default();
        let unchanged = service(
            UserSettings::default(),
            AppStateSnapshot { entries_workspace: current.clone(), ..AppStateSnapshot::default() },
            Vec::new(),
            false,
        )
        .save_workspace_if_changed(current)
        .await
        .expect("save unchanged workspace");
        assert!(!unchanged.changed);

        let next = EntriesWorkspaceState {
            read_filter: ReadFilter::UnreadOnly,
            ..EntriesWorkspaceState::default()
        };
        let changed =
            service(UserSettings::default(), AppStateSnapshot::default(), Vec::new(), false)
                .save_workspace_if_changed(next)
                .await
                .expect("save changed workspace");
        assert!(changed.changed);
    }
}
