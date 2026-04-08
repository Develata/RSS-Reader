use std::sync::Arc;

use anyhow::{Context, Result};
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
    ParsedFeedUpdate, RefreshCommit, RefreshFailure, RefreshHttpMetadata, RefreshStorePort,
    RefreshTarget,
};
use rssr_domain::FeedRepository;

use crate::{
    db::{entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository},
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
                return Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                    message: format!("抓取订阅失败: {error}"),
                    metadata: None,
                }));
            }
        };

        match response {
            FetchResult::NotModified(metadata) => {
                Ok(FeedRefreshSourceOutput::NotModified(map_http_metadata(metadata)))
            }
            FetchResult::Fetched { body, metadata } => match self.parser.parse(&body) {
                Ok(parsed) => Ok(FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
                    metadata: map_http_metadata(metadata),
                    feed: map_parsed_feed(parsed),
                })),
                Err(error) => Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                    message: format!("解析订阅失败: {error}"),
                    metadata: Some(map_http_metadata(metadata)),
                })),
            },
        }
    }
}

#[derive(Clone)]
pub struct SqliteRefreshStore {
    feed_repository: Arc<SqliteFeedRepository>,
    entry_repository: Arc<SqliteEntryRepository>,
}

impl SqliteRefreshStore {
    pub fn new(
        feed_repository: Arc<SqliteFeedRepository>,
        entry_repository: Arc<SqliteEntryRepository>,
    ) -> Self {
        Self { feed_repository, entry_repository }
    }

    async fn persist_failure(
        &self,
        feed_id: i64,
        failure: &RefreshFailure,
    ) -> rssr_domain::Result<()> {
        self.feed_repository
            .update_fetch_state(
                feed_id,
                failure.metadata.as_ref().and_then(|metadata| metadata.etag.as_deref()),
                failure.metadata.as_ref().and_then(|metadata| metadata.last_modified.as_deref()),
                Some(&failure.message),
                false,
            )
            .await
    }
}

#[async_trait::async_trait]
impl RefreshStorePort for SqliteRefreshStore {
    async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
        Ok(self
            .feed_repository
            .list_feeds()
            .await?
            .into_iter()
            .map(|feed| RefreshTarget {
                feed_id: feed.id,
                url: feed.url,
                etag: feed.etag,
                last_modified: feed.last_modified,
            })
            .collect())
    }

    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
        Ok(self.feed_repository.get_feed(feed_id).await?.map(|feed| RefreshTarget {
            feed_id: feed.id,
            url: feed.url,
            etag: feed.etag,
            last_modified: feed.last_modified,
        }))
    }

    async fn commit(&self, feed_id: i64, commit: RefreshCommit) -> Result<()> {
        match commit {
            RefreshCommit::NotModified { metadata } => {
                self.feed_repository
                    .update_fetch_state(
                        feed_id,
                        metadata.etag.as_deref(),
                        metadata.last_modified.as_deref(),
                        None,
                        true,
                    )
                    .await
                    .context("更新订阅抓取状态失败")?;
            }
            RefreshCommit::Updated { update } => {
                let parsed_feed = map_application_feed(&update.feed);
                if let Err(error) =
                    self.feed_repository.update_feed_metadata(feed_id, &parsed_feed).await
                {
                    let failure = RefreshFailure {
                        message: format!("更新订阅元数据失败: {error}"),
                        metadata: Some(update.metadata.clone()),
                    };
                    let _ = self.persist_failure(feed_id, &failure).await;
                    return Err(anyhow::Error::new(error).context("更新订阅元数据失败"));
                }

                let entries = map_application_entries(&update.feed.entries);
                if let Err(error) = self.entry_repository.upsert_entries(feed_id, &entries).await {
                    let failure = RefreshFailure {
                        message: format!("写入文章失败: {error}"),
                        metadata: Some(update.metadata.clone()),
                    };
                    let _ = self.persist_failure(feed_id, &failure).await;
                    return Err(anyhow::Error::new(error).context("写入文章失败"));
                }

                self.feed_repository
                    .update_fetch_state(
                        feed_id,
                        update.metadata.etag.as_deref(),
                        update.metadata.last_modified.as_deref(),
                        None,
                        true,
                    )
                    .await
                    .context("更新订阅抓取状态失败")?;
            }
            RefreshCommit::Failed { failure } => {
                self.persist_failure(feed_id, &failure).await.context("更新订阅抓取状态失败")?;
            }
        }

        Ok(())
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

fn map_application_feed(feed: &ParsedFeedUpdate) -> ParsedFeed {
    ParsedFeed {
        title: feed.title.clone(),
        site_url: feed.site_url.clone(),
        description: feed.description.clone(),
        entries: map_application_entries(&feed.entries),
    }
}

fn map_application_entries(entries: &[ParsedEntryData]) -> Vec<ParsedEntry> {
    entries
        .iter()
        .map(|entry| ParsedEntry {
            external_id: entry.external_id.clone(),
            dedup_key: entry.dedup_key.clone(),
            url: entry.url.clone(),
            title: entry.title.clone(),
            author: entry.author.clone(),
            summary: entry.summary.clone(),
            content_html: entry.content_html.clone(),
            content_text: entry.content_text.clone(),
            published_at: entry.published_at,
            updated_at_source: entry.updated_at_source,
        })
        .collect()
}
