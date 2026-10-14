#![cfg(not(target_arch = "wasm32"))]

//! The configuration-exchange contract run against the v0.2 library store (ports of
//! `test_config_exchange_contract_harness.rs`).
//!
//! Every case runs through the shipping path: the library's one-transaction
//! `ConfigReplacementPort` (the application has no other way to replace a configuration).

#[path = "support/library.rs"]
mod library;

use std::sync::{Arc, Mutex};

use library::Library;
use rssr_application::ParsedEntryData;
use rssr_application::import_export_service::{ImportExportService, RemoteConfigStore};
use rssr_domain::{
    AppStateRepository, ConfigFeed, ConfigPackage, FeedRepository, ListDensity,
    NewFeedSubscription, SettingsRepository, StartupView, ThemeMode, UserSettings,
};
use rssr_infra::{
    application_adapters::InfraOpmlCodec,
    config_sync::file_format::{decode_config_package, encode_config_package},
    opml::OpmlCodec,
};
use time::OffsetDateTime;
use url::Url;

#[derive(Default)]
struct MemoryRemoteConfigStore {
    payload: Mutex<Option<String>>,
}

#[async_trait::async_trait]
impl RemoteConfigStore for MemoryRemoteConfigStore {
    async fn upload_config(&self, raw: &str) -> anyhow::Result<()> {
        *self.payload.lock().expect("lock payload") = Some(raw.to_string());
        Ok(())
    }

    async fn download_config(&self) -> anyhow::Result<Option<String>> {
        Ok(self.payload.lock().expect("lock payload").clone())
    }
}

struct Fixture {
    service: ImportExportService,
    library: Library,
}

async fn build_fixture() -> Fixture {
    let library = Library::memory().await;
    let service = ImportExportService::new(
        Arc::new(library.feeds.clone()),
        Arc::new(library.queries.clone()),
        Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
        Arc::new(rssr_application::SystemClock),
        Arc::new(library.mutations.clone()),
    );
    Fixture { service, library }
}

async fn set_last_opened(library: &Library, feed_id: i64) {
    library
        .app_state
        .update(Box::new(move |state| state.last_opened_feed_id = Some(feed_id)))
        .await
        .expect("save last opened feed");
}

async fn last_opened(library: &Library) -> Option<i64> {
    library.app_state.load().await.expect("load app state").last_opened_feed_id
}

fn named(url: &str, title: &str, folder: Option<&str>) -> NewFeedSubscription {
    NewFeedSubscription {
        title: Some(title.to_string()),
        folder: folder.map(str::to_string),
        ..library::subscription(url)
    }
}

fn sample_settings() -> UserSettings {
    UserSettings {
        theme: ThemeMode::Dark,
        list_density: ListDensity::Compact,
        startup_view: StartupView::LastFeed,
        refresh_interval_minutes: 15,
        archive_after_months: 3,
        entries_page_size: 100,
        reader_font_scale: 1.2,
        custom_css: "[data-page=\"feeds\"] .feed-card { order: 2; }".to_string(),
    }
}

