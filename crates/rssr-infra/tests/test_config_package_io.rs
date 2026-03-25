use std::sync::Arc;

use rssr_application::import_export_service::ImportExportService;
use rssr_domain::{
    ConfigFeed, ConfigPackage, FeedRepository, ListDensity, NewFeedSubscription,
    SettingsRepository, StartupView, ThemeMode, UserSettings,
};
use rssr_infra::{
    config_sync::file_format::{decode_config_package, encode_config_package},
    db::{
        feed_repository::SqliteFeedRepository, migrate,
        settings_repository::SqliteSettingsRepository, sqlite_native::NativeSqliteBackend,
        storage_backend::StorageBackend,
    },
};
use time::OffsetDateTime;
use url::Url;

#[tokio::test]
async fn config_package_roundtrip_restores_feeds_and_settings() {
    let export_backend = NativeSqliteBackend::new("sqlite::memory:");
    let export_pool = export_backend.connect().await.expect("connect export sqlite");
    migrate(&export_pool).await.expect("migrate export sqlite");

    let export_feed_repository = Arc::new(SqliteFeedRepository::new(export_pool.clone()));
    let export_settings_repository = Arc::new(SqliteSettingsRepository::new(export_pool));

    export_feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example Feed".to_string()),
            folder: Some("Tech".to_string()),
        })
        .await
        .expect("create example feed");
    export_feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://news.example.com/rss").expect("valid url"),
            title: Some("News".to_string()),
            folder: None,
        })
        .await
        .expect("create news feed");

    let expected_settings = UserSettings {
        theme: ThemeMode::Dark,
        list_density: ListDensity::Compact,
        startup_view: StartupView::LastFeed,
        refresh_interval_minutes: 15,
        reader_font_scale: 1.2,
    };
    export_settings_repository.save(&expected_settings).await.expect("save settings");

    let export_service = ImportExportService::new(
        export_feed_repository.clone(),
        export_settings_repository.clone(),
    );
    let exported = export_service.export_config().await.expect("export config");
    let encoded = encode_config_package(&exported).expect("encode config package");
    let decoded = decode_config_package(&encoded).expect("decode config package");

    let import_backend = NativeSqliteBackend::new("sqlite::memory:");
    let import_pool = import_backend.connect().await.expect("connect import sqlite");
    migrate(&import_pool).await.expect("migrate import sqlite");

    let import_feed_repository = Arc::new(SqliteFeedRepository::new(import_pool.clone()));
    let import_settings_repository = Arc::new(SqliteSettingsRepository::new(import_pool));
    let import_service = ImportExportService::new(
        import_feed_repository.clone(),
        import_settings_repository.clone(),
    );

    import_service.import_config_package(&decoded).await.expect("import config package");

    let imported_feeds = import_feed_repository.list_feeds().await.expect("list imported feeds");
    assert_eq!(imported_feeds.len(), 2);
    assert_eq!(imported_feeds[0].folder.as_deref(), Some("Tech"));
    assert_eq!(imported_feeds[1].folder, None);

    let imported_settings = import_settings_repository.load().await.expect("load settings");
    assert_eq!(imported_settings, expected_settings);
}

#[tokio::test]
async fn config_import_overwrites_local_feed_membership() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite");
    migrate(&pool).await.expect("migrate sqlite");

    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let settings_repository = Arc::new(SqliteSettingsRepository::new(pool));
    let service = ImportExportService::new(feed_repository.clone(), settings_repository);

    feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://stale.example.com/rss").expect("valid url"),
            title: Some("Stale".to_string()),
            folder: None,
        })
        .await
        .expect("create stale feed");

    let package = ConfigPackage {
        version: 1,
        exported_at: OffsetDateTime::UNIX_EPOCH,
        feeds: vec![ConfigFeed {
            url: "https://fresh.example.com/feed.xml".to_string(),
            title: Some("Fresh".to_string()),
            folder: Some("Inbox".to_string()),
        }],
        settings: UserSettings::default(),
    };

    service.import_config_package(&package).await.expect("import package");

    let feeds = feed_repository.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].url.as_str(), "https://fresh.example.com/feed.xml");
    assert_eq!(feeds[0].folder.as_deref(), Some("Inbox"));
}
