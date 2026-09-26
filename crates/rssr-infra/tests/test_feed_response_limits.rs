#![cfg(not(target_arch = "wasm32"))]

use std::{sync::Arc, time::Duration};

use rssr_application::RefreshService;
use rssr_domain::{EntryQuery, FeedRepository, NewFeedSubscription};
use rssr_infra::{
    application_adapters::{InfraFeedRefreshSource, SqliteRefreshStore},
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    fetch::{FetchClient, FetchRequest, FetchResult},
    subscription_probe::MAX_SUBSCRIPTION_BYTES,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

async fn server(responses: Vec<Vec<u8>>, keep_open: bool) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/feed.xml", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        for response in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let received = socket.read(&mut request).await.unwrap();
            assert!(received > 0, "expected a request before sending the fixture");
            // An early size rejection may close the connection before the server finishes.
            let _ = socket.write_all(&response).await;
            if keep_open {
                std::future::pending::<()>().await;
            }
        }
    });
    (url, task)
}

fn response(headers: &str, body: &[u8]) -> Vec<u8> {
    let mut result = format!("HTTP/1.1 200 OK\r\nConnection: close\r\n{headers}\r\n").into_bytes();
    result.extend_from_slice(body);
    result
}

async fn fetch(raw: Vec<u8>, keep_open: bool, deadline: Duration) -> anyhow::Result<FetchResult> {
    let (url, server) = server(vec![raw], keep_open).await;
    let result = tokio::time::timeout(
        deadline,
        FetchClient::new().fetch(&FetchRequest { url, etag: None, last_modified: None }),
    )
    .await;
    server.abort();
    result.expect("fetch must finish before the test deadline")
}

#[tokio::test]
async fn feed_accepts_exact_limit_with_or_without_content_length() {
    let body = vec![b'x'; MAX_SUBSCRIPTION_BYTES];
    for headers in [String::new(), format!("Content-Length: {}\r\n", body.len())] {
        let result = fetch(response(&headers, &body), false, Duration::from_secs(5)).await.unwrap();
        assert!(
            matches!(result, FetchResult::Fetched { body, .. } if body.len() == MAX_SUBSCRIPTION_BYTES)
        );
    }
}

#[tokio::test]
async fn feed_rejects_oversized_header_without_waiting_for_a_body() {
    let raw = response(&format!("Content-Length: {}\r\n", MAX_SUBSCRIPTION_BYTES + 1), &[]);
    let error = fetch(raw, true, Duration::from_secs(2)).await.unwrap_err();
    assert!(error.to_string().contains("8 MiB"), "{error:#}");
}

#[tokio::test]
async fn feed_rejects_chunked_body_over_the_limit() {
    let mut chunks = Vec::new();
    for size in [MAX_SUBSCRIPTION_BYTES, 1] {
        chunks.extend_from_slice(format!("{size:x}\r\n").as_bytes());
        chunks.extend(std::iter::repeat_n(b'x', size));
        chunks.extend_from_slice(b"\r\n");
    }
    // No terminal chunk: crossing the cap must fail before waiting for end of response.
    let error =
        fetch(response("Transfer-Encoding: chunked\r\n", &chunks), true, Duration::from_secs(5))
            .await
            .unwrap_err();
    assert!(error.to_string().contains("8 MiB"), "{error:#}");
}

#[tokio::test]
async fn feed_bounds_decompressed_bytes_not_the_compressed_content_length() {
    // Python: gzip.compress(b'x' * (8 * 1024 * 1024 + 1), mtime=0).
    let compressed = include_bytes!("fixtures/feed-response-oversized.gz");
    let headers = format!("Content-Encoding: gzip\r\nContent-Length: {}\r\n", compressed.len());
    let error =
        fetch(response(&headers, compressed), false, Duration::from_secs(5)).await.unwrap_err();
    assert!(error.to_string().contains("8 MiB"), "{error:#}");
}

#[tokio::test]
async fn feed_preserves_charset_and_bom_decoding_and_bounds_converted_text() {
    for (headers, bytes, expected) in [
        ("Content-Type: application/xml; charset=windows-1252\r\n", b"caf\xe9".as_slice(), "café"),
        ("Content-Type: application/xml\r\n", b"\xef\xbb\xbfcaf\xc3\xa9".as_slice(), "café"),
    ] {
        let result = fetch(response(headers, bytes), false, Duration::from_secs(5)).await.unwrap();
        assert!(matches!(result, FetchResult::Fetched { body, .. } if body == expected));
    }
    let error = fetch(
        response(
            "Content-Type: application/xml; charset=windows-1252\r\n",
            &vec![0xe9; MAX_SUBSCRIPTION_BYTES / 2 + 1],
        ),
        false,
        Duration::from_secs(5),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("8 MiB"), "{error:#}");
}

#[tokio::test]
async fn feed_body_stall_still_obeys_the_30_second_request_timeout() {
    let error = fetch(response("Content-Length: 100\r\n", b"x"), true, Duration::from_secs(35))
        .await
        .unwrap_err();
    assert!(
        error.chain().any(|error| error
            .downcast_ref::<reqwest::Error>()
            .is_some_and(reqwest::Error::is_timeout)),
        "{error:#}"
    );
}

#[tokio::test]
async fn oversized_refresh_keeps_old_articles_counts_and_success_history() {
    let xml = br#"<rss version="2.0"><channel><title>Test</title><description>Test</description><item><guid>entry</guid><title>Article</title><description>body</description></item></channel></rss>"#;
    let (url, server) = server(
        vec![
            response("Content-Type: application/rss+xml\r\n", xml),
            response(&format!("Content-Length: {}\r\n", MAX_SUBSCRIPTION_BYTES + 1), &[]),
        ],
        false,
    )
    .await;
    let pool = NativeSqliteBackend::new("sqlite::memory:").connect().await.unwrap();
    migrate(&pool).await.unwrap();
    let feeds = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entries = Arc::new(SqliteEntryRepository::new(pool));
    let feed = feeds
        .upsert_subscription(&NewFeedSubscription {
            url: url.parse().unwrap(),
            title: None,
            site_url: None,
            folder: None,
        })
        .await
        .unwrap();
    let service = RefreshService::new(
        Arc::new(InfraFeedRefreshSource::default()),
        Arc::new(SqliteRefreshStore::new(feeds.clone(), entries.clone())),
    );
    assert_eq!(service.refresh_feed(feed.id).await.unwrap().inserted_count(), 1);
    let before = entries.get_content(1).await.unwrap().unwrap();
    let success_at = feeds.get_feed(feed.id).await.unwrap().unwrap().last_success_at;
    let outcome = service.refresh_feed(feed.id).await.unwrap();
    assert_eq!(outcome.inserted_count(), 0);
    assert!(outcome.failure_message().unwrap().contains("8 MiB"));
    assert_eq!(entries.get_content(1).await.unwrap().unwrap(), before);
    assert_eq!(entries.list_entries(&EntryQuery::default()).await.unwrap().len(), 1);
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 1);
    assert_eq!(feeds.get_feed(feed.id).await.unwrap().unwrap().last_success_at, success_at);
    server.await.unwrap();
}
