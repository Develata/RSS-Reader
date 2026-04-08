use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use reqwest::{StatusCode, header};
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshCommit, RefreshFailure, RefreshHttpMetadata, RefreshService,
    RefreshStorePort, RefreshTarget,
};
use tokio::sync::oneshot;
use url::Url;
use wasm_bindgen_futures::spawn_local;

use super::{
    feed::{ParsedEntry, parse_feed, web_fetch_feed_response},
    state::{PersistedFeed, PersistedState, save_state_snapshot, upsert_entries},
    web_now_utc,
};

pub(super) fn build_refresh_service(
    state: Arc<Mutex<PersistedState>>,
    client: reqwest::Client,
) -> RefreshService {
    RefreshService::new(
        Arc::new(BrowserFeedRefreshSource::new(client)),
        Arc::new(BrowserRefreshStore::new(state, Arc::new(LocalStorageSnapshotWriter))),
    )
}

#[derive(Clone)]
struct BrowserFeedRefreshSource {
    client: reqwest::Client,
}

impl BrowserFeedRefreshSource {
    fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl FeedRefreshSourcePort for BrowserFeedRefreshSource {
    async fn refresh(&self, target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
        let (sender, receiver) = oneshot::channel();
        let client = self.client.clone();
        let url = target.url.to_string();
        spawn_local(async move {
            let _ = sender.send(fetch_and_parse_source_output(client, url).await);
        });

        match receiver.await {
            Ok(result) => result,
            Err(_) => Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                message: "刷新任务意外终止".to_string(),
                metadata: None,
            })),
        }
    }
}

async fn fetch_and_parse_source_output(
    client: reqwest::Client,
    raw_url: String,
) -> Result<FeedRefreshSourceOutput> {
    let response = match web_fetch_feed_response(&client, &raw_url).await {
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

    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(error) => {
            return Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                message: format!("feed 抓取返回非成功状态: {error}"),
                metadata: Some(metadata),
            }));
        }
    };

    let body = match response.text().await {
        Ok(body) => body,
        Err(error) => {
            return Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                message: format!("读取 feed 响应正文失败: {error}"),
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

trait SnapshotWriter: Send + Sync {
    fn write(&self, state: PersistedState) -> Result<()>;
}

struct LocalStorageSnapshotWriter;

impl SnapshotWriter for LocalStorageSnapshotWriter {
    fn write(&self, state: PersistedState) -> Result<()> {
        save_state_snapshot(state)
    }
}

#[derive(Clone)]
struct BrowserRefreshStore {
    state: Arc<Mutex<PersistedState>>,
    writer: Arc<dyn SnapshotWriter>,
}

impl BrowserRefreshStore {
    fn new(state: Arc<Mutex<PersistedState>>, writer: Arc<dyn SnapshotWriter>) -> Self {
        Self { state, writer }
    }

    fn map_feed_to_target(feed: &PersistedFeed) -> Result<RefreshTarget> {
        Ok(RefreshTarget {
            feed_id: feed.id,
            url: Url::parse(&feed.url).with_context(|| format!("订阅 URL 不合法：{}", feed.url))?,
            etag: feed.etag.clone(),
            last_modified: feed.last_modified.clone(),
        })
    }

    fn write_snapshot(&self, state: PersistedState) -> Result<()> {
        self.writer.write(state).context("写入浏览器本地状态失败")
    }
}

#[async_trait::async_trait]
impl RefreshStorePort for BrowserRefreshStore {
    async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
        let state = self.state.lock().expect("lock state");
        state.feeds.iter().filter(|feed| !feed.is_deleted).map(Self::map_feed_to_target).collect()
    }

    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
        let state = self.state.lock().expect("lock state");
        state
            .feeds
            .iter()
            .find(|feed| feed.id == feed_id && !feed.is_deleted)
            .map(Self::map_feed_to_target)
            .transpose()
    }

    async fn commit(&self, feed_id: i64, commit: RefreshCommit) -> Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let feed_index = state
                .feeds
                .iter()
                .position(|feed| feed.id == feed_id && !feed.is_deleted)
                .context("订阅不存在")?;
            let now = web_now_utc();

            match commit {
                RefreshCommit::NotModified { metadata } => {
                    let feed = &mut state.feeds[feed_index];
                    feed.etag = metadata.etag;
                    feed.last_modified = metadata.last_modified;
                    feed.last_fetched_at = Some(now);
                    feed.last_success_at = Some(now);
                    feed.fetch_error = None;
                    feed.updated_at = now;
                }
                RefreshCommit::Updated { update } => {
                    {
                        let feed = &mut state.feeds[feed_index];
                        if let Some(title) = update.feed.title.clone() {
                            feed.title = Some(title);
                        }
                        if let Some(site_url) = update.feed.site_url.clone() {
                            feed.site_url = Some(site_url.to_string());
                        }
                        if let Some(description) = update.feed.description.clone() {
                            feed.description = Some(description);
                        }
                        feed.etag = update.metadata.etag.clone();
                        feed.last_modified = update.metadata.last_modified.clone();
                        feed.last_fetched_at = Some(now);
                        feed.last_success_at = Some(now);
                        feed.fetch_error = None;
                        feed.updated_at = now;
                    }

                    upsert_entries(
                        &mut state,
                        feed_id,
                        map_application_entries(update.feed.entries),
                    )?;
                }
                RefreshCommit::Failed { failure } => {
                    let feed = &mut state.feeds[feed_index];
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

        self.write_snapshot(snapshot)
    }
}

fn map_parsed_feed(feed: super::feed::ParsedFeed) -> ParsedFeedUpdate {
    ParsedFeedUpdate {
        title: feed.title,
        site_url: feed.site_url,
        description: feed.description,
        entries: feed.entries.into_iter().map(map_parsed_entry).collect(),
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
