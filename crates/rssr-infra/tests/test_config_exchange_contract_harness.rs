use std::sync::{Arc, Mutex};

use rssr_application::import_export_service::{ImportExportService, RemoteConfigStore};
use rssr_domain::{
    ConfigFeed, ConfigPackage, FeedRepository, ListDensity, NewFeedSubscription,
    SettingsRepository, StartupView, ThemeMode, UserSettings,
};
use rssr_infra::{
    application_adapters::{InfraOpmlCodec, SqliteAppStateAdapter},
    config_sync::file_format::{decode_config_package, encode_config_package},
    db::{
        app_state_repository::SqliteAppStateRepository, entry_repository::SqliteEntryRepository,
        feed_repository::SqliteFeedRepository, migrate,
        settings_repository::SqliteSettingsRepository, sqlite_native::NativeSqliteBackend,
        storage_backend::StorageBackend,
    },
    opml::OpmlCodec,
    parser::ParsedEntry,
};
use sqlx::Row;
use time::OffsetDateTime;
use url::Url;

#[derive(Default)]
struct MemoryRemoteConfigStore {
    payload: Mutex<Option<String>>,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigStore for MemoryRemoteConfigStore {
    async fn upload_config(&self, raw: &str) -> anyhow::Result<()> {
        *self.payload.lock().expect("lock payload") = Some(raw.to_string());
        Ok(())
    }

    async fn download_config(&self) -> anyhow::Result<Option<String>> {
        Ok(self.payload.lock().expect("lock payload").clone())
    }
}

struct SqliteFixture {
    service: ImportExportService,
    feed_repository: Arc<SqliteFeedRepository>,
    entry_repository: Arc<SqliteEntryRepository>,
    settings_repository: Arc<SqliteSettingsRepository>,
    app_state_repository: Arc<SqliteAppStateRepository>,
    pool: rssr_infra::db::SqlitePool,
}

async fn build_sqlite_fixture() -> anyhow::Result<SqliteFixture> {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await?;
    migrate(&pool).await?;

    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
    let settings_repository = Arc::new(SqliteSettingsRepository::new(pool.clone()));
    let app_state_repository = Arc::new(SqliteAppStateRepository::new(pool.clone()));
    let cleanup = Arc::new(SqliteAppStateAdapter::new(app_state_repository.clone()));
    let service = ImportExportService::new_with_app_state_cleanup(
        feed_repository.clone(),
        entry_repository.clone(),
        settings_repository.clone(),
        Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
        cleanup,
    );

    Ok(SqliteFixture {
        service,
        feed_repository,
        entry_repository,
        settings_repository,
        app_state_repository,
        pool,
    })
}

fn sample_settings() -> UserSettings {
    UserSettings {
        theme: ThemeMode::Dark,
        list_density: ListDensity::Compact,
        startup_view: StartupView::LastFeed,
        refresh_interval_minutes: 15,
        archive_after_months: 3,
        reader_font_scale: 1.2,
        custom_css: "[data-page=\"feeds\"] .feed-card { order: 2; }".to_string(),
    }
}

fn sample_entry(label: &str) -> ParsedEntry {
    ParsedEntry {
        external_id: format!("{label}-external"),
        dedup_key: format!("{label}-dedup"),
        url: Some(Url::parse(&format!("https://example.com/articles/{label}")).expect("valid url")),
        title: format!("Entry {label}"),
        author: Some("RSSR".to_string()),
        summary: Some(format!("Summary {label}")),
        content_html: Some(format!("<p>Summary {label}</p>")),
        content_text: Some(format!("Summary {label}")),
        published_at: Some(OffsetDateTime::UNIX_EPOCH),
        updated_at_source: None,
    }
}

#[tokio::test]
async fn config_exchange_contract_json_roundtrip_restores_feeds_and_settings() {
    let export_fixture = build_sqlite_fixture().await.expect("build export fixture");

    export_fixture
        .feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example Feed".to_string()),
            folder: Some("Tech".to_string()),
        })
        .await
        .expect("seed example feed");
    export_fixture
        .feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://news.example.com/rss").expect("valid url"),
            title: Some("News".to_string()),
            folder: None,
        })
        .await
        .expect("seed news feed");
    let expected_settings = sample_settings();
    export_fixture.settings_repository.save(&expected_settings).await.expect("save settings");

    let raw = export_fixture.service.export_config_json().await.expect("export config json");
    let package = decode_config_package(
        &encode_config_package(
            &serde_json::from_str::<ConfigPackage>(&raw).expect("decode export json"),
        )
        .expect("encode config package"),
    )
    .expect("decode config package");

    let import_fixture = build_sqlite_fixture().await.expect("build import fixture");
    import_fixture.service.import_config_package(&package).await.expect("import package");

    let imported_feeds = import_fixture.feed_repository.list_feeds().await.expect("list feeds");
    assert_eq!(imported_feeds.len(), 2);
    assert_eq!(imported_feeds[0].folder.as_deref(), Some("Tech"));
    assert_eq!(imported_feeds[1].folder, None);
    assert_eq!(
        import_fixture.settings_repository.load().await.expect("load settings"),
        expected_settings
    );
}

