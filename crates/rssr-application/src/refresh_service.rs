use std::sync::Arc;

use anyhow::{Context, Result};
use time::OffsetDateTime;
use url::Url;

#[cfg(not(target_arch = "wasm32"))]
use tokio::task::JoinSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshTarget {
    pub feed_id: i64,
    pub url: Url,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshHttpMetadata {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFeedUpdate {
    pub title: Option<String>,
    pub site_url: Option<Url>,
    pub description: Option<String>,
    pub entries: Vec<ParsedEntryData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEntryData {
    pub external_id: String,
    pub dedup_key: String,
    pub url: Option<Url>,
    pub title: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub published_at: Option<OffsetDateTime>,
    pub updated_at_source: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedRefreshUpdate {
    pub metadata: RefreshHttpMetadata,
    pub feed: ParsedFeedUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshFailure {
    pub message: String,
    pub metadata: Option<RefreshHttpMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedRefreshSourceOutput {
    NotModified(RefreshHttpMetadata),
    Updated(FeedRefreshUpdate),
    Failed(RefreshFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshCommit {
    NotModified { metadata: RefreshHttpMetadata },
    Updated { update: FeedRefreshUpdate },
    Failed { failure: RefreshFailure },
}

/// 「刷新全部」的默认并发度。
///
/// 这是一个跨 crate 的不变量的锚点：连接池大小必须大于它（见
/// `rssr_infra::db::default_sqlite_max_connections`），否则刷新任务会占满池、把界面查询挤掉。
/// 桌面端与 CLI 都引用这个常量，不要各自写字面量。
pub const DEFAULT_REFRESH_CONCURRENCY: usize = 4;

/// 无法确认数据库处于 WAL 模式时退回的并发度。
///
/// 回滚日志模式下写者会阻塞读者，并发写只能靠 busy_timeout 干等，因此退回串行。
pub const SERIAL_REFRESH_CONCURRENCY: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshAllInput {
    pub max_concurrency: usize,
}

impl Default for RefreshAllInput {
    fn default() -> Self {
        Self { max_concurrency: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshFeedResult {
    NotModified,
    Updated { entry_count: usize, localization_entries: Vec<RefreshLocalizedEntry> },
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshLocalizedEntry {
    pub dedup_key: String,
    pub url: Option<Url>,
    pub title: String,
    pub content_html: String,
    pub content_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshFeedOutcome {
    pub feed_id: i64,
    pub url: String,
    pub result: RefreshFeedResult,
}

impl RefreshFeedOutcome {
    pub fn is_success(&self) -> bool {
        !matches!(self.result, RefreshFeedResult::Failed { .. })
    }

    pub fn failure_message(&self) -> Option<&str> {
        match &self.result {
            RefreshFeedResult::Failed { message } => Some(message),
            _ => None,
        }
    }

    pub fn failure_summary(&self) -> Option<RefreshFeedFailureSummary> {
        self.failure_message().map(|message| RefreshFeedFailureSummary {
            feed_id: self.feed_id,
            url: self.url.clone(),
            message: message.to_string(),
        })
    }

    pub fn failure_line(&self) -> Option<String> {
        self.failure_summary().map(|failure| failure.display_line())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshFeedFailureSummary {
    pub feed_id: i64,
    pub url: String,
    pub message: String,
}

impl RefreshFeedFailureSummary {
    pub fn display_line(&self) -> String {
        format!("{}: {}", self.url, self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshAllOutcome {
    pub feeds: Vec<RefreshFeedOutcome>,
}

impl RefreshAllOutcome {
    pub fn summary(&self) -> RefreshAllSummary {
        let mut summary =
            RefreshAllSummary { total_count: self.feeds.len(), ..RefreshAllSummary::default() };

        for outcome in &self.feeds {
            match &outcome.result {
                RefreshFeedResult::Updated { .. } => summary.updated_count += 1,
                RefreshFeedResult::NotModified => summary.not_modified_count += 1,
                RefreshFeedResult::Failed { .. } => {
                    summary.failed_count += 1;
                    if let Some(failure) = outcome.failure_summary() {
                        summary.failures.push(failure);
                    }
                }
            }
        }

        summary
    }

    pub fn has_failures(&self) -> bool {
        self.summary().has_failures()
    }

    pub fn updated_count(&self) -> usize {
        self.summary().updated_count
    }

    pub fn not_modified_count(&self) -> usize {
        self.summary().not_modified_count
    }

    pub fn failures(&self) -> Vec<&RefreshFeedOutcome> {
        self.feeds.iter().filter(|outcome| outcome.failure_message().is_some()).collect()
    }

    pub fn failure_summaries(&self) -> Vec<RefreshFeedFailureSummary> {
        self.summary().failures
    }

    pub fn joined_failure_lines(&self) -> Option<String> {
        self.summary().joined_failure_lines()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshAllSummary {
    pub total_count: usize,
    pub updated_count: usize,
    pub not_modified_count: usize,
    pub failed_count: usize,
    pub failures: Vec<RefreshFeedFailureSummary>,
}

impl RefreshAllSummary {
    pub fn has_failures(&self) -> bool {
        self.failed_count > 0
    }

    pub fn joined_failure_lines(&self) -> Option<String> {
        if self.failures.is_empty() {
            None
        } else {
            Some(
                self.failures
                    .iter()
                    .map(RefreshFeedFailureSummary::display_line)
                    .collect::<Vec<_>>()
                    .join(" | "),
            )
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait FeedRefreshSourcePort: Send + Sync {
    async fn refresh(&self, target: &RefreshTarget) -> Result<FeedRefreshSourceOutput>;
}

#[async_trait::async_trait]
pub trait RefreshStorePort: Send + Sync {
    async fn list_targets(&self) -> Result<Vec<RefreshTarget>>;
    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>>;
    async fn commit(&self, feed_id: i64, commit: RefreshCommit) -> Result<()>;

    /// 打开一个写入批次：批次期间的 [`RefreshStorePort::commit`] 只需保证改动对后续读取可见，
    /// 真正的落盘可以推迟到 [`RefreshStorePort::end_batch`]。
    ///
    /// 默认空实现。事务型存储（SQLite）每次 `commit` 就已经落盘，没有可推迟的东西；
    /// 需要整片重写的存储（浏览器 `localStorage`）才靠它把「每个订阅重写一次全库」压成
    /// 「整轮重写一次」。
    ///
    /// **不变量**：批次一旦打开，必须以 `end_batch` 或 `abort_batch` 收尾。
    /// 这一条由 `refresh_all` 里的 [`RefreshBatchGuard`] 强制，不靠调用方自觉。
    /// 重复打开也必须是安全的：实现方要先把上一个没关掉的批次冲掉。
    async fn begin_batch(&self) -> Result<()> {
        Ok(())
    }

    /// 关闭批次，把批次内累积的改动一次性落盘。批次内没有发生过 `commit` 时不产生写入。
    async fn end_batch(&self) -> Result<()> {
        Ok(())
    }

    /// 放弃批次：关掉批次状态，并尽力把已经累积的改动落盘。
    ///
    /// **必须是同步的**，因为它要能从 [`RefreshBatchGuard`] 的 `Drop` 里调用——
    /// 而 future 被取消时能跑的只有析构。也因此它没有返回值：这条路径上没有任何地方
    /// 可以上报错误，实现方只能自己记日志。
    ///
    /// **必须幂等**：正常路径上 `end_batch` 之后守卫析构还会再调一次，那时应当什么都不做。
    fn abort_batch(&self) {}
}

/// 保证批次一定被关掉的守卫。
///
/// 光靠「`begin_batch` 与 `end_batch` 之间不写 `?`」是不够的：Web 端刷新任务是
/// Dioxus 作用域绑定的 `spawn`（`rssr-app` 的 `spawn_projected_ui_command`），
/// 用户在刷新途中离开订阅页，组件一卸载整个 future 就被 drop，`end_batch` 永远等不到。
/// 那样批次会永久停在打开状态，此后**批次外**的提交（单订阅刷新、添加订阅的首刷）
/// 全都只改内存不落盘，而且不报错——正是最难被发现的那种丢数据。
///
/// 守卫不设「解除」开关：`abort_batch` 要求幂等，正常收尾后再调一次是空操作，
/// 这样就不存在「已解除但还没 `end_batch`」的窗口。
struct RefreshBatchGuard<'a> {
    store: &'a dyn RefreshStorePort,
}

impl Drop for RefreshBatchGuard<'_> {
    fn drop(&mut self) {
        self.store.abort_batch();
    }
}

#[derive(Clone)]
pub struct RefreshService {
    source: Arc<dyn FeedRefreshSourcePort>,
    store: Arc<dyn RefreshStorePort>,
}

impl RefreshService {
    pub fn new(source: Arc<dyn FeedRefreshSourcePort>, store: Arc<dyn RefreshStorePort>) -> Self {
        Self { source, store }
    }

    pub async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedOutcome> {
        let target = self.store.get_target(feed_id).await?.context("订阅不存在")?;
        self.refresh_target(target).await
    }

    /// 刷新全部订阅。
    ///
    /// 单个订阅的失败**不会**中断整轮：它会变成该订阅的 [`RefreshFeedResult::Failed`]，
    /// 这也是 [`RefreshAllOutcome`] 本来就建模了 per-feed 失败的原因。
    /// 返回 `Err` 只留给整轮级别的前置失败（例如读不出订阅列表）。
    ///
    /// 这一点在并发化之后变得关键：此前用 `?` 传播单个订阅的错误，会在 `JoinSet` 仍有在飞任务时
    /// 直接返回，`JoinSet` 析构会 abort 那些任务——它们可能正停在写事务里，而且整轮结果连同
    /// 已经成功的订阅一起被丢掉（既不做图片本地化，也给不出 per-feed 失败报告）。
    pub async fn refresh_all(&self, input: RefreshAllInput) -> Result<RefreshAllOutcome> {
        let targets = self.store.list_targets().await.context("读取订阅列表失败")?;

        // 整轮刷新算一个写入批次。
        //
        // 守卫在 `begin_batch` 的结果被 `?` 掉之前就建立：那次调用即使失败，实现方也可能
        // 已经把批次置为打开，带着 `?` 直接返回会把它永久留在打开状态。
        // 守卫同时兜住 future 被取消的情况——见 [`RefreshBatchGuard`]，这条路径在 Web 上
        // 只需要用户在刷新途中离开订阅页就会发生。
        let begun = self.store.begin_batch().await;
        let _batch = RefreshBatchGuard { store: self.store.as_ref() };
        begun.context("打开刷新写入批次失败")?;

        let mut outcomes = self.refresh_targets(targets, input).await;
        self.end_batch_or_mark_failed(&mut outcomes).await;

        Ok(RefreshAllOutcome { feeds: outcomes })
    }

    /// 关闭批次并落盘；写失败时把本轮的非失败结果一律改写成失败。
    ///
    /// 批次没写进去还报成功是在骗人——重开页面时这些条目并不存在，
    /// 而且下游会照着这份结果去做图片本地化。改写成失败也与「每次 commit 各自落盘」时代的
    /// 行为一致：那时写入失败同样会让对应订阅报错。
    async fn end_batch_or_mark_failed(&self, outcomes: &mut [RefreshFeedOutcome]) {
        let Err(error) = self.store.end_batch().await else {
            return;
        };

        let message = format!("刷新结果写入本地存储失败: {error}");
        for outcome in outcomes.iter_mut() {
            if !matches!(outcome.result, RefreshFeedResult::Failed { .. }) {
                outcome.result = RefreshFeedResult::Failed { message: message.clone() };
            }
        }
    }

    /// 逐个刷新订阅并把每个的结果收敛成 [`RefreshFeedOutcome`]，本身不会返回 `Err`。
    ///
    /// 平台差异只在「串行还是并发」这一点上：wasm 没有可用的多线程运行时，走串行；
    /// 原生端按 `max_concurrency` 走 `JoinSet`。两边产出的语义完全一致。
    async fn refresh_targets(
        &self,
        targets: Vec<RefreshTarget>,
        input: RefreshAllInput,
    ) -> Vec<RefreshFeedOutcome> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = input;
            let mut outcomes = Vec::with_capacity(targets.len());
            for target in targets {
                outcomes.push(self.refresh_target_reporting(target).await);
            }
            return outcomes;
        }

        #[cfg(not(target_arch = "wasm32"))]
        let max_concurrency = input.max_concurrency.max(1);

        #[cfg(not(target_arch = "wasm32"))]
        if max_concurrency == 1 {
            let mut outcomes = Vec::with_capacity(targets.len());
            for target in targets {
                outcomes.push(self.refresh_target_reporting(target).await);
            }
            return outcomes;
        }

        #[cfg(not(target_arch = "wasm32"))]
        self.refresh_targets_concurrent(targets, max_concurrency).await
    }

    /// 原生端的并发分支：最多 `max_concurrency` 个订阅同时在飞。
    #[cfg(not(target_arch = "wasm32"))]
    async fn refresh_targets_concurrent(
        &self,
        targets: Vec<RefreshTarget>,
        max_concurrency: usize,
    ) -> Vec<RefreshFeedOutcome> {
        {
            let mut outcomes = Vec::with_capacity(targets.len());
            let mut target_iter = targets.into_iter();
            let mut in_flight = JoinSet::new();
            // 任务 id -> 订阅身份：任务 panic 时 join 不回任何结果，靠这张表把它记成该订阅的失败，
            // 而不是让一个 bug 把整轮刷新变成一条笼统错误。
            let mut pending: std::collections::HashMap<tokio::task::Id, (i64, String)> =
                std::collections::HashMap::new();

            loop {
                while in_flight.len() < max_concurrency {
                    let Some(target) = target_iter.next() else {
                        break;
                    };
                    let identity = (target.feed_id, target.url.to_string());
                    let service = self.clone();
                    let handle = in_flight
                        .spawn(async move { service.refresh_target_reporting(target).await });
                    pending.insert(handle.id(), identity);
                }

                let Some(result) = in_flight.join_next_with_id().await else {
                    break;
                };

                match result {
                    Ok((task_id, outcome)) => {
                        pending.remove(&task_id);
                        outcomes.push(outcome);
                    }
                    Err(join_error) => {
                        if let Some((feed_id, url)) = pending.remove(&join_error.id()) {
                            outcomes.push(RefreshFeedOutcome {
                                feed_id,
                                url,
                                result: RefreshFeedResult::Failed {
                                    message: format!("刷新任务意外结束: {join_error}"),
                                },
                            });
                        }
                    }
                }
            }

            // 完成顺序是非确定的，按 feed_id 归一，避免失败提示的行序在界面与 CLI 里抖动。
            outcomes.sort_by_key(|outcome| outcome.feed_id);

            outcomes
        }
    }

    /// 把「这一个订阅刷新失败」收敛成该订阅的 `Failed` 结果。
    async fn refresh_target_reporting(&self, target: RefreshTarget) -> RefreshFeedOutcome {
        let feed_id = target.feed_id;
        let url = target.url.to_string();

        match self.refresh_target(target).await {
            Ok(outcome) => outcome,
            Err(error) => RefreshFeedOutcome {
                feed_id,
                url,
                result: RefreshFeedResult::Failed { message: error.to_string() },
            },
        }
    }

    async fn refresh_target(&self, target: RefreshTarget) -> Result<RefreshFeedOutcome> {
        let source_output = match self.source.refresh(&target).await {
            Ok(output) => output,
            Err(error) => FeedRefreshSourceOutput::Failed(RefreshFailure {
                message: error.to_string(),
                metadata: None,
            }),
        };

        match source_output {
            FeedRefreshSourceOutput::NotModified(metadata) => {
                self.store
                    .commit(
                        target.feed_id,
                        RefreshCommit::NotModified { metadata: metadata.clone() },
                    )
                    .await?;
                Ok(RefreshFeedOutcome {
                    feed_id: target.feed_id,
                    url: target.url.to_string(),
                    result: RefreshFeedResult::NotModified,
                })
            }
            FeedRefreshSourceOutput::Updated(update) => {
                let entry_count = update.feed.entries.len();
                let localization_entries = build_localization_entries(&update.feed.entries);
                self.store.commit(target.feed_id, RefreshCommit::Updated { update }).await?;
                Ok(RefreshFeedOutcome {
                    feed_id: target.feed_id,
                    url: target.url.to_string(),
                    result: RefreshFeedResult::Updated { entry_count, localization_entries },
                })
            }
            FeedRefreshSourceOutput::Failed(failure) => {
                self.store
                    .commit(target.feed_id, RefreshCommit::Failed { failure: failure.clone() })
                    .await?;
                Ok(RefreshFeedOutcome {
                    feed_id: target.feed_id,
                    url: target.url.to_string(),
                    result: RefreshFeedResult::Failed { message: failure.message },
                })
            }
        }
    }
}

fn build_localization_entries(entries: &[ParsedEntryData]) -> Vec<RefreshLocalizedEntry> {
    entries
        .iter()
        .filter_map(|entry| {
            entry.content_html.as_ref().map(|content_html| RefreshLocalizedEntry {
                dedup_key: entry.dedup_key.clone(),
                url: entry.url.clone(),
                title: entry.title.clone(),
                content_html: content_html.clone(),
                content_text: entry.content_text.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::Result;
    use url::Url;

    use super::{
        FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
        ParsedFeedUpdate, RefreshAllInput, RefreshCommit, RefreshFailure, RefreshHttpMetadata,
        RefreshService, RefreshStorePort, RefreshTarget,
    };

    struct SourceStub {
        outputs: Mutex<Vec<FeedRefreshSourceOutput>>,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl FeedRefreshSourcePort for SourceStub {
        async fn refresh(&self, _target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
            Ok(self.outputs.lock().expect("lock outputs").remove(0))
        }
    }

    struct StoreStub {
        targets: Vec<RefreshTarget>,
        commits: Mutex<Vec<(i64, RefreshCommit)>>,
    }

    #[async_trait::async_trait]
    impl RefreshStorePort for StoreStub {
        async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
            Ok(self.targets.clone())
        }

        async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
            Ok(self.targets.iter().find(|target| target.feed_id == feed_id).cloned())
        }

        async fn commit(&self, feed_id: i64, commit: RefreshCommit) -> Result<()> {
            self.commits.lock().expect("lock commits").push((feed_id, commit));
            Ok(())
        }
    }

    fn sample_target(feed_id: i64, url: &str) -> RefreshTarget {
        RefreshTarget {
            feed_id,
            url: Url::parse(url).expect("valid url"),
            etag: Some("etag".to_string()),
            last_modified: Some("last-modified".to_string()),
        }
    }

    fn sample_entry() -> ParsedEntryData {
        ParsedEntryData {
            external_id: "entry-1".to_string(),
            dedup_key: "entry-1".to_string(),
            url: Some(Url::parse("https://example.com/entry-1").expect("valid url")),
            title: "Entry".to_string(),
            author: None,
            summary: Some("summary".to_string()),
            content_html: Some("<p>summary</p>".to_string()),
            content_text: Some("summary".to_string()),
            published_at: None,
            updated_at_source: None,
        }
    }

    fn sample_entry_without_html() -> ParsedEntryData {
        ParsedEntryData {
            external_id: "entry-2".to_string(),
            dedup_key: "entry-2".to_string(),
            url: Some(Url::parse("https://example.com/entry-2").expect("valid url")),
            title: "Entry 2".to_string(),
            author: None,
            summary: Some("summary 2".to_string()),
            content_html: None,
            content_text: Some("summary 2".to_string()),
            published_at: None,
            updated_at_source: None,
        }
    }

    #[test]
    fn refresh_feed_failure_summary_keeps_feed_identity() {
        let outcome = super::RefreshFeedOutcome {
            feed_id: 9,
            url: "https://example.com/feed.xml".to_string(),
            result: super::RefreshFeedResult::Failed { message: "boom".to_string() },
        };

        assert!(!outcome.is_success());
        assert_eq!(outcome.failure_message(), Some("boom"));
        assert_eq!(outcome.failure_line(), Some("https://example.com/feed.xml: boom".to_string()));
        assert_eq!(
            outcome.failure_summary(),
            Some(super::RefreshFeedFailureSummary {
                feed_id: 9,
                url: "https://example.com/feed.xml".to_string(),
                message: "boom".to_string(),
            })
        );
    }

    #[test]
    fn refresh_all_summary_counts_results_and_formats_failures() {
        let outcome = super::RefreshAllOutcome {
            feeds: vec![
                super::RefreshFeedOutcome {
                    feed_id: 1,
                    url: "https://example.com/one.xml".to_string(),
                    result: super::RefreshFeedResult::Updated {
                        entry_count: 2,
                        localization_entries: Vec::new(),
                    },
                },
                super::RefreshFeedOutcome {
                    feed_id: 2,
                    url: "https://example.com/two.xml".to_string(),
                    result: super::RefreshFeedResult::NotModified,
                },
                super::RefreshFeedOutcome {
                    feed_id: 3,
                    url: "https://example.com/three.xml".to_string(),
                    result: super::RefreshFeedResult::Failed { message: "boom".to_string() },
                },
            ],
        };

        let summary = outcome.summary();

        assert_eq!(summary.total_count, 3);
        assert_eq!(summary.updated_count, 1);
        assert_eq!(summary.not_modified_count, 1);
        assert_eq!(summary.failed_count, 1);
        assert!(summary.has_failures());
        assert_eq!(summary.failures[0].feed_id, 3);
        assert_eq!(outcome.updated_count(), 1);
        assert_eq!(outcome.not_modified_count(), 1);
        assert_eq!(
            outcome.joined_failure_lines(),
            Some("https://example.com/three.xml: boom".to_string())
        );
    }

    #[tokio::test]
    async fn refresh_feed_commits_updated_entries_on_happy_path() {
        let service = RefreshService::new(
            Arc::new(SourceStub {
                outputs: Mutex::new(vec![FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
                    metadata: RefreshHttpMetadata {
                        etag: Some("new-etag".to_string()),
                        last_modified: Some("new-last-modified".to_string()),
                    },
                    feed: ParsedFeedUpdate {
                        title: Some("Example".to_string()),
                        site_url: Some(Url::parse("https://example.com").expect("valid url")),
                        description: Some("desc".to_string()),
                        entries: vec![sample_entry()],
                    },
                })]),
            }),
            Arc::new(StoreStub {
                targets: vec![sample_target(1, "https://example.com/feed.xml")],
                commits: Mutex::new(Vec::new()),
            }),
        );

        let outcome = service.refresh_feed(1).await.expect("refresh feed");

        assert!(matches!(outcome.result, super::RefreshFeedResult::Updated { entry_count: 1, .. }));
    }

    #[tokio::test]
    async fn refresh_feed_outcome_only_exposes_localization_candidates() {
        let service = RefreshService::new(
            Arc::new(SourceStub {
                outputs: Mutex::new(vec![FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
                    metadata: RefreshHttpMetadata::default(),
                    feed: ParsedFeedUpdate {
                        title: Some("Example".to_string()),
                        site_url: None,
                        description: None,
                        entries: vec![sample_entry(), sample_entry_without_html()],
                    },
                })]),
            }),
            Arc::new(StoreStub {
                targets: vec![sample_target(4, "https://example.com/feed.xml")],
                commits: Mutex::new(Vec::new()),
            }),
        );

        let outcome = service.refresh_feed(4).await.expect("refresh feed");

        match outcome.result {
            super::RefreshFeedResult::Updated { entry_count, localization_entries } => {
                assert_eq!(entry_count, 2);
                assert_eq!(localization_entries.len(), 1);
                assert_eq!(localization_entries[0].dedup_key, "entry-1");
                assert_eq!(localization_entries[0].title, "Entry");
                assert_eq!(localization_entries[0].content_html, "<p>summary</p>");
            }
            _ => panic!("expected updated outcome"),
        }
    }

    #[tokio::test]
    async fn refresh_feed_commits_failures_without_hiding_them() {
        let store = Arc::new(StoreStub {
            targets: vec![sample_target(2, "https://example.com/feed.xml")],
            commits: Mutex::new(Vec::new()),
        });
        let service = RefreshService::new(
            Arc::new(SourceStub {
                outputs: Mutex::new(vec![FeedRefreshSourceOutput::Failed(RefreshFailure {
                    message: "抓取订阅失败".to_string(),
                    metadata: None,
                })]),
            }),
            store.clone(),
        );

        let outcome = service.refresh_feed(2).await.expect("refresh feed");

        assert_eq!(outcome.failure_message(), Some("抓取订阅失败"));
        assert!(matches!(
            store.commits.lock().expect("lock commits")[0].1,
            RefreshCommit::Failed { .. }
        ));
    }

    #[tokio::test]
    async fn refresh_feed_handles_not_modified_without_rewriting_entries() {
        let store = Arc::new(StoreStub {
            targets: vec![sample_target(3, "https://example.com/feed.xml")],
            commits: Mutex::new(Vec::new()),
        });
        let service = RefreshService::new(
            Arc::new(SourceStub {
                outputs: Mutex::new(vec![FeedRefreshSourceOutput::NotModified(
                    RefreshHttpMetadata {
                        etag: Some("etag-2".to_string()),
                        last_modified: Some("lm-2".to_string()),
                    },
                )]),
            }),
            store.clone(),
        );

        let outcome = service.refresh_feed(3).await.expect("refresh feed");

        assert!(matches!(outcome.result, super::RefreshFeedResult::NotModified));
        assert!(matches!(
            store.commits.lock().expect("lock commits")[0].1,
            RefreshCommit::NotModified { .. }
        ));
    }

    /// 并发分支专用的 source stub：按 target 应答，而不是按调用次序从队列里 pop。
    struct KeyedSourceStub;

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl FeedRefreshSourcePort for KeyedSourceStub {
        async fn refresh(&self, _target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
            Ok(FeedRefreshSourceOutput::NotModified(RefreshHttpMetadata::default()))
        }
    }

    /// 对某一个 feed 的 commit 报错，用来验证单个订阅失败不会带走整轮。
    struct FailingCommitStore {
        targets: Vec<RefreshTarget>,
        failing_feed_id: i64,
    }

    #[async_trait::async_trait]
    impl RefreshStorePort for FailingCommitStore {
        async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
            Ok(self.targets.clone())
        }

        async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
            Ok(self.targets.iter().find(|target| target.feed_id == feed_id).cloned())
        }

        async fn commit(&self, feed_id: i64, _commit: RefreshCommit) -> Result<()> {
            if feed_id == self.failing_feed_id {
                anyhow::bail!("提交失败");
            }
            Ok(())
        }
    }

    /// 按调用次序记录端口调用，用来验证批次确实是整轮的外壳而不是每个订阅各开一个。
    struct BatchRecordingStore {
        targets: Vec<RefreshTarget>,
        calls: Mutex<Vec<String>>,
        end_batch_error: Option<String>,
    }

    impl BatchRecordingStore {
        fn new(targets: Vec<RefreshTarget>, end_batch_error: Option<&str>) -> Self {
            Self {
                targets,
                calls: Mutex::new(Vec::new()),
                end_batch_error: end_batch_error.map(ToOwned::to_owned),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("lock calls").clone()
        }

        fn record(&self, call: &str) {
            self.calls.lock().expect("lock calls").push(call.to_string());
        }
    }

    #[async_trait::async_trait]
    impl RefreshStorePort for BatchRecordingStore {
        async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
            Ok(self.targets.clone())
        }

        async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
            Ok(self.targets.iter().find(|target| target.feed_id == feed_id).cloned())
        }

        async fn commit(&self, feed_id: i64, _commit: RefreshCommit) -> Result<()> {
            self.record(&format!("commit:{feed_id}"));
            Ok(())
        }

        async fn begin_batch(&self) -> Result<()> {
            self.record("begin");
            Ok(())
        }

        fn abort_batch(&self) {
            self.record("abort");
        }

        async fn end_batch(&self) -> Result<()> {
            self.record("end");
            match &self.end_batch_error {
                Some(message) => anyhow::bail!("{message}"),
                None => Ok(()),
            }
        }
    }

    /// 整轮刷新只开一个批次，且所有 commit 都落在批次内。
    ///
    /// 这条断言的是浏览器存储省下整片重写的前提：批次要是退化成「每个订阅一对 begin/end」，
    /// 写入次数就一次没少。
    #[tokio::test]
    async fn refresh_all_wraps_the_whole_round_in_a_single_write_batch() {
        let targets = (1..=3)
            .map(|feed_id| sample_target(feed_id, &format!("https://example.com/{feed_id}.xml")))
            .collect::<Vec<_>>();
        let store = Arc::new(BatchRecordingStore::new(targets, None));

        let outcome = RefreshService::new(Arc::new(KeyedSourceStub), store.clone())
            .refresh_all(RefreshAllInput { max_concurrency: 1 })
            .await
            .expect("refresh all");

        assert_eq!(outcome.feeds.len(), 3);
        assert_eq!(
            store.calls(),
            vec!["begin", "commit:1", "commit:2", "commit:3", "end", "abort"],
            "批次必须包住整轮，而不是每个订阅各开一次"
        );
        // 末尾那次 `abort` 来自批次守卫析构。它必须是幂等的空操作——
        // 守卫刻意不设「解除」开关，否则就会出现「已解除但还没 end_batch」的取消窗口。
    }

    /// 刷新途中 future 被取消时，批次必须被关掉。
    ///
    /// Web 端刷新任务绑定在订阅页组件的作用域上，用户刷新途中切走页面，整个 future 就被 drop，
    /// `end_batch` 永远等不到。少了守卫的话批次会永久停在打开状态，此后单订阅刷新与
    /// 添加订阅的首刷都只改内存不落盘，而且一声不吭。
    #[tokio::test]
    async fn a_cancelled_round_still_closes_the_batch() {
        struct HangingSourceStub;

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl FeedRefreshSourcePort for HangingSourceStub {
            async fn refresh(&self, _target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
                std::future::pending().await
            }
        }

        let targets = vec![sample_target(1, "https://example.com/1.xml")];
        let store = Arc::new(BatchRecordingStore::new(targets, None));
        let service = RefreshService::new(Arc::new(HangingSourceStub), store.clone());

        let cancelled = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            service.refresh_all(RefreshAllInput { max_concurrency: 1 }),
        )
        .await;

        assert!(cancelled.is_err(), "抓取永不返回，这一轮必须是被超时取消的");
        assert_eq!(
            store.calls(),
            vec!["begin", "abort"],
            "取消路径上必须留下 abort，否则批次就永久开着了"
        );
    }

    /// 批次落盘失败时，本轮不能报成功。
    ///
    /// 抓取确实成功了，但结果没写进存储——重开页面时这些条目并不存在。
    /// 这里报成功还会让下游照着这份结果去做图片本地化。
    #[tokio::test]
    async fn a_failed_batch_write_turns_the_whole_round_into_failures() {
        let targets = (1..=2)
            .map(|feed_id| sample_target(feed_id, &format!("https://example.com/{feed_id}.xml")))
            .collect::<Vec<_>>();
        let store = Arc::new(BatchRecordingStore::new(targets, Some("存储配额已满")));

        let outcome = RefreshService::new(Arc::new(KeyedSourceStub), store.clone())
            .refresh_all(RefreshAllInput { max_concurrency: 1 })
            .await
            .expect("落盘失败也不该把整轮变成 Err——per-feed 结果仍要交回去");

        assert_eq!(outcome.feeds.len(), 2);
        for feed in &outcome.feeds {
            match &feed.result {
                super::RefreshFeedResult::Failed { message } => {
                    assert!(
                        message.contains("存储配额已满"),
                        "失败原因要带上真正的写入错误：{message}"
                    );
                }
                other => panic!("批次没写成功时不该报成功：{other:?}"),
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_refresh_reports_one_feed_failure_without_dropping_the_round() {
        let targets = (1..=6)
            .map(|feed_id| sample_target(feed_id, &format!("https://example.com/{feed_id}.xml")))
            .collect::<Vec<_>>();
        let service = RefreshService::new(
            Arc::new(KeyedSourceStub),
            Arc::new(FailingCommitStore { targets, failing_feed_id: 3 }),
        );

        let outcome = service
            .refresh_all(RefreshAllInput { max_concurrency: 4 })
            .await
            .expect("单个订阅失败不应让整轮刷新返回 Err");

        // 六个订阅都要有结果，而不是「一个失败就整轮丢掉」。
        assert_eq!(outcome.feeds.len(), 6);
        // 完成顺序不确定，因此结果必须按 feed_id 归一，否则失败提示行序会抖动。
        assert_eq!(
            outcome.feeds.iter().map(|feed| feed.feed_id).collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );

        let summary = outcome.summary();
        assert_eq!(summary.failed_count, 1);
        assert_eq!(summary.not_modified_count, 5);
        assert_eq!(summary.failures[0].feed_id, 3);
    }

    #[tokio::test]
    async fn refresh_all_aggregates_per_feed_outcomes() {
        let service = RefreshService::new(
            Arc::new(SourceStub {
                outputs: Mutex::new(vec![
                    FeedRefreshSourceOutput::NotModified(RefreshHttpMetadata::default()),
                    FeedRefreshSourceOutput::Failed(RefreshFailure {
                        message: "boom".to_string(),
                        metadata: None,
                    }),
                ]),
            }),
            Arc::new(StoreStub {
                targets: vec![
                    sample_target(1, "https://example.com/one.xml"),
                    sample_target(2, "https://example.com/two.xml"),
                ],
                commits: Mutex::new(Vec::new()),
            }),
        );

        let outcome =
            service.refresh_all(RefreshAllInput { max_concurrency: 1 }).await.expect("refresh all");

        assert_eq!(outcome.not_modified_count(), 1);
        assert_eq!(outcome.failures().len(), 1);
    }
}
