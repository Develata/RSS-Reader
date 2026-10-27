#![cfg(not(target_arch = "wasm32"))]

//! The storage side of the feed response limits, run against the v0.2 library store (ported
//! from `test_feed_response_limits.rs`). The other tests of that file exercise only the HTTP
//! transport (size caps, decompression, charset, stall timeout) and never construct a
//! repository: "not storage, unchanged".

#[path = "support/library.rs"]
mod library;

use std::sync::Arc;

use library::Library;
use rssr_application::RefreshService;
use rssr_domain::{EntryContentRepository, EntryIndexRepository, EntryQuery, FeedRepository};
use rssr_infra::{
    application_adapters::InfraFeedRefreshSource, subscription_probe::MAX_SUBSCRIPTION_BYTES,
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
    let library = Library::memory().await;
    let feed = library.add_feed(&url).await;
    let service = RefreshService::new(
        Arc::new(InfraFeedRefreshSource::default()),
        Arc::new(library.refresh.clone()),
    );
    assert_eq!(service.refresh_feed(feed.id).await.unwrap().inserted_count(), 1);
    let before = library.entries.get_content(1).await.unwrap().unwrap();
    let success_at = library.feeds.get_feed(feed.id).await.unwrap().unwrap().last_success_at;
    let outcome = service.refresh_feed(feed.id).await.unwrap();
    assert_eq!(outcome.inserted_count(), 0);
    assert!(outcome.failure_message().unwrap().contains("8 MiB"));
    assert_eq!(library.entries.get_content(1).await.unwrap().unwrap(), before);
    assert_eq!(library.entries.list_entries(&EntryQuery::default()).await.unwrap().len(), 1);
    assert_eq!(library.feeds.list_summaries().await.unwrap()[0].unread_count, 1);
    assert_eq!(library.feeds.get_feed(feed.id).await.unwrap().unwrap().last_success_at, success_at);
    server.await.unwrap();
}
