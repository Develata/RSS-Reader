use rssr_domain::StartupView;

use crate::{AppStateService, FeedService, SettingsService};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupTarget {
    AllEntries,
    FeedEntries { feed_id: i64 },
}

#[derive(Clone)]
pub struct StartupService {
    settings_service: SettingsService,
    app_state_service: AppStateService,
    feed_service: FeedService,
}

impl StartupService {
    pub fn new(
        settings_service: SettingsService,
        app_state_service: AppStateService,
        feed_service: FeedService,
    ) -> Self {
        Self { settings_service, app_state_service, feed_service }
    }

    pub async fn resolve_startup_target(&self) -> anyhow::Result<StartupTarget> {
        let settings = self.settings_service.load().await?;
        if settings.startup_view == StartupView::All {
            return Ok(StartupTarget::AllEntries);
        }

        let Some(feed_id) = self.app_state_service.load_last_opened_feed_id().await.ok().flatten()
        else {
            return Ok(StartupTarget::AllEntries);
        };

        let feed_exists = self
            .feed_service
            .list_feeds()
            .await
            .map(|feeds| feeds.iter().any(|feed| feed.id == feed_id))
            .unwrap_or(false);

        if feed_exists {
            Ok(StartupTarget::FeedEntries { feed_id })
        } else {
            Ok(StartupTarget::AllEntries)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{
        AppStateRepository, AppStateSnapshot, Entry, EntryNavigation, EntryQuery, EntryRepository,
        Feed, FeedRepository, FeedSummary, NewFeedSubscription, SettingsRepository, StartupView,
        UserSettings,
    };

    use super::{StartupService, StartupTarget};

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
    }

    #[async_trait::async_trait]
    impl AppStateRepository for AppStateRepositoryStub {
        async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
            Ok(self.state.lock().expect("lock app state").clone())
        }

        async fn save(&self, state: &AppStateSnapshot) -> rssr_domain::Result<()> {
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
                "upsert not used in startup tests".to_string(),
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

    fn service(settings: UserSettings, state: AppStateSnapshot, feeds: Vec<i64>) -> StartupService {
        StartupService::new(
            crate::SettingsService::new(Arc::new(SettingsRepositoryStub { settings })),
            crate::AppStateService::new(Arc::new(AppStateRepositoryStub {
                state: Mutex::new(state),
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
    async fn startup_target_uses_all_entries_for_all_view() {
        let target = service(UserSettings::default(), AppStateSnapshot::default(), vec![7])
            .resolve_startup_target()
            .await
            .expect("resolve startup");

        assert_eq!(target, StartupTarget::AllEntries);
    }

    #[tokio::test]
    async fn startup_target_uses_last_feed_when_it_exists() {
        let settings =
            UserSettings { startup_view: StartupView::LastFeed, ..UserSettings::default() };
        let state =
            AppStateSnapshot { last_opened_feed_id: Some(7), ..AppStateSnapshot::default() };

        let target = service(settings, state, vec![7])
            .resolve_startup_target()
            .await
            .expect("resolve startup");

        assert_eq!(target, StartupTarget::FeedEntries { feed_id: 7 });
    }

    #[tokio::test]
    async fn startup_target_falls_back_when_last_feed_is_missing() {
        let settings =
            UserSettings { startup_view: StartupView::LastFeed, ..UserSettings::default() };
        let state =
            AppStateSnapshot { last_opened_feed_id: Some(7), ..AppStateSnapshot::default() };

        let target = service(settings, state, vec![9])
            .resolve_startup_target()
            .await
            .expect("resolve startup");

        assert_eq!(target, StartupTarget::AllEntries);
    }
}
