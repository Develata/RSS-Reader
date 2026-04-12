use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use reqwest::{StatusCode, header};
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshCommit, RefreshFailure, RefreshHttpMetadata, RefreshStorePort,
    RefreshTarget,
};

use crate::application_adapters::browser::{
    feed::{ParsedEntry, ParsedFeed, parse_feed, web_fetch_feed_response},
    now_utc,
    state::{BrowserState, save_state_snapshot, upsert_entries},
};

use super::shared::map_persistence_error;

#[derive(Clone)]
pub struct BrowserFeedRefreshSource {
    client: reqwest::Client,
}

impl BrowserFeedRefreshSource {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl FeedRefreshSourcePort for BrowserFeedRefreshSource {
    async fn refresh(&self, target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
        let response = match web_fetch_feed_response(&self.client, target.url.as_str()).await {
            Ok(response) => response,
            Err(error) => {
                return Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                    message: format!("抓取订阅失败: {error}"),
                    metadata: None,
                }));
            }
        };

        let metadata = RefreshHttpMetadata {
            etag: response
                .headers()
                .get(header::ETAG)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            last_modified: response
                .headers()
                .get(header::LAST_MODIFIED)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
        };

        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(FeedRefreshSourceOutput::NotModified(metadata));
        }

        let body = match response.error_for_status() {
            Ok(response) => response.text().await.context("读取 feed 响应正文失败")?,
            Err(error) => {
                return Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                    message: format!("feed 抓取返回非成功状态: {error}"),
                    metadata: Some(metadata),
                }));
            }
        };

        match parse_feed(&body) {
            Ok(parsed) => Ok(FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
                metadata,
                feed: map_parsed_feed(parsed),
            })),
            Err(error) => Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                message: format!("解析订阅失败: {error}"),
                metadata: Some(metadata),
            })),
        }
    }
}

#[derive(Clone)]
pub struct BrowserRefreshStore {
    state: Arc<Mutex<BrowserState>>,
}

impl BrowserRefreshStore {
    pub fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl RefreshStorePort for BrowserRefreshStore {
    async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
        let state = self.state.lock().expect("lock state");
        state
            .core
            .feeds
            .iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| {
                Ok(RefreshTarget {
                    feed_id: feed.id,
                    url: rssr_domain::normalize_feed_url(
                        &url::Url::parse(&feed.url).map_err(map_persistence_error)?,
                    ),
                    etag: feed.etag.clone(),
                    last_modified: feed.last_modified.clone(),
                })
            })
            .collect()
    }

    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
        let state = self.state.lock().expect("lock state");
        state
            .core
            .feeds
            .iter()
            .find(|feed| feed.id == feed_id && !feed.is_deleted)
            .map(|feed| {
                Ok(RefreshTarget {
                    feed_id: feed.id,
                    url: rssr_domain::normalize_feed_url(
                        &url::Url::parse(&feed.url).map_err(map_persistence_error)?,
                    ),
                    etag: feed.etag.clone(),
                    last_modified: feed.last_modified.clone(),
                })
            })
            .transpose()
    }

    async fn commit(&self, feed_id: i64, commit: RefreshCommit) -> Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let now = now_utc();
            let feed = state
                .core
                .feeds
                .iter_mut()
                .find(|feed| feed.id == feed_id)
                .context("订阅不存在")?;

            match commit {
                RefreshCommit::NotModified { metadata } => {
                    feed.etag = metadata.etag;
                    feed.last_modified = metadata.last_modified;
                    feed.last_fetched_at = Some(now);
                    feed.last_success_at = Some(now);
                    feed.fetch_error = None;
                    feed.updated_at = now;
                }
                RefreshCommit::Updated { update } => {
                    if update.feed.title.is_some() {
                        feed.title = update.feed.title;
                    }
                    if update.feed.site_url.is_some() {
                        feed.site_url = update.feed.site_url.map(|url| url.to_string());
                    }
                    if update.feed.description.is_some() {
                        feed.description = update.feed.description;
                    }
                    feed.etag = update.metadata.etag;
                    feed.last_modified = update.metadata.last_modified;
                    feed.last_fetched_at = Some(now);
                    feed.last_success_at = Some(now);
                    feed.fetch_error = None;
                    feed.updated_at = now;
                    upsert_entries(
                        &mut state.core,
                        feed_id,
                        map_application_entries(update.feed.entries),
                    )?;
                }
                RefreshCommit::Failed { failure } => {
                    if let Some(metadata) = failure.metadata {
                        feed.etag = metadata.etag;
                        feed.last_modified = metadata.last_modified;
                    }
                    feed.last_fetched_at = Some(now);
                    feed.fetch_error = Some(failure.message);
                    feed.updated_at = now;
                }
            }

            state.clone()
        };

        save_state_snapshot(snapshot)
    }
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

fn map_application_entries(entries: Vec<ParsedEntryData>) -> Vec<ParsedEntry> {
    entries
        .into_iter()
        .map(|entry| ParsedEntry {
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
        })
        .collect()
}
