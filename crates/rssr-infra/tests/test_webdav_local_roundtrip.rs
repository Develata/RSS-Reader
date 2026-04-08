use std::sync::Arc;

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
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex, oneshot},
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
        entry_repository,
        settings_repository.clone(),
        Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
    );

    feed_repository
        .upsert_subscription(&NewFeedSubscription {
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
    let request_paths = Arc::new(Mutex::new(Vec::<String>::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind local server");
    let addr = listener.local_addr().expect("listener addr");
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    let server_body = stored_body.clone();
    let server_paths = request_paths.clone();
    let server = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accept = listener.accept() => {
                    let Ok((mut stream, _)) = accept else { break };
                    let mut raw = Vec::new();
                    let mut buf = [0_u8; 4096];
                    let header_end;
                    let content_length;

                    loop {
                        let read = stream.read(&mut buf).await.expect("read request chunk");
                        if read == 0 {
                            break;
                        }
                        raw.extend_from_slice(&buf[..read]);
                        if let Some(idx) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                            header_end = idx + 4;
                            let head = String::from_utf8_lossy(&raw[..idx]).to_string();
                            content_length = head
                                .lines()
                                .find_map(|line| {
                                    let (name, value) = line.split_once(':')?;
                                    if name.eq_ignore_ascii_case("content-length") {
                                        value.trim().parse::<usize>().ok()
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0);
                            while raw.len() < header_end + content_length {
                                let read = stream.read(&mut buf).await.expect("read request body");
                                if read == 0 {
                                    break;
                                }
                                raw.extend_from_slice(&buf[..read]);
                            }
                            break;
                        }
                    }

                    let request = String::from_utf8_lossy(&raw).to_string();
                    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
                    let mut lines = head.lines();
                    let request_line = lines.next().unwrap_or_default();
                    let mut parts = request_line.split_whitespace();
                    let method = parts.next().unwrap_or_default();
                    let path = parts.next().unwrap_or_default().to_string();
                    server_paths.lock().await.push(path.clone());

                    match method {
                        "PUT" => {
                            *server_body.lock().await = Some(body.to_string());
                            stream
                                .write_all(
                                    b"HTTP/1.1 201 Created\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                                )
                                .await
                                .expect("write put response");
                            let _ = stream.shutdown().await;
                        }
                        "GET" => {
                            if let Some(payload) = server_body.lock().await.clone() {
                                let response = format!(
                                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                                    payload.len(),
                                    payload
                                );
                                stream.write_all(response.as_bytes()).await.expect("write get response");
                                let _ = stream.shutdown().await;
                            } else {
                                stream
                                    .write_all(
                                        b"HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                                    )
                                    .await
                                    .expect("write 404 response");
                                let _ = stream.shutdown().await;
                            }
                        }
                        _ => {
                            stream
                                .write_all(
                                    b"HTTP/1.1 405 Method Not Allowed\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                                )
                                .await
                                .expect("write 405 response");
                            let _ = stream.shutdown().await;
                        }
                    }
                }
            }
        }
    });

    let remote = WebDavRemote(
        WebDavConfigSync::new(format!("http://{}/base", addr), "config/rss-reader.json")
            .expect("create webdav sync"),
    );

    service.push_remote_config(&remote).await.expect("push config");

    feed_repository
        .upsert_subscription(&NewFeedSubscription {
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
    assert!(restored);

    let feeds = feed_repository.list_feeds().await.expect("list feeds");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].url.as_str(), "https://example.com/feed.xml");
    assert_eq!(feeds[0].folder.as_deref(), Some("Inbox"));

    let settings = settings_repository.load().await.expect("load settings");
    assert_eq!(settings.theme, ThemeMode::Dark);

    let body = stored_body.lock().await.clone().expect("uploaded body");
    assert!(body.contains("\"feeds\""));
    assert!(body.contains("\"settings\""));

    let paths = request_paths.lock().await.clone();
    assert_eq!(paths, vec!["/base/config/rss-reader.json", "/base/config/rss-reader.json"]);

    let _ = shutdown_tx.send(());
    server.await.expect("join server");
}
