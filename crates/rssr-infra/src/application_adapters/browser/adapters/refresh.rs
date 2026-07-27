use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

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

        if let Some(output) = classify_browser_refresh_status(response.status(), metadata.clone()) {
            return Ok(output);
        }

        let body = response.text().await.context("读取 feed 响应正文失败")?;

        Ok(classify_browser_refresh_body(metadata, &body))
    }
}

pub fn classify_browser_refresh_status(
    status: StatusCode,
    metadata: RefreshHttpMetadata,
) -> Option<FeedRefreshSourceOutput> {
    if status == StatusCode::NOT_MODIFIED {
        Some(FeedRefreshSourceOutput::NotModified(metadata))
    } else if !status.is_success() {
        Some(FeedRefreshSourceOutput::Failed(RefreshFailure {
            message: format!("feed 抓取返回非成功状态: HTTP status {status}"),
            metadata: Some(metadata),
        }))
    } else {
        None
    }
}

pub fn classify_browser_refresh_body(
    metadata: RefreshHttpMetadata,
    body: &str,
) -> FeedRefreshSourceOutput {
    match parse_feed(body) {
        Ok(parsed) => FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
            metadata,
            feed: map_parsed_feed(parsed),
        }),
        Err(error) => FeedRefreshSourceOutput::Failed(RefreshFailure {
            message: format!("解析订阅失败: {error}"),
            metadata: Some(metadata),
        }),
    }
}

/// 批次状态放在 `Arc` 里而不是直接内嵌：本类型是 `Clone` 的，而克隆出来的副本
/// 必须与原件共享同一个批次——否则一份克隆开着批次、另一份看到 `active == false`，
/// 就会出现「以为推迟了其实立刻写」或反过来的错配。
#[derive(Clone)]
pub struct BrowserRefreshStore {
    state: Arc<Mutex<BrowserState>>,
    batch: Arc<RefreshWriteBatch>,
}

/// 一轮刷新的写入批次状态。
///
/// `localStorage` 只能整片覆盖：`save_state_snapshot` 每次都要把全部订阅、全部条目索引、
/// 全部标记与**全部正文**重新序列化一遍写回去。逐个订阅提交时这份开销要乘以订阅数，
/// 而且发生在主线程上——订阅一多，刷新期间整个页面就是卡住的。批次把它压回整轮一次。
///
/// 只用 `AtomicBool` 而不是再加一把锁：这个适配器只在 wasm 上编译，浏览器里是单线程执行，
/// 这里不存在真正的竞争，用原子量只是为了能在 `&self` 上改。
#[derive(Default)]
struct RefreshWriteBatch {
    /// 批次进行中：`commit` 只改内存，不落盘。
    active: AtomicBool,
    /// 批次内至少发生过一次 `commit`，`end_batch` 才需要真的写一次。
    dirty: AtomicBool,
}

impl BrowserRefreshStore {
    pub fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state, batch: Arc::new(RefreshWriteBatch::default()) }
    }

    /// 批次内累积过改动才落盘。
    ///
    /// 没有改动就不写：整轮所有订阅都返回 304 是常态，那种情况下不该白白整片重写一次全库。
    ///
    /// 写失败时把脏标记放回去。清标记发生在写之前，若不还原，这批「还在内存里、尚未落盘」的
    /// 改动就再也没有机会被重试了——下一次冲刷会以为无事可做。
    fn flush_if_dirty(&self) -> Result<()> {
        if !self.batch.dirty.swap(false, Ordering::SeqCst) {
            return Ok(());
        }

        let state = self.state.lock().expect("lock state");
        save_state_snapshot(&state).inspect_err(|_| {
            self.batch.dirty.store(true, Ordering::SeqCst);
        })
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
        {
            let mut state = self.state.lock().expect("lock state");
            let now = now_utc();
            let feed = state
                .core
                .feeds
                .iter_mut()
                .find(|feed| feed.id == feed_id)
                .context("订阅不存在")?;

            // 在动内存之前就标脏。`RefreshCommit::Updated` 会先改完 feed 元数据再走
            // `upsert_entries`，后者失败时带 `?` 返回——那时元数据已经改了。
            // 把标脏放在末尾的话这份改动就不会被计入批次，`end_batch` 什么都不写，
            // 内存与存储从此长期不一致。
            let batching = self.batch.active.load(Ordering::SeqCst);
            if batching {
                self.batch.dirty.store(true, Ordering::SeqCst);
            }

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
                        &mut state,
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

            // 批次进行中就此返回：改动已经落在共享的内存状态里，页面那一侧照样读得到，
            // 真正的整片写盘推迟到 `end_batch` 一次做完。
            if batching {
                return Ok(());
            }

            save_state_snapshot(&state)
        }
    }

    async fn begin_batch(&self) -> Result<()> {
        // 先冲掉上一个没关掉的批次，再无条件开张。
        //
        // 冲刷失败也要开张：若在这里带着 `?` 返回而把 `active` 留在上一轮的 `true`，
        // 存储就会停在「只改内存不落盘」的状态，而调用方拿到的只是一个开批次失败的错误，
        // 根本看不出后续提交都被吞了。错误照样上报，但状态必须是确定的。
        let flushed = self.flush_if_dirty();
        self.batch.active.store(true, Ordering::SeqCst);
        flushed
    }

    async fn end_batch(&self) -> Result<()> {
        self.batch.active.store(false, Ordering::SeqCst);
        self.flush_if_dirty()
    }

    /// 幂等：`end_batch` 正常收尾后批次守卫析构还会再调一次，那时两个标志都已清零，
    /// `flush_if_dirty` 直接返回，不会产生第二次写入。
    ///
    /// 这条路径没有地方可以上报错误（它从 `Drop` 里被调用），因此落盘失败只能记日志。
    /// 脏标记会被 `flush_if_dirty` 还原，下一轮刷新开张时还会再试一次。
    fn abort_batch(&self) {
        self.batch.active.store(false, Ordering::SeqCst);
        if let Err(error) = self.flush_if_dirty() {
            tracing::warn!(%error, "刷新批次被中断且落盘失败，本轮抓取的改动仍留在内存里");
        }
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
