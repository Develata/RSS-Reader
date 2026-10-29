#![cfg(not(target_arch = "wasm32"))]

//! Configuration package export and import against the v0.2 library store (ports of
//! `test_config_package_io.rs`), through the library's one-transaction `ConfigReplacementPort`
//! (the only way the application replaces a configuration).

#[path = "support/library.rs"]
mod library;

use std::sync::Arc;

use library::Library;
use rssr_application::{ParsedEntryData, import_export_service::ImportExportService};
use rssr_domain::{
    ConfigFeed, ConfigPackage, FeedRepository, ListDensity, NewFeedSubscription,
    SettingsRepository, StartupView, ThemeMode, UserSettings,
};
use rssr_infra::{
    application_adapters::InfraOpmlCodec,
    config_sync::file_format::{decode_config_package, encode_config_package},
    opml::OpmlCodec,
};
use time::OffsetDateTime;
use url::Url;

fn service(library: &Library) -> ImportExportService {
    ImportExportService::new(
        Arc::new(library.feeds.clone()),
        Arc::new(library.queries.clone()),
        Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
        Arc::new(rssr_application::SystemClock),
        Arc::new(library.mutations.clone()),
    )
}

fn named(url: &str, title: &str, folder: Option<&str>) -> NewFeedSubscription {
    NewFeedSubscription {
        title: Some(title.to_string()),
        folder: folder.map(str::to_string),
        ..library::subscription(url)
    }
}

async fn roundtrip_restores_feeds_and_settings() {
    let export = Library::memory().await;
    export
        .feeds
        .upsert_subscription(&named("https://example.com/feed.xml", "Example Feed", Some("Tech")))
        .await
        .expect("create example feed");
    export
        .feeds
        .upsert_subscription(&named("https://news.example.com/rss", "News", None))
        .await
        .expect("create news feed");

    let expected_settings = UserSettings {
        theme: ThemeMode::Dark,
        list_density: ListDensity::Compact,
        startup_view: StartupView::LastFeed,
        refresh_interval_minutes: 15,
        archive_after_months: 3,
        entries_page_size: 100,
        reader_font_scale: 1.2,
        custom_css: "[data-page=\"feeds\"] .feed-card { order: 2; }".to_string(),
    };
    export.settings.save(&expected_settings).await.expect("save settings");

    let exported = service(&export).export_config().await.expect("export config");
    let encoded = encode_config_package(&exported).expect("encode config package");
    let decoded = decode_config_package(&encoded).expect("decode config package");

    let import = Library::memory().await;
    let current = service(&import).config_fingerprint().await.unwrap();
    service(&import)
        .import_config_package(&decoded, &current)
        .await
        .expect("import config package");

    let imported_feeds = import.feeds.list_feeds().await.expect("list imported feeds");
    assert_eq!(imported_feeds.len(), 2);
    assert_eq!(imported_feeds[0].folder.as_deref(), Some("Tech"));
    assert_eq!(imported_feeds[1].folder, None);

    let imported_settings = import.settings.load().await.expect("load settings");
    assert_eq!(imported_settings, expected_settings);
}

#[tokio::test]
async fn config_package_roundtrip_restores_feeds_and_settings() {
    roundtrip_restores_feeds_and_settings().await;
}

async fn import_overwrites_local_feed_membership() {
    let library = Library::memory().await;
    let service = service(&library);

    library
        .feeds
        .upsert_subscription(&named("https://stale.example.com/rss", "Stale", None))
        .await
        .expect("create stale feed");

    let package = ConfigPackage {
        version: 2,
        exported_at: OffsetDateTime::UNIX_EPOCH,
        feeds: vec![ConfigFeed {
            url: "https://fresh.example.com/feed.xml".to_string(),
            title: Some("Fresh".to_string()),
            folder: Some("Inbox".to_string()),
        }],
        settings: UserSettings::default(),
    };

    let current = service.config_fingerprint().await.unwrap();
    service.import_config_package(&package, &current).await.expect("import package");

    let feeds = library.feeds.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].url.as_str(), "https://fresh.example.com/feed.xml");
    assert_eq!(feeds[0].folder.as_deref(), Some("Inbox"));
}

#[tokio::test]
async fn config_import_overwrites_local_feed_membership() {
    import_overwrites_local_feed_membership().await;
}

async fn import_removes_dropped_feed_entries_and_clears_metadata() {
    let library = Library::memory().await;
    let service = service(&library);

    let retained_feed = library
        .feeds
        .upsert_subscription(&named("https://example.com/feed.xml", "Legacy", Some("Archive")))
        .await
        .expect("create retained feed");
    let dropped_feed = library
        .feeds
        .upsert_subscription(&named("https://stale.example.com/rss", "Stale", None))
        .await
        .expect("create dropped feed");

    library
        .seed(
            dropped_feed.id,
            vec![ParsedEntryData {
                external_id: "stale-entry".to_string(),
                dedup_key: "stale-entry".to_string(),
                url: Some(Url::parse("https://stale.example.com/article").expect("valid url")),
                title: "Old article".to_string(),
                author: None,
                summary: Some("old".to_string()),
                content_html: None,
                content_text: Some("old".to_string()),
                published_at: Some(OffsetDateTime::UNIX_EPOCH),
                updated_at_source: None,
            }],
        )
        .await;

    let current = service.config_fingerprint().await.unwrap();
    service
        .import_config_package(
            &ConfigPackage {
                version: 2,
                exported_at: OffsetDateTime::UNIX_EPOCH,
                feeds: vec![ConfigFeed {
                    url: retained_feed.url.to_string(),
                    title: None,
                    folder: None,
                }],
                settings: UserSettings::default(),
            },
            &current,
        )
        .await
        .expect("import config package");

    let feeds = library.feeds.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].url, retained_feed.url);
    assert_eq!(feeds[0].title, None);
    assert_eq!(feeds[0].folder, None);

    let remaining_entries = library
        .scalar(format!("SELECT COUNT(*) FROM entries WHERE feed_id = {}", dropped_feed.id))
        .await;
    assert_eq!(remaining_entries, 0);
}

#[tokio::test]
async fn config_import_removes_dropped_feed_entries_and_clears_metadata() {
    import_removes_dropped_feed_entries_and_clears_metadata().await;
}