fn sample_entry(label: &str) -> ParsedEntryData {
    ParsedEntryData {
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

async fn json_roundtrip_restores_feeds_and_settings() {
    let export_fixture = build_fixture().await;
    let export = &export_fixture.library;

    export
        .feeds
        .upsert_subscription(&named("https://example.com/feed.xml", "Example Feed", Some("Tech")))
        .await
        .expect("seed example feed");
    export
        .feeds
        .upsert_subscription(&named("https://news.example.com/rss", "News", None))
        .await
        .expect("seed news feed");
    let expected_settings = sample_settings();
    export.settings.save(&expected_settings).await.expect("save settings");

    let raw = export_fixture.service.export_config_json().await.expect("export config json");
    let package = decode_config_package(
        &encode_config_package(
            &serde_json::from_str::<ConfigPackage>(&raw).expect("decode export json"),
        )
        .expect("encode config package"),
    )
    .expect("decode config package");

    let import_fixture = build_fixture().await;
    let current = import_fixture.service.config_fingerprint().await.unwrap();
    import_fixture.service.import_config_package(&package, &current).await.expect("import package");

    let imported_feeds = import_fixture.library.feeds.list_feeds().await.expect("list feeds");
    assert_eq!(imported_feeds.len(), 2);
    assert_eq!(imported_feeds[0].folder.as_deref(), Some("Tech"));
    assert_eq!(imported_feeds[1].folder, None);
    assert_eq!(
        import_fixture.library.settings.load().await.expect("load settings"),
        expected_settings
    );
}

#[tokio::test]
async fn config_exchange_contract_json_roundtrip_restores_feeds_and_settings() {
    json_roundtrip_restores_feeds_and_settings().await;
}

async fn import_cleans_removed_feed_entries_and_last_opened_state() {
    let fixture = build_fixture().await;
    let library = &fixture.library;

    let retained_feed = library
        .feeds
        .upsert_subscription(&named("https://example.com/feed.xml", "Retained", Some("Inbox")))
        .await
        .expect("seed retained feed");
    let dropped_feed = library
        .feeds
        .upsert_subscription(&named("https://stale.example.com/rss", "Dropped", None))
        .await
        .expect("seed dropped feed");
    library.seed(dropped_feed.id, vec![sample_entry("stale")]).await;
    set_last_opened(library, dropped_feed.id).await;

    let current = fixture.service.config_fingerprint().await.unwrap();
    fixture
        .service
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
    assert_eq!(last_opened(library).await, None);

    let remaining_entries = library
        .scalar(format!("SELECT COUNT(*) FROM entries WHERE feed_id = {}", dropped_feed.id))
        .await;
    assert_eq!(remaining_entries, 0);
    // Bodies leave with their entries (intentional difference 1).
    assert_eq!(library.scalar("SELECT COUNT(*) FROM entry_contents").await, 0);

    let deleted_flag =
        library.scalar(format!("SELECT is_deleted FROM feeds WHERE id = {}", dropped_feed.id));
    assert_eq!(deleted_flag.await, 1);
}

#[tokio::test]
async fn config_exchange_contract_import_cleans_removed_feed_entries_and_last_opened_state() {
    import_cleans_removed_feed_entries_and_last_opened_state().await;
}

#[tokio::test]
async fn config_exchange_contract_opml_import_upserts_normalized_feeds() {
    let fixture = build_fixture().await;

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

    let feeds = fixture.library.feeds.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 2);
    assert_eq!(feeds[0].url.as_str(), "https://example.com/feed.xml");
    assert_eq!(feeds[1].url.as_str(), "https://second.example.com/rss");
}

async fn remote_push_and_pull_roundtrip() {
    let export_fixture = build_fixture().await;
    export_fixture
        .library
        .feeds
        .upsert_subscription(&named("https://example.com/feed.xml", "Example Feed", Some("Inbox")))
        .await
        .expect("seed feed");
    let settings = sample_settings();
    export_fixture.library.settings.save(&settings).await.expect("save settings");

    let remote = Arc::new(MemoryRemoteConfigStore::default());
    export_fixture.service.push_remote_config(remote.as_ref()).await.expect("push remote config");
    let payload = remote.payload.lock().expect("lock payload").clone().expect("remote payload");
    assert!(payload.contains("\"feeds\""));
    assert!(payload.contains("Example Feed"));

    let import_fixture = build_fixture().await;
    let pulled = import_fixture
        .service
        .pull_remote_config(remote.as_ref())
        .await
        .expect("pull remote config");
    assert!(pulled.found());
    assert_eq!(pulled.import.as_ref().expect("import outcome").imported_feed_count, 1);

    let imported_feeds = import_fixture.library.feeds.list_feeds().await.expect("list feeds");
    assert_eq!(imported_feeds.len(), 1);
    assert_eq!(imported_feeds[0].url.as_str(), "https://example.com/feed.xml");
    assert_eq!(import_fixture.library.settings.load().await.expect("load settings"), settings);
}

#[tokio::test]
async fn config_exchange_contract_remote_push_and_pull_roundtrip() {
    remote_push_and_pull_roundtrip().await;
}

#[tokio::test]
async fn opml_import_rejects_truncation_without_persisting_prefix() {
    let fixture = build_fixture().await;
    fixture
        .service
        .import_opml(
            r#"<opml version="2.0"><body>
            <outline text="One" xmlUrl="https://example.com/one.xml" />"#,
        )
        .await
        .expect_err("truncated OPML must fail");

    assert!(fixture.library.feeds.list_feeds().await.unwrap().is_empty());
}

#[tokio::test]
async fn opml_import_rejects_unsupported_scheme_before_any_write() {
    let fixture = build_fixture().await;
    let error = fixture
        .service
        .import_opml(
            r#"<opml version="2.0"><body>
            <outline text="Good" xmlUrl="https://example.com/good.xml" />
            <outline text="Bad" xmlUrl="file:///tmp/private.xml" />
            </body></opml>"#,
        )
        .await
        .expect_err("file scheme must fail");

    assert!(format!("{error:#}").contains("OPML 中存在无效订阅 URL"));
    assert!(fixture.library.feeds.list_feeds().await.unwrap().is_empty());
}

#[tokio::test]
async fn opml_batch_database_failure_rolls_back_earlier_feed() {
    let fixture = build_fixture().await;
    fixture
        .library
        .exec(
            "CREATE TRIGGER reject_second_opml_feed
             BEFORE INSERT ON feeds
             WHEN NEW.url = 'https://example.com/two.xml'
             BEGIN
                 SELECT RAISE(ABORT, 'forced second feed failure');
             END",
        )
        .await;

    fixture
        .service
        .import_opml(
            r#"<opml version="2.0"><body>
            <outline text="One" xmlUrl="https://example.com/one.xml" />
            <outline text="Two" xmlUrl="https://example.com/two.xml" />
            </body></opml>"#,
        )
        .await
        .expect_err("second database write must abort the batch");

    assert!(fixture.library.feeds.list_feeds().await.unwrap().is_empty());
}

#[tokio::test]
async fn opml_import_rejects_material_before_root_without_persisting() {
    let fixture = build_fixture().await;
    fixture
        .service
        .import_opml(
            r#"leading-garbage<opml version="2.0"><body>
            <outline text="One" xmlUrl="https://example.com/one.xml" />
            </body></opml>"#,
        )
        .await
        .expect_err("material before root must fail");

    assert!(fixture.library.feeds.list_feeds().await.unwrap().is_empty());
}

#[tokio::test]
async fn opml_import_rejects_material_after_root_without_persisting() {
    let fixture = build_fixture().await;
    fixture
        .service
        .import_opml(
            r#"<opml version="2.0"><body>
            <outline text="One" xmlUrl="https://example.com/one.xml" />
            </body></opml>trailing-garbage"#,
        )
        .await
        .expect_err("material after root must fail");

    assert!(fixture.library.feeds.list_feeds().await.unwrap().is_empty());
}
