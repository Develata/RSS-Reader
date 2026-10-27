use anyhow::Result;
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, RefreshFailure,
    RefreshHttpMetadata, RefreshTarget,
};

use crate::{
    fetch::{FetchClient, FetchRequest, FetchResult},
    parser::FeedParser,
};

#[derive(Clone, Default)]
pub struct InfraFeedRefreshSource {
    fetch_client: FetchClient,
    parser: FeedParser,
}

impl InfraFeedRefreshSource {
    pub fn new(fetch_client: FetchClient, parser: FeedParser) -> Self {
        Self { fetch_client, parser }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl FeedRefreshSourcePort for InfraFeedRefreshSource {
    async fn refresh(&self, target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
        let response = match self
            .fetch_client
            .fetch(&FetchRequest {
                url: target.url.to_string(),
                etag: target.etag.clone(),
                last_modified: target.last_modified.clone(),
            })
            .await
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(FeedRefreshSourceOutput::Failed(RefreshFailure::fetch(
                    format!("抓取订阅失败: {error}"),
                    None,
                )));
            }
        };

        match response {
            FetchResult::NotModified(metadata) => {
                Ok(FeedRefreshSourceOutput::NotModified(map_http_metadata(metadata)))
            }
            FetchResult::Fetched { body, metadata } => {
                let _slot = crate::feed_normalization::PARSE_SLOTS.acquire().await;
                match self.parser.parse(&body) {
                    Ok(parsed) => Ok(FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
                        metadata: map_http_metadata(metadata),
                        feed: parsed.into(),
                    })),
                    Err(error) => Ok(FeedRefreshSourceOutput::Failed(RefreshFailure::parse(
                        format!("解析订阅失败: {error}"),
                        Some(map_http_metadata(metadata)),
                    ))),
                }
            }
        }
    }
}

fn map_http_metadata(metadata: crate::fetch::HttpMetadata) -> RefreshHttpMetadata {
    RefreshHttpMetadata { etag: metadata.etag, last_modified: metadata.last_modified }
}
