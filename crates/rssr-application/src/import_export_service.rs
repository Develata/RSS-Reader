use std::sync::Arc;

use anyhow::Result;
use rssr_domain::{ConfigFeed, ConfigPackage, FeedRepository, SettingsRepository};
use time::OffsetDateTime;

pub struct ImportExportService {
    feed_repository: Arc<dyn FeedRepository>,
    settings_repository: Arc<dyn SettingsRepository>,
}

impl ImportExportService {
    pub fn new(
        feed_repository: Arc<dyn FeedRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
    ) -> Self {
        Self { feed_repository, settings_repository }
    }

    pub async fn export_config(&self) -> Result<ConfigPackage> {
        let feeds = self
            .feed_repository
            .list_feeds()
            .await?
            .into_iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| ConfigFeed {
                url: feed.url.to_string(),
                title: feed.title,
                folder: feed.folder,
            })
            .collect();

        let settings = self.settings_repository.load().await?;

        Ok(ConfigPackage { version: 1, exported_at: OffsetDateTime::now_utc(), feeds, settings })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{
        DomainError, FeedRepository, SettingsRepository, UserSettings,
        feed::{Feed, FeedSummary, NewFeedSubscription},
    };
    use time::OffsetDateTime;
    use url::Url;

    use super::ImportExportService;

    struct StubFeedRepository {
        feeds: Vec<Feed>,
    }

    #[async_trait::async_trait]
    impl FeedRepository for StubFeedRepository {
        async fn upsert_subscription(
            &self,
            _new_feed: &NewFeedSubscription,
        ) -> rssr_domain::Result<Feed> {
            Err(DomainError::Persistence("not used in test".into()))
        }

        async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
            Ok(self.feeds.clone())
        }

        async fn get_feed(&self, feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
            Ok(self.feeds.iter().find(|feed| feed.id == feed_id).cloned())
        }

        async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
            Ok(Vec::new())
        }
    }

    struct StubSettingsRepository {
        settings: UserSettings,
    }

    #[async_trait::async_trait]
    impl SettingsRepository for StubSettingsRepository {
        async fn load(&self) -> rssr_domain::Result<UserSettings> {
            Ok(self.settings.clone())
        }

        async fn save(&self, _settings: &UserSettings) -> rssr_domain::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn export_config_contains_active_feeds_and_settings() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let active_feed = Feed {
            id: 1,
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
            created_at: now,
            updated_at: now,
        };
        let deleted_feed = Feed {
            id: 2,
            url: Url::parse("https://example.com/deleted.xml").expect("valid url"),
            title: Some("Deleted".to_string()),
            site_url: None,
            description: None,
            icon_url: None,
            folder: Some("Archive".to_string()),
            etag: None,
            last_modified: None,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
            is_deleted: true,
            created_at: now,
            updated_at: now,
        };

        let service = ImportExportService::new(
            Arc::new(StubFeedRepository { feeds: vec![active_feed, deleted_feed] }),
            Arc::new(StubSettingsRepository { settings: UserSettings::default() }),
        );

        let exported = service.export_config().await.expect("export config");

        assert_eq!(exported.version, 1);
        assert_eq!(exported.feeds.len(), 1);
        assert_eq!(exported.feeds[0].url, "https://example.com/feed.xml");
        assert_eq!(exported.feeds[0].title.as_deref(), Some("Example"));
        assert_eq!(exported.feeds[0].folder.as_deref(), Some("Tech"));
    }
}
