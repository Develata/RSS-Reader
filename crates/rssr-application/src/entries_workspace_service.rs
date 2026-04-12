use anyhow::Context;
use rssr_domain::{EntriesWorkspaceState, FeedSummary, UserSettings};

use crate::{AppStateService, FeedService, SettingsService};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntriesBootstrapInput {
    pub feed_id: Option<i64>,
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
    settings_service: SettingsService,
    app_state_service: AppStateService,
    feed_service: FeedService,
}

impl EntriesWorkspaceService {
    pub fn new(
        settings_service: SettingsService,
        app_state_service: AppStateService,
        feed_service: FeedService,
    ) -> Self {
        Self { settings_service, app_state_service, feed_service }
    }

    pub async fn bootstrap(
        &self,
        input: EntriesBootstrapInput,
    ) -> anyhow::Result<EntriesBootstrapOutcome> {
        if let Some(feed_id) = input.feed_id {
            let _ = self.app_state_service.save_last_opened_feed_id(Some(feed_id)).await;
        }

        let settings = if input.load_preferences {
            Some(self.settings_service.load().await.context("读取设置失败")?)
        } else {
            None
        };

        let workspace = if input.load_preferences {
            Some(
                self.app_state_service
                    .load_entries_workspace()
                    .await
                    .context("读取文章页工作区状态失败")?,
            )
        } else {
            None
        };

        let feeds = if input.load_feeds {
            Some(self.feed_service.list_feeds().await.context("读取订阅失败")?)
        } else {
            None
        };

        Ok(EntriesBootstrapOutcome { settings, workspace, feeds })
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
        AppStateRepository, AppStateSnapshot, EntriesWorkspaceState, Entry, EntryNavigation,
        EntryQuery, EntryRepository, Feed, FeedRepository, FeedSummary, NewFeedSubscription,
        ReadFilter, SettingsRepository, UserSettings,
    };

    use super::{EntriesBootstrapInput, EntriesWorkspaceService};

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

        async fn save(&self, state: &AppStateSnapshot) -> rssr_domain::Result<()> {
            if self.fail_save {
                return Err(rssr_domain::DomainError::Persistence("save failed".to_string()));
            }
            *self.state.lock().expect("lock app state") = state.clone();
            Ok(())
        }
    }

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

    struct EntryRepositoryStub;

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

        async fn delete_for_feed(&self, _feed_id: i64) -> rssr_domain::Result<()> {
            Ok(())
        }
    }

    fn service(
        settings: UserSettings,
        state: AppStateSnapshot,
        feeds: Vec<i64>,
        fail_save: bool,
    ) -> EntriesWorkspaceService {
        EntriesWorkspaceService::new(
            crate::SettingsService::new(Arc::new(SettingsRepositoryStub { settings })),
            crate::AppStateService::new(Arc::new(AppStateRepositoryStub {
                state: Mutex::new(state),
                fail_save,
            })),
            crate::FeedService::new(
                Arc::new(FeedRepositoryStub {
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
                }),
                Arc::new(EntryRepositoryStub),
            ),
        )
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
            .bootstrap(EntriesBootstrapInput {
                feed_id: None,
                load_preferences: true,
                load_feeds: true,
            })
            .await
            .expect("bootstrap entries");

        assert_eq!(outcome.settings, Some(settings));
        assert_eq!(outcome.workspace, Some(workspace));
        assert_eq!(outcome.feeds.expect("feeds").len(), 2);
    }

    #[tokio::test]
    async fn bootstrap_remembers_feed_best_effort_without_blocking() {
        let outcome =
            service(UserSettings::default(), AppStateSnapshot::default(), Vec::new(), true)
                .bootstrap(EntriesBootstrapInput {
                    feed_id: Some(7),
                    load_preferences: false,
                    load_feeds: false,
                })
                .await
                .expect("bootstrap ignores failed last feed save");

        assert_eq!(outcome.settings, None);
        assert_eq!(outcome.workspace, None);
        assert_eq!(outcome.feeds, None);
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
