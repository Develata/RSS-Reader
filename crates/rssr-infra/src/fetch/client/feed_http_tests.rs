//! Slow but progressing feeds, idle connections, and a client's whole-request deadline.
use std::time::{Duration, Instant};

use rssr_application::SubscriptionProbePort;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

use super::{FetchClient, FetchRequest, FetchResult};
use crate::{outbound::OutboundPolicy, subscription_probe::HttpSubscriptionProbe};

const IDLE: Duration = Duration::from_millis(300);
const TOTAL: Duration = Duration::from_secs(5);

async fn trickle_server(
    content_length: Option<usize>,
    chunks: Option<usize>,
    interval: Duration,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/feed.xml", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let _ = socket.read(&mut request).await.unwrap();
        let length = content_length
            .map(|length| format!("Content-Length: {length}\r\n"))
            .unwrap_or_default();
        socket
            .write_all(format!("HTTP/1.1 200 OK\r\nConnection: close\r\n{length}\r\n").as_bytes())
            .await
            .unwrap();
        let mut sent = 0;
        while chunks.is_none_or(|limit| sent < limit) {
            tokio::time::sleep(interval).await;
            if socket.write_all(b"x").await.is_err() {
                return;
            }
            sent += 1;
        }
        std::future::pending::<()>().await;
    });
    (url, task)
}

fn client(total: Duration) -> reqwest::Client {
    super::feed_client_builder(&OutboundPolicy::Unrestricted)
        .no_proxy()
        .connect_timeout(Duration::from_secs(1))
        .read_timeout(IDLE)
        .timeout(total)
        .build()
        .unwrap()
}

async fn fetch(url: String, total: Duration) -> anyhow::Result<FetchResult> {
    FetchClient { inner: client(total), policy: OutboundPolicy::Unrestricted }
        .fetch(&FetchRequest { url, etag: None, last_modified: None })
        .await
}

#[tokio::test]
async fn steady_body_may_outlast_the_idle_timeout() {
    let (url, server) = trickle_server(Some(8), Some(8), Duration::from_millis(100)).await;
    let started = Instant::now();
    let result = tokio::time::timeout(TOTAL * 2, fetch(url, TOTAL)).await;
    server.abort();
    assert!(started.elapsed() > IDLE);
    assert!(matches!(result, Ok(Ok(FetchResult::Fetched { ref body, .. })) if body == "xxxxxxxx"));
}

#[tokio::test]
async fn stalled_body_fails_on_the_idle_timeout_not_the_total() {
    let (url, server) = trickle_server(Some(8), Some(1), Duration::ZERO).await;
    let started = Instant::now();
    let result = tokio::time::timeout(TOTAL, fetch(url, TOTAL)).await;
    server.abort();
    assert!(result.expect("idle timeout must end the request").is_err());
    assert!(started.elapsed() < TOTAL / 2, "stall took {:?}", started.elapsed());
}

#[tokio::test]
async fn endless_trickle_is_cut_off_by_the_total_timeout() {
    let total = Duration::from_millis(800);
    let (url, server) = trickle_server(None, None, Duration::from_millis(100)).await;
    let started = Instant::now();
    let result = tokio::time::timeout(TOTAL, fetch(url, total)).await;
    server.abort();
    assert!(result.expect("fetch must preserve its client's total timeout").is_err());
    assert!(started.elapsed() >= total);
    assert!(started.elapsed() < total * 3, "took {:?}", started.elapsed());
}

#[tokio::test]
async fn subscription_probe_preserves_the_clients_total_timeout() {
    let total = Duration::from_millis(800);
    let (url, server) = trickle_server(None, None, Duration::from_millis(100)).await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        TOTAL,
        HttpSubscriptionProbe::new(client(total)).probe(&url.parse().unwrap()),
    )
    .await;
    server.abort();
    assert!(result.expect("probe must preserve its client's total timeout").is_err());
    assert!(started.elapsed() >= total);
    assert!(started.elapsed() < total * 3, "took {:?}", started.elapsed());
}