#[tokio::test]
async fn config_exchange_contract_import_cleans_removed_feed_entries_and_last_opened_state() {
    let fixture = build_sqlite_fixture().await.expect("build fixture");

    let retained_feed = fixture
        .feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Retained".to_string()),
            folder: Some("Inbox".to_string()),
        })
        .await
        .expect("seed retained feed");
    let dropped_feed = fixture
        .feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://stale.example.com/rss").expect("valid url"),
            title: Some("Dropped".to_string()),
            folder: None,
        })
        .await
        .expect("seed dropped feed");
    fixture
        .entry_repository
        .upsert_entries(dropped_feed.id, &[sample_entry("stale")])
        .await
        .expect("seed dropped feed entries");
    fixture
        .app_state_repository
        .save_last_opened_feed_id(Some(dropped_feed.id))
        .await
        .expect("save last opened feed");

    fixture
        .service
        .import_config_package(&ConfigPackage {
            version: 2,
            exported_at: OffsetDateTime::UNIX_EPOCH,
            feeds: vec![ConfigFeed {
                url: retained_feed.url.to_string(),
                title: None,
                folder: None,
            }],
            settings: UserSettings::default(),
        })
        .await
        .expect("import config package");

    let feeds = fixture.feed_repository.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].url, retained_feed.url);
    assert_eq!(feeds[0].title, None);
    assert_eq!(feeds[0].folder, None);
    assert_eq!(
        fixture.app_state_repository.load_last_opened_feed_id().await.expect("load app state"),
        None
    );

    let remaining_entries = sqlx::query("SELECT COUNT(*) AS count FROM entries WHERE feed_id = ?1")
        .bind(dropped_feed.id)
        .fetch_one(&fixture.pool)
        .await
        .expect("count dropped entries")
        .get::<i64, _>("count");
    assert_eq!(remaining_entries, 0);

    let deleted_flag = sqlx::query("SELECT is_deleted FROM feeds WHERE id = ?1")
        .bind(dropped_feed.id)
        .fetch_one(&fixture.pool)
        .await
        .expect("load dropped feed row")
        .get::<i64, _>("is_deleted");
    assert_eq!(deleted_flag, 1);
}

#[tokio::test]
async fn config_exchange_contract_opml_import_upserts_normalized_feeds() {
    let fixture = build_sqlite_fixture().await.expect("build fixture");

    fixture
        .service
        .import_opml(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Example Feed" title="Example Feed" type="rss" xmlUrl="https://example.com:443/feed.xml#fragment" />
    <outline text="Second Feed" title="Second Feed" type="rss" xmlUrl="https://second.example.com/rss" />
  </body>
</opml>"#,
        )
        .await
        .expect("import opml");

    let feeds = fixture.feed_repository.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 2);
    assert_eq!(feeds[0].url.as_str(), "https://example.com/feed.xml");
    assert_eq!(feeds[1].url.as_str(), "https://second.example.com/rss");
}

#[tokio::test]
async fn config_exchange_contract_remote_push_and_pull_roundtrip() {
    let export_fixture = build_sqlite_fixture().await.expect("build export fixture");
    export_fixture
        .feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example Feed".to_string()),
            folder: Some("Inbox".to_string()),
        })
        .await
        .expect("seed feed");
    let settings = sample_settings();
    export_fixture.settings_repository.save(&settings).await.expect("save settings");

    let remote = Arc::new(MemoryRemoteConfigStore::default());
    export_fixture.service.push_remote_config(remote.as_ref()).await.expect("push remote config");
    let payload = remote.payload.lock().expect("lock payload").clone().expect("remote payload");
    assert!(payload.contains("\"feeds\""));
    assert!(payload.contains("Example Feed"));

    let import_fixture = build_sqlite_fixture().await.expect("build import fixture");
    let pulled = import_fixture
        .service
        .pull_remote_config(remote.as_ref())
        .await
        .expect("pull remote config");
    assert!(pulled.found());
    assert_eq!(pulled.import.as_ref().expect("import outcome").imported_feed_count, 1);

    let imported_feeds = import_fixture.feed_repository.list_feeds().await.expect("list feeds");
    assert_eq!(imported_feeds.len(), 1);
    assert_eq!(imported_feeds[0].url.as_str(), "https://example.com/feed.xml");
    assert_eq!(import_fixture.settings_repository.load().await.expect("load settings"), settings);
}
