#![cfg(not(target_arch = "wasm32"))]

#[path = "support/request_capture.rs"]
mod request_capture;

use request_capture::{Response, Server};
use std::sync::{Arc, Mutex};

use rssr_application::import_export_service::{ImportExportService, RemoteConfigStore};
use rssr_domain::{
    FeedRepository, NewFeedSubscription, SettingsRepository, ThemeMode, UserSettings,
};
use rssr_infra::{
    application_adapters::InfraOpmlCodec,
    config_sync::webdav::WebDavConfigSync,
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        settings_repository::SqliteSettingsRepository, sqlite_native::NativeSqliteBackend,
        storage_backend::StorageBackend,
    },
    opml::OpmlCodec,
};

use url::Url;

struct WebDavRemote(WebDavConfigSync);

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigStore for WebDavRemote {
    async fn upload_config(&self, raw: &str) -> anyhow::Result<()> {
        self.0.upload_text(raw).await
    }

    async fn download_config(&self) -> anyhow::Result<Option<String>> {
        self.0.download_text().await
    }
}

#[tokio::test]
async fn local_webdav_roundtrip_restores_config_over_http_put_get() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite");
    migrate(&pool).await.expect("migrate sqlite");

    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
    let settings_repository = Arc::new(SqliteSettingsRepository::new(pool));
    let service = ImportExportService::new(
        feed_repository.clone(),
        entry_repository.clone(),
        entry_repository,
        settings_repository.clone(),
        Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
    );

    feed_repository
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/feed.xml").expect("valid url"),
            title: Some("Example Feed".to_string()),
            folder: Some("Inbox".to_string()),
        })
        .await
        .expect("insert source feed");

    settings_repository
        .save(&UserSettings { theme: ThemeMode::Dark, ..UserSettings::default() })
        .await
        .expect("save source settings");

    let stored_body = Arc::new(Mutex::new(None::<String>));
    let server_body = stored_body.clone();
    let server = Server::start(move |request| match request.method.as_str() {
        "PUT" => {
            *server_body.lock().unwrap() = Some(String::from_utf8(request.body.clone()).unwrap());
            Response::new(201, "", [])
        }
        "GET" => match server_body.lock().unwrap().as_ref() {
            Some(body) => Response::new(200, "Content-Type: application/json\r\n", body.as_bytes()),
            None => Response::new(404, "", []),
        },
        _ => Response::new(405, "", []),
    })
    .await;

    let remote = WebDavRemote(
        WebDavConfigSync::new(server.url.join("base").unwrap().as_str(), "config/rss-reader.json")
            .expect("create webdav sync"),
    );

    service.push_remote_config(&remote).await.expect("push config");

    feed_repository
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://stale.example.com/rss").expect("valid url"),
            title: Some("Stale".to_string()),
            folder: None,
        })
        .await
        .expect("insert stale feed");
    settings_repository
        .save(&UserSettings { theme: ThemeMode::Light, ..UserSettings::default() })
        .await
        .expect("overwrite settings");

    let restored = service.pull_remote_config(&remote).await.expect("pull config");
    assert!(restored.found());
    assert_eq!(restored.import.as_ref().expect("import outcome").imported_feed_count, 1);

    let feeds = feed_repository.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].url.as_str(), "https://example.com/feed.xml");
    assert_eq!(feeds[0].folder.as_deref(), Some("Inbox"));

    let settings = settings_repository.load().await.expect("load settings");
    assert_eq!(settings.theme, ThemeMode::Dark);

    let body = stored_body.lock().unwrap().clone().expect("uploaded body");
    assert!(body.contains("\"feeds\""));
    assert!(body.contains("\"settings\""));

    let paths: Vec<_> = server.requests().into_iter().map(|request| request.path).collect();
    assert_eq!(paths, vec!["/base/config/rss-reader.json", "/base/config/rss-reader.json"]);
}
