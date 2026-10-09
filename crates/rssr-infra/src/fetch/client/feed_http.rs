use std::time::Duration;

use anyhow::Context;
use reqwest::header;

use crate::feed_body::read_feed_text;

use super::feed_response::{
    FeedResponseStatus, classify_feed_response_status, http_metadata_from_headers,
};

/// 单个 feed 的抓取上限，包含响应体读取；一个不响应的源不能让整轮刷新永久等待。
const FEED_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const FEED_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// 订阅抓取、订阅探测与 WebDAV 请求共用的 User-Agent（RFC 9110 §10.1.5：客户端 SHOULD 发送）。
/// 版本取发布构建注入的 `RELEASE_TAG`（与 CLI 一致），本地构建回落到 crate 版本。
pub(crate) fn user_agent() -> String {
    let version = option_env!("RELEASE_TAG").unwrap_or(env!("CARGO_PKG_VERSION")).trim();
    let version = version.strip_prefix('v').unwrap_or(version);
    format!("RSS-Reader/{version} (+https://github.com/Develata/RSS-Reader)")
}

/// 订阅抓取与订阅探测共用的客户端配置：同一个 UA、连接与总时限。
pub(crate) fn feed_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .user_agent(user_agent())
        .timeout(FEED_REQUEST_TIMEOUT)
        .connect_timeout(FEED_CONNECT_TIMEOUT)
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
        let inner = feed_client_builder().build().unwrap_or_else(|_| reqwest::Client::new());
        Self { inner }
    }

    pub async fn fetch(&self, request: &FetchRequest) -> anyhow::Result<FetchResult> {
        let mut builder = self.inner.get(&request.url).header(
            header::ACCEPT,
            "application/atom+xml, application/rss+xml, application/xml, text/xml;q=0.9, */*;q=0.1",
        ).timeout(FEED_REQUEST_TIMEOUT);

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
