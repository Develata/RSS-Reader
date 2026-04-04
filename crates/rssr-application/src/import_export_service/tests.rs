use std::sync::{Arc, Mutex};

use rssr_domain::{
    ConfigFeed, ConfigPackage, DomainError, Entry, EntryQuery, EntryRepository, FeedRepository,
    SettingsRepository, UserSettings,
    feed::{Feed, FeedSummary, NewFeedSubscription},
};
use time::OffsetDateTime;
use url::Url;

use super::{ImportExportService, RemoteConfigStore};

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

    async fn set_deleted(&self, _feed_id: i64, _is_deleted: bool) -> rssr_domain::Result<()> {
        Ok(())
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

struct StubEntryRepository;

#[async_trait::async_trait]
impl EntryRepository for StubEntryRepository {
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
    ) -> rssr_domain::Result<rssr_domain::EntryNavigation> {
        Ok(rssr_domain::EntryNavigation::default())
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

struct StubRemoteConfigStore {
    payload: Mutex<Option<String>>,
}

#[async_trait::async_trait]
impl RemoteConfigStore for StubRemoteConfigStore {
    async fn upload_config(&self, raw: &str) -> anyhow::Result<()> {
        *self.payload.lock().expect("lock payload") = Some(raw.to_string());
        Ok(())
    }

    async fn download_config(&self) -> anyhow::Result<Option<String>> {
        Ok(self.payload.lock().expect("lock payload").clone())
    }
}

struct MemoryFeedRepository {
    feeds: Mutex<Vec<Feed>>,
}

#[async_trait::async_trait]
impl FeedRepository for MemoryFeedRepository {
    async fn upsert_subscription(
        &self,
        new_feed: &NewFeedSubscription,
    ) -> rssr_domain::Result<Feed> {
        let mut feeds = self.feeds.lock().expect("lock feeds");
        if let Some(feed) = feeds.iter_mut().find(|feed| feed.url == new_feed.url) {
            if let Some(title) = &new_feed.title {
                feed.title = (!title.is_empty()).then_some(title.clone());
            }
            if let Some(folder) = &new_feed.folder {
                feed.folder = (!folder.is_empty()).then_some(folder.clone());
            }
            feed.is_deleted = false;
            return Ok(feed.clone());
        }

        let now = OffsetDateTime::UNIX_EPOCH;
        let feed = Feed {
            id: feeds.len() as i64 + 1,
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
            created_at: now,
            updated_at: now,
        };
        feeds.push(feed.clone());
        Ok(feed)
    }

    async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> rssr_domain::Result<()> {
        let mut feeds = self.feeds.lock().expect("lock feeds");
        let Some(feed) = feeds.iter_mut().find(|feed| feed.id == feed_id) else {
            return Err(DomainError::NotFound);
        };
        feed.is_deleted = is_deleted;
        Ok(())
    }

    async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
        Ok(self.feeds.lock().expect("lock feeds").clone())
    }

    async fn get_feed(&self, feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
        Ok(self.feeds.lock().expect("lock feeds").iter().find(|feed| feed.id == feed_id).cloned())
    }

    async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
        Ok(Vec::new())
    }
}

struct MemoryEntryRepository {
    deleted_feed_ids: Mutex<Vec<i64>>,
}

#[async_trait::async_trait]
impl EntryRepository for MemoryEntryRepository {
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
    ) -> rssr_domain::Result<rssr_domain::EntryNavigation> {
        Ok(rssr_domain::EntryNavigation::default())
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

struct MemorySettingsRepository {
    settings: Mutex<UserSettings>,
}

#[async_trait::async_trait]
impl SettingsRepository for MemorySettingsRepository {
    async fn load(&self) -> rssr_domain::Result<UserSettings> {
        Ok(self.settings.lock().expect("lock settings").clone())
    }

    async fn save(&self, settings: &UserSettings) -> rssr_domain::Result<()> {
        *self.settings.lock().expect("lock settings") = settings.clone();
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
        Arc::new(StubEntryRepository),
        Arc::new(StubSettingsRepository { settings: UserSettings::default() }),
    );

    let exported = service.export_config().await.expect("export config");

    assert_eq!(exported.version, 1);
    assert_eq!(exported.feeds.len(), 1);
    assert_eq!(exported.feeds[0].url, "https://example.com/feed.xml");
    assert_eq!(exported.feeds[0].title.as_deref(), Some("Example"));
    assert_eq!(exported.feeds[0].folder.as_deref(), Some("Tech"));
}

#[tokio::test]
async fn remote_config_roundtrip_uses_json_payload() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let entry_repository =
        Arc::new(MemoryEntryRepository { deleted_feed_ids: Mutex::new(Vec::new()) });
    let service = ImportExportService::new(
        Arc::new(MemoryFeedRepository {
            feeds: Mutex::new(vec![Feed {
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
            }]),
        }),
        entry_repository.clone(),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
    );
    let remote = StubRemoteConfigStore { payload: Mutex::new(None) };

    service.push_remote_config(&remote).await.expect("push config");
    let pulled = service.pull_remote_config(&remote).await.expect("pull config");

    assert!(pulled);
    assert!(
        remote
            .payload
            .lock()
            .expect("lock payload")
            .as_ref()
            .expect("payload exists")
            .contains("\"feeds\"")
    );
}

#[tokio::test]
async fn import_config_clears_removed_feed_entries_and_metadata() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let feed_repository = Arc::new(MemoryFeedRepository {
        feeds: Mutex::new(vec![
            Feed {
                id: 1,
                url: Url::parse("https://example.com/feed.xml").expect("valid url"),
                title: Some("Legacy".to_string()),
                site_url: None,
                description: None,
                icon_url: None,
                folder: Some("Archive".to_string()),
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
                created_at: now,
                updated_at: now,
            },
            Feed {
                id: 2,
                url: Url::parse("https://stale.example.com/rss").expect("valid url"),
                title: Some("Stale".to_string()),
                site_url: None,
                description: None,
                icon_url: None,
                folder: None,
                etag: None,
                last_modified: None,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
                is_deleted: false,
                created_at: now,
                updated_at: now,
            },
        ]),
    });
    let entry_repository =
        Arc::new(MemoryEntryRepository { deleted_feed_ids: Mutex::new(Vec::new()) });
    let service = ImportExportService::new(
        feed_repository.clone(),
        entry_repository.clone(),
        Arc::new(MemorySettingsRepository { settings: Mutex::new(UserSettings::default()) }),
    );

    let package = ConfigPackage {
        version: 1,
        exported_at: OffsetDateTime::UNIX_EPOCH,
        feeds: vec![ConfigFeed {
            url: "https://example.com/feed.xml".to_string(),
            title: None,
            folder: None,
        }],
        settings: UserSettings::default(),
    };

    service.import_config_package(&package).await.expect("import package");

    let feeds = feed_repository.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 2);
    let retained = feeds.iter().find(|feed| feed.id == 1).expect("retained feed exists");
    let removed = feeds.iter().find(|feed| feed.id == 2).expect("removed feed exists");
    assert_eq!(retained.title, None);
    assert_eq!(retained.folder, None);
    assert!(removed.is_deleted);
    assert_eq!(
        entry_repository.deleted_feed_ids.lock().expect("lock deleted ids").as_slice(),
        &[2]
    );
}
