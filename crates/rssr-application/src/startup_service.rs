use std::sync::Arc;

use anyhow::Context;
use rssr_domain::StartupView;

use crate::library_queries::{LibraryQueryPort, StartupState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StartupTarget {
    AllEntries,
    FeedEntries { feed_id: i64 },
}

#[derive(Clone)]
pub struct StartupService {
    queries: Arc<dyn LibraryQueryPort>,
}

impl StartupService {
    pub fn new(queries: Arc<dyn LibraryQueryPort>) -> Self {
        Self { queries }
    }

    /// 设置与“上次打开的订阅”来自同一个读快照；该订阅已删除时回到全部文章。
    pub async fn resolve_startup_target(&self) -> anyhow::Result<StartupTarget> {
        let StartupState { settings, last_opened_feed_id } =
            self.queries.startup_state().await.context("读取启动数据失败")?;
        Ok(match (settings.startup_view, last_opened_feed_id) {
            (StartupView::LastFeed, Some(feed_id)) => StartupTarget::FeedEntries { feed_id },
            (StartupView::All, _) | (StartupView::LastFeed, None) => StartupTarget::AllEntries,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{
        AppStateRepository, AppStateSnapshot, Feed, FeedRepository, FeedSummary,
        NewFeedSubscription, SettingsRepository, StartupView, UserSettings,
    };
    use time::OffsetDateTime;
    use url::Url;

    use super::{StartupService, StartupTarget};
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
    }

    #[async_trait::async_trait]
    impl AppStateRepository for AppStateRepositoryStub {
        async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
            Ok(self.state.lock().expect("lock app state").clone())
        }

        async fn update(&self, mutate: rssr_domain::AppStateMutation) -> rssr_domain::Result<()> {
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
                "upsert not used in startup tests".to_string(),
            ))
        }

        async fn set_deleted(&self, _feed_id: i64, _is_deleted: bool) -> rssr_domain::Result<()> {
            Ok(())
        }

        async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
            Ok(Vec::new())
        }

        async fn get_feed(&self, feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
            self.summaries
                .iter()
                .find(|summary| summary.id == feed_id)
                .map(|summary| {
                    Ok(Feed {
                        id: summary.id,
                        url: Url::parse(&summary.url).expect("test feed url"),
                        title: Some(summary.title.clone()),
                        source_title: None,
                        site_url: None,
                        description: None,
                        icon_url: None,
                        folder: None,
                        etag: None,
                        last_modified: None,
                        last_fetched_at: summary.last_fetched_at,
                        last_success_at: summary.last_success_at,
                        fetch_error: summary.fetch_error.clone(),
                        is_deleted: false,
                        created_at: OffsetDateTime::UNIX_EPOCH,
                        updated_at: OffsetDateTime::UNIX_EPOCH,
                    })
                })
                .transpose()
        }

        async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
            Ok(self.summaries.clone())
        }
    }

    fn service(settings: UserSettings, state: AppStateSnapshot, feeds: Vec<i64>) -> StartupService {
        let queries = RepositoryQueries {
            settings: Some(Arc::new(SettingsRepositoryStub { settings })),
            app_state: Some(Arc::new(AppStateRepositoryStub { state: Mutex::new(state) })),
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
        StartupService::new(Arc::new(queries))
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
