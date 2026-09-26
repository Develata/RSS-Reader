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
        let feeds = self.feed_repository.list_feeds().await?;
        let feed_ids_with_entries = self.entry_repository.list_feed_ids_with_entries().await?;

        Ok(feeds
            .into_iter()
            .map(|feed| {
                let has_entries = feed_ids_with_entries.contains(&feed.id);
                map_refresh_target(feed, has_entries)
            })
            .collect())
    }

    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
        let Some(feed) = self.feed_repository.get_feed(feed_id).await? else {
            return Ok(None);
        };
        let has_entries = self.entry_repository.has_entries_for_feed(feed_id).await?;
        Ok(Some(map_refresh_target(feed, has_entries)))
    }

    async fn commit(
        &self,
        feed_id: i64,
        commit: RefreshCommit,
    ) -> Result<rssr_application::RefreshCommitOutcome> {
        let mut inserted_count = 0;
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
                let parsed_feed = map_application_feed_metadata(&update.feed);
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

                let entries = map_application_entries(update.feed.entries);
                let resolved_contents = match self
                    .entry_repository
                    .upsert_entries_with_outcome(feed_id, &entries)
                    .await
                {
                    Ok(resolved) => resolved,
                    Err(error) => {
                        let failure = RefreshFailure {
                            message: format!("写入文章索引失败: {error}"),
                            metadata: Some(update.metadata.clone()),
                        };
                        let _ = self.persist_failure(feed_id, &failure).await;
                        return Err(anyhow::Error::new(error).context("写入文章索引失败"));
                    }
                };

                inserted_count = resolved_contents.inserted_count;
                if let Err(error) = self
                    .entry_repository
                    .upsert_contents(feed_id, &resolved_contents.contents)
                    .await
                {
                    let failure = RefreshFailure {
                        message: format!("写入文章正文失败: {error}"),
                        metadata: Some(update.metadata.clone()),
                    };
                    let _ = self.persist_failure(feed_id, &failure).await;
                    return Err(anyhow::Error::new(error).context("写入文章正文失败"));
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

        Ok(rssr_application::RefreshCommitOutcome { inserted_count })
    }
}

fn map_refresh_target(feed: rssr_domain::Feed, has_entries: bool) -> RefreshTarget {
    // Failure metadata describes the received response, not a successfully persisted cache.
    // Sending its validators can yield 304 forever after a partial index/content write. Keep
    // the stored metadata and last-success history, but retry a full response until success.
    let (etag, last_modified) = if has_entries && feed.fetch_error.is_none() {
        (feed.etag, feed.last_modified)
    } else {
        tracing::debug!(
            feed_id = feed.id,
            url = %feed.url,
            "订阅本地无文章缓存或上次刷新失败，跳过条件请求并强制全量抓取"
        );
        (None, None)
    };

    RefreshTarget { feed_id: feed.id, url: feed.url, etag, last_modified }
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

/// `update_feed_metadata` 只读 title / site_url / description，因此不把条目一起克隆进来。
/// 之前这里会把整批条目深拷贝一次，紧接着 `map_application_entries` 又拷贝一次，等于每轮刷新
/// 都为一批用不到的条目付两次分配。
fn map_application_feed_metadata(feed: &ParsedFeedUpdate) -> ParsedFeed {
    ParsedFeed {
        title: feed.title.clone(),
        site_url: feed.site_url.clone(),
        description: feed.description.clone(),
        entries: Vec::new(),
    }
}

// `commit` owns the update: move the strings into the adapter representation instead of
// retaining a second copy of every body until the SQLite writes finish.
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

#[cfg(test)]
mod tests {
    use std::{hint::black_box, time::Instant};

    use super::{ParsedEntryData, map_application_entries};

    /// Run explicitly in release mode; fixture construction and result destruction are outside
    /// the measured adapter conversion. This measures mapping only, not network or SQLite I/O.
    #[test]
    #[ignore = "explicit refresh adapter microbenchmark"]
    fn refresh_entry_mapping_performance_probe() {
        for count in [800, 2_000] {
            let fixture = (0..count)
                .map(|id| ParsedEntryData {
                    external_id: format!("entry-{id}"),
                    dedup_key: format!("dedup-{id}"),
                    url: Some(
                        url::Url::parse(&format!("https://example.com/articles/{id}"))
                            .expect("fixture URL"),
                    ),
                    title: format!("中文文章 {id} with spaces"),
                    author: Some("Fixture author".to_string()),
                    summary: Some("Summary with UTF-8 内容".repeat(8)),
                    content_html: Some(format!("<p>{}</p>", "html内容 ".repeat(800))),
                    content_text: Some("正文 text ".repeat(400)),
                    published_at: Some(time::OffsetDateTime::UNIX_EPOCH),
                    updated_at_source: None,
                })
                .collect::<Vec<_>>();
            let mut elapsed = std::time::Duration::ZERO;
            const ITERATIONS: u32 = 40;
            for iteration in 0..ITERATIONS + 5 {
                let entries = fixture.clone();
                let start = Instant::now();
                let mapped = map_application_entries(black_box(entries));
                let duration = start.elapsed();
                black_box(&mapped);
                assert_eq!(mapped.len(), count);
                if iteration >= 5 {
                    elapsed += duration;
                }
            }
            println!(
                "REFRESH_MAPPING entries={count} iterations={ITERATIONS} mean_us={:.3}",
                elapsed.as_secs_f64() * 1_000_000.0 / f64::from(ITERATIONS)
            );
        }
    }
}
