use std::time::Duration;

use anyhow::Context;
use reqwest::header;

use super::feed_response::{
    FeedResponseStatus, classify_feed_response_status, http_metadata_from_headers,
};

/// 单个 feed 的抓取上限。刷新全部订阅默认是串行的，没有超时的话一个不响应的源就能把整轮
/// 刷新（以及后台自动刷新循环）永久挂住。
const FEED_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const FEED_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

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
        let inner = reqwest::Client::builder()
            .timeout(FEED_REQUEST_TIMEOUT)
            .connect_timeout(FEED_CONNECT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { inner }
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
        let body = response.text().await.context("读取 feed 响应正文失败")?;

        Ok(FetchResult::Fetched { body, metadata })
    }
}
