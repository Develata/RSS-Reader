use std::time::Duration;

use anyhow::Context;
use reqwest::header;

use crate::feed_body::read_feed_text;

use super::feed_response::{
    FeedResponseStatus, classify_feed_response_status, http_metadata_from_headers,
};

/// feed 抓取的超时分三层。停住的连接由空闲读超时尽快判死；仍在持续传输的大 feed 只受
/// 整体上限约束，所以慢链路上的数 MB 全文 feed 不会因为固定的短总时长失败，而一个
/// 涓流般吐字节的源也不能让整轮刷新永久等待。
#[derive(Debug, Clone, Copy)]
pub(crate) struct FeedTimeouts {
    pub(crate) connect: Duration,
    /// 两次成功读之间（含等待响应头）允许的最长间隔。
    pub(crate) read_idle: Duration,
    /// 从开始连接到响应体读完的总时长。
    pub(crate) total: Duration,
}

pub(crate) const FEED_TIMEOUTS: FeedTimeouts = FeedTimeouts {
    connect: Duration::from_secs(10),
    read_idle: Duration::from_secs(30),
    total: Duration::from_secs(120),
};

/// 订阅抓取与订阅探测共用的原生 HTTP client。
pub(crate) fn feed_http_client(timeouts: FeedTimeouts) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(timeouts.connect)
        .read_timeout(timeouts.read_idle)
        .timeout(timeouts.total)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

#[derive(Debug, Clone)]
pub struct FetchClient {
    inner: reqwest::Client,
}

impl Default for FetchClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct FetchRequest {
    pub url: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpMetadata {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchResult {
    NotModified(HttpMetadata),
    Fetched { body: String, metadata: HttpMetadata },
}

impl FetchClient {
    pub fn new() -> Self {
        Self::with_timeouts(FEED_TIMEOUTS)
    }

    pub(crate) fn with_timeouts(timeouts: FeedTimeouts) -> Self {
        Self { inner: feed_http_client(timeouts) }
    }

    pub async fn fetch(&self, request: &FetchRequest) -> anyhow::Result<FetchResult> {
        let mut builder = self.inner.get(&request.url).header(
            header::ACCEPT,
            "application/atom+xml, application/rss+xml, application/xml, text/xml;q=0.9, */*;q=0.1",
        );

        if let Some(etag) = &request.etag {
            builder = builder.header(header::IF_NONE_MATCH, etag);
        }
        if let Some(last_modified) = &request.last_modified {
            builder = builder.header(header::IF_MODIFIED_SINCE, last_modified);
        }

        let response = builder.send().await.context("发送 feed 抓取请求失败")?;
        let metadata = http_metadata_from_headers(response.headers());
        if classify_feed_response_status(response.status()) == FeedResponseStatus::NotModified {
            return Ok(FetchResult::NotModified(metadata));
        }

        let response = response.error_for_status().context("feed 抓取返回非成功状态")?;
        let body = read_feed_text(response).await?;

        Ok(FetchResult::Fetched { body, metadata })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    use super::{FeedTimeouts, FetchClient, FetchRequest, FetchResult};

    const TIMEOUTS: FeedTimeouts = FeedTimeouts {
        connect: Duration::from_secs(1),
        read_idle: Duration::from_millis(300),
        total: Duration::from_secs(5),
    };

    /// 先回响应头，再每隔 `interval` 写一个 `chunk`；`chunks` 为 `None` 时一直写下去，
    /// 写完后保持连接不关，让停滞只能由客户端的超时结束。
    async fn trickle_server(
        content_length: Option<usize>,
        chunk: &'static [u8],
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
            let head = format!("HTTP/1.1 200 OK\r\nConnection: close\r\n{length}\r\n");
            socket.write_all(head.as_bytes()).await.unwrap();
            let mut sent = 0;
            while chunks.is_none_or(|limit| sent < limit) {
                tokio::time::sleep(interval).await;
                if socket.write_all(chunk).await.is_err() {
                    return;
                }
                sent += 1;
            }
            std::future::pending::<()>().await;
        });
        (url, task)
    }

    async fn fetch(url: String) -> anyhow::Result<FetchResult> {
        FetchClient::with_timeouts(TIMEOUTS)
            .fetch(&FetchRequest { url, etag: None, last_modified: None })
            .await
    }

    #[tokio::test]
    async fn steady_body_may_outlast_the_idle_timeout() {
        // 8 × 100ms = 800ms，超过 300ms 的空闲读超时，但每次读之间都不空闲。
        let (url, server) =
            trickle_server(Some(8), b"x", Some(8), Duration::from_millis(100)).await;
        let started = Instant::now();
        let result = fetch(url).await;
        server.abort();

        assert!(started.elapsed() > TIMEOUTS.read_idle);
        assert!(matches!(result, Ok(FetchResult::Fetched { ref body, .. }) if body == "xxxxxxxx"));
    }

    #[tokio::test]
    async fn stalled_body_fails_on_the_idle_timeout_not_the_total() {
        let (url, server) = trickle_server(Some(8), b"x", Some(1), Duration::ZERO).await;
        let started = Instant::now();
        let result = fetch(url).await;
        server.abort();

        assert!(result.is_err());
        assert!(started.elapsed() < TIMEOUTS.total / 2, "stall took {:?}", started.elapsed());
    }

    #[tokio::test]
    async fn endless_trickle_is_cut_off_by_the_total_timeout() {
        let timeouts = FeedTimeouts { total: Duration::from_millis(800), ..TIMEOUTS };
        let (url, server) = trickle_server(None, b"x", None, Duration::from_millis(100)).await;
        let started = Instant::now();
        let result = FetchClient::with_timeouts(timeouts)
            .fetch(&FetchRequest { url, etag: None, last_modified: None })
            .await;
        server.abort();

        assert!(result.is_err());
        assert!(started.elapsed() >= timeouts.total);
        assert!(started.elapsed() < timeouts.total * 3, "took {:?}", started.elapsed());
    }
}
