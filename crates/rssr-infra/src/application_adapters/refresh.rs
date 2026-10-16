use anyhow::Result;
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshFailure, RefreshHttpMetadata, RefreshTarget,
};

use crate::{
    fetch::{FetchClient, FetchRequest, FetchResult},
    parser::{FeedParser, ParsedEntry, ParsedFeed},
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
                        feed: map_parsed_feed(parsed),
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

fn map_parsed_feed(parsed: ParsedFeed) -> ParsedFeedUpdate {
    ParsedFeedUpdate {
        title: parsed.title,
        site_url: parsed.site_url,
        description: parsed.description,
        entries: parsed.entries.into_iter().map(map_parsed_entry).collect(),
    }
}

fn map_parsed_entry(entry: ParsedEntry) -> ParsedEntryData {
    ParsedEntryData {
        external_id: entry.external_id,
        dedup_key: entry.dedup_key,
        url: entry.url,
        title: entry.title,
        author: entry.author,
        summary: entry.summary,
        content_html: entry.content_html,
        content_text: entry.content_text,
        published_at: entry.published_at,
        updated_at_source: entry.updated_at_source,
    }
}
