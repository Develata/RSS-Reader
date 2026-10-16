use std::{
    collections::HashMap,
    sync::{
        Arc, PoisonError, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::{Context, Result};
use rssr_domain::{DomainError, FetchErrorKind};
use time::OffsetDateTime;

use crate::error::{ErrorKind, classify};
use url::Url;

#[cfg(not(target_arch = "wasm32"))]
use tokio::task::JoinSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshTarget {
    pub feed_id: i64,
    /// Monotonic subscription generation. Incremented only when a soft-deleted feed is reactivated.
    /// A refresh may commit only to the generation from which it was started.
    pub generation: i64,
    pub url: Url,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    /// Persistent refresh ticket; [`RefreshTicket::UNTRACKED`] for stores without tickets.
    pub ticket: RefreshTicket,
}

/// A refresh's place in the persistent order of one subscription's refreshes, so that refreshes
/// from different processes sharing a library (GUI, CLI, server) cannot overwrite a newer result
/// with an older one.
///
/// The store keeps per subscription the highest ticket issued (I), published (P) and diagnosed
/// (D), with `0 <= P <= D <= I`. A ticket is issued when the fetch starts, together with the
/// validators sent and the P they belong to (`baseline`). At commit:
/// - a fetched feed is published only if `issued > P`;
/// - a 304 is recorded only if `baseline == P` (it confirms exactly the published version);
/// - a failure is recorded only if `issued > D`.
///
/// Otherwise the commit is superseded and writes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RefreshTicket {
    pub issued: i64,
    pub baseline: i64,
}

impl RefreshTicket {
    /// No ticket: for stores without persistent tickets (the browser store), ordered by the
    /// in-process sequence instead; a store that tracks tickets refuses commits carrying it.
    pub const UNTRACKED: Self = Self { issued: 0, baseline: 0 };
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

/// A failed fetch of one subscription, as the store records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshFailure {
    pub kind: FetchErrorKind,
    /// Shown as recorded for fetch and parse failures; internal for storage ones.
    pub message: String,
    pub metadata: Option<RefreshHttpMetadata>,
}

impl RefreshFailure {
    pub fn fetch(message: impl Into<String>, metadata: Option<RefreshHttpMetadata>) -> Self {
        Self { kind: FetchErrorKind::Fetch, message: message.into(), metadata }
    }

    pub fn parse(message: impl Into<String>, metadata: Option<RefreshHttpMetadata>) -> Self {
        Self { kind: FetchErrorKind::Parse, message: message.into(), metadata }
    }

    pub fn storage(message: impl Into<String>, metadata: Option<RefreshHttpMetadata>) -> Self {
        Self { kind: FetchErrorKind::Storage, message: message.into(), metadata }
    }
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
    Updated {
        entry_count: usize,
        inserted_count: u64,
        localization_entries: Vec<RefreshLocalizedEntry>,
    },
    /// `message` is for the user: a storage or internal cause is a fixed text (logged in full
    /// where it was classified).
    Failed {
        kind: ErrorKind,
        message: String,
    },
}

impl RefreshFeedResult {
    /// The failure an error stands for, classified before it becomes text.
    fn failed(error: &anyhow::Error) -> Self {
        let public = classify(error);
        Self::Failed { kind: public.kind, message: public.message }
    }

    /// The failure a fetch reported: a remote failure shows its text, a storage one does not.
    fn source_failed(failure: &RefreshFailure) -> Self {
        match failure.kind {
            FetchErrorKind::Fetch | FetchErrorKind::Parse => {
                Self::Failed { kind: ErrorKind::Network, message: failure.message.clone() }
            }
            FetchErrorKind::Storage => {
                tracing::warn!(error = %failure.message, "保存刷新结果失败");
                Self::Failed {
                    kind: ErrorKind::Storage,
                    message: ErrorKind::Storage.generic_message().to_string(),
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshLocalizedEntry {
    /// Feed generation captured by the refresh that produced this localization work.
    pub generation: i64,
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
    pub fn inserted_count(&self) -> u64 {
        match self.result {
            RefreshFeedResult::Updated { inserted_count, .. } => inserted_count,
            _ => 0,
        }
    }

    pub fn is_success(&self) -> bool {
        !matches!(self.result, RefreshFeedResult::Failed { .. })
    }

    pub fn failure_message(&self) -> Option<&str> {
        match &self.result {
            RefreshFeedResult::Failed { message, .. } => Some(message),
            _ => None,
        }
    }

    /// The failure, without a name (the refresh does not read it).
    pub fn failure_summary(&self) -> Option<RefreshFailureItem> {
        match &self.result {
            RefreshFeedResult::Failed { kind, message } => Some(RefreshFailureItem {
                feed_id: self.feed_id,
                url: self.url.clone(),
                name: None,
                kind: *kind,
                message: message.clone(),
            }),
            _ => None,
        }
    }
}

/// A subscription whose refresh failed. The GUI and the CLI format it themselves.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RefreshFailureItem {
    pub feed_id: i64,
    pub url: String,
    /// The subscription's display name when known; show the address otherwise.
    pub name: Option<String>,
    pub kind: ErrorKind,
    /// For the user (see [`RefreshFeedResult::Failed`]).
    pub message: String,
}

/// What refreshing every subscription led to, as the user interface receives it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RefreshAllExecutionOutcome {
    pub inserted_count: u64,
    pub total_count: u64,
    pub failed_count: u64,
    /// Empty when everything succeeded; fewer than `total_count` is a partial success. Bounded
    /// (`RefreshAllExecutionOutcome::bounded`): see `failures_truncated`.
    pub failures: Vec<RefreshFailureItem>,
    /// Some failures are not listed (too many, or too large to send): `failed_count` is the full
    /// count.
    #[serde(default)]
    pub failures_truncated: bool,
}

/// What refreshing one subscription led to, as the user interface receives it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RefreshFeedExecutionOutcome {
    pub inserted_count: u64,
    pub failure: Option<RefreshFailureItem>,
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
                RefreshFeedResult::Updated { inserted_count, .. } => {
                    summary.updated_count += 1;
                    summary.inserted_count += inserted_count;
                }
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

    pub fn failure_summaries(&self) -> Vec<RefreshFailureItem> {
        self.summary().failures
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshAllSummary {
    pub inserted_count: u64,
    pub total_count: usize,
    pub updated_count: usize,
    pub not_modified_count: usize,
    pub failed_count: usize,
    pub failures: Vec<RefreshFailureItem>,
}

impl RefreshAllSummary {
    pub fn has_failures(&self) -> bool {
        self.failed_count > 0
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait FeedRefreshSourcePort: Send + Sync {
    async fn refresh(&self, target: &RefreshTarget) -> Result<FeedRefreshSourceOutput>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RefreshCommitOutcome {
    pub inserted_count: u64,
    /// A newer refresh (possibly in another process) already published or diagnosed this
    /// subscription; nothing was written.
    pub superseded: bool,
}

#[async_trait::async_trait]
pub trait RefreshStorePort: Send + Sync {
    async fn list_targets(&self) -> Result<Vec<RefreshTarget>>;
    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>>;

    /// Starts a refresh: the current target with a newly issued [`RefreshTicket`], or `None`
    /// when the subscription is gone or no longer in `expected_generation`. Called right before
    /// the fetch, so ticket order is fetch-start order.
    ///
    /// The default issues no ticket (stores that do not share their data across processes).
    async fn begin_refresh(
        &self,
        feed_id: i64,
        expected_generation: Option<i64>,
    ) -> Result<Option<RefreshTarget>> {
        Ok(self.get_target(feed_id).await?.filter(|target| {
            expected_generation.is_none_or(|generation| generation == target.generation)
        }))
    }
    /// 返回真实新增索引数；批次内的计数需等 end_batch 成功后才可发布。
    async fn commit(
        &self,
        target: &RefreshTarget,
        commit: RefreshCommit,
    ) -> Result<crate::RefreshCommitOutcome>;

    /// 打开一个写入批次：批次期间的 [`RefreshStorePort::commit`] 只需保证改动对后续读取可见，
    /// 真正的落盘可以推迟到 [`RefreshStorePort::end_batch`]。
    ///
    /// 默认空实现。目前两端都用默认实现：SQLite 每次 `commit` 就已经落盘；浏览器端
    /// （`BrowserRefreshStore`）也逐订阅持久化，代价是每个订阅重写一次整片索引（审计 D2）。
    /// 想把整轮写入合并成一次的存储才需要实现它。
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
/// 对推迟落盘的实现，批次会永久停在打开状态，此后**批次外**的提交（单订阅刷新、添加
/// 订阅的首刷）全都只改内存不落盘，而且不报错——正是最难被发现的那种丢数据。
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

/// 同一订阅的刷新逐个进行（审计 A1）。
///
/// 单源刷新、全量刷新、添加订阅的首刷与自动刷新都经过 [`RefreshService`]；抓取与提交
/// 都在该订阅的锁内完成，于是后开始的抓取一定后提交，先发出却后返回的旧响应不会再覆盖
/// 新响应，两次提交的多个写入步骤也不会交错。不同订阅之间互不等待。
///
/// 添加订阅的首刷用的是激活之前就抓好的结果，锁管不到它的“开始时间”：每次抓取开始时
/// 领取一个单调序号，记下每个订阅最后落库内容的序号，预抓取结果只在没有更新的刷新落库时
/// 才写入。
///
/// 只协调同一进程内的刷新：CLI 与图形界面同时刷新同一个数据库、或 Web 端多个标签页，
/// 仍可能交错。锁表只留弱引用；序号表每个订阅一个整数。
#[derive(Default)]
struct FeedRefreshLocks {
    locks: std::sync::Mutex<HashMap<i64, Weak<tokio::sync::Mutex<()>>>>,
    next_fetch: AtomicU64,
    committed_fetch: std::sync::Mutex<HashMap<i64, u64>>,
}

impl FeedRefreshLocks {
    fn lock_for(&self, feed_id: i64) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(lock) = locks.get(&feed_id).and_then(Weak::upgrade) {
            return lock;
        }
        locks.retain(|_, lock| lock.strong_count() > 0);
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(feed_id, Arc::downgrade(&lock));
        lock
    }

    fn begin_fetch(&self) -> u64 {
        self.next_fetch.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// 调用方持有该订阅的锁。
    fn superseded(&self, feed_id: i64, fetch: u64) -> bool {
        let committed = self.committed_fetch.lock().unwrap_or_else(PoisonError::into_inner);
        committed.get(&feed_id).is_some_and(|&latest| latest > fetch)
    }

    /// 调用方持有该订阅的锁。
    fn record_commit(&self, feed_id: i64, fetch: u64) {
        let mut committed = self.committed_fetch.lock().unwrap_or_else(PoisonError::into_inner);
        let latest = committed.entry(feed_id).or_default();
        *latest = (*latest).max(fetch);
    }
}

/// 预抓取结果的序号，必须在订阅对刷新可见（激活）之前领取。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedUpdateTicket(u64);

#[derive(Clone)]
pub struct RefreshService {
    source: Arc<dyn FeedRefreshSourcePort>,
    store: Arc<dyn RefreshStorePort>,
    feed_locks: Arc<FeedRefreshLocks>,
}

impl RefreshService {
    pub fn new(source: Arc<dyn FeedRefreshSourcePort>, store: Arc<dyn RefreshStorePort>) -> Self {
        Self { source, store, feed_locks: Arc::default() }
    }

    /// 在激活订阅之前调用：此后开始的刷新都比预抓取结果新。
    pub fn prepared_update_ticket(&self) -> PreparedUpdateTicket {
        PreparedUpdateTicket(self.feed_locks.begin_fetch())
    }

    /// 复用添加订阅时已获取的 feed，首次入库不再发起第二次请求。
    ///
    /// 订阅激活后、首刷拿到锁之前，后台刷新可能已经把更新的内容落库；那时预抓取的结果
    /// 已经过时，不再写入，按“未变化”返回（审计 A1）。
    /// Writes the response fetched while adding the subscription. The subscription is saved by
    /// then: a failed write is this refresh's failure, not the add's (B2-D5).
    pub async fn apply_prepared_update(
        &self,
        target: RefreshTarget,
        update: FeedRefreshUpdate,
        ticket: PreparedUpdateTicket,
    ) -> RefreshFeedOutcome {
        let lock = self.feed_locks.lock_for(target.feed_id);
        let _serialized = lock.lock().await;
        // A target with a persistent ticket is ordered by the store; the in-process sequence only
        // serves stores without tickets.
        if target.ticket == RefreshTicket::UNTRACKED {
            if self.feed_locks.superseded(target.feed_id, ticket.0) {
                return RefreshFeedOutcome {
                    feed_id: target.feed_id,
                    url: target.url.to_string(),
                    result: RefreshFeedResult::NotModified,
                };
            }
            self.feed_locks.record_commit(target.feed_id, ticket.0);
        }
        let (feed_id, url) = (target.feed_id, target.url.to_string());
        match self.apply_source_output(target, FeedRefreshSourceOutput::Updated(update)).await {
            Ok(outcome) => outcome,
            Err(error) => {
                RefreshFeedOutcome { feed_id, url, result: RefreshFeedResult::failed(&error) }
            }
        }
    }

    pub async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedOutcome> {
        let lock = self.feed_locks.lock_for(feed_id);
        let _serialized = lock.lock().await;
        // 在锁内读取目标：排在前一次刷新之后的请求要带上它刚写入的校验器。
        let target = self
            .store
            .begin_refresh(feed_id, None)
            .await?
            .ok_or(rssr_domain::DomainError::NotFound)?;
        self.fetch_and_apply(target).await
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

        let failed = RefreshFeedResult::failed(&error.context("刷新结果写入本地存储失败"));
        for outcome in outcomes.iter_mut() {
            if !matches!(outcome.result, RefreshFeedResult::Failed { .. }) {
                outcome.result = failed.clone();
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
        #[cfg(not(target_arch = "wasm32"))]
        {
            let max_concurrency = input.max_concurrency.max(1);
            if max_concurrency > 1 {
                return self.refresh_targets_concurrent(targets, max_concurrency).await;
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = input;

        // 串行路径：wasm 始终走这里，原生端在 `max_concurrency == 1` 时走这里。
        let mut outcomes = Vec::with_capacity(targets.len());
        for target in targets {
            outcomes.push(self.refresh_target_reporting(target).await);
        }
        outcomes
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
                            let error = anyhow::Error::new(join_error).context("刷新任务意外结束");
                            outcomes.push(RefreshFeedOutcome {
                                feed_id,
                                url,
                                result: RefreshFeedResult::failed(&error),
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
            Err(error) => {
                RefreshFeedOutcome { feed_id, url, result: RefreshFeedResult::failed(&error) }
            }
        }
    }

    async fn refresh_target(&self, listed: RefreshTarget) -> Result<RefreshFeedOutcome> {
        let lock = self.feed_locks.lock_for(listed.feed_id);
        let _serialized = lock.lock().await;
        // 订阅列表是锁外取的快照。排在别的刷新之后时，要带上它刚写入的校验器：拿旧校验器
        // 换来的 304 只确认旧表示，会把旧 ETag 与新正文配在一起（RFC 9110 §13.1.2）。
        // 期间被删除或重新订阅的不在这一轮刷新。
        let target = match self.store.begin_refresh(listed.feed_id, Some(listed.generation)).await?
        {
            Some(current) => current,
            None => {
                return Err(DomainError::Conflict("订阅已删除或已重新订阅".to_string()).into());
            }
        };
        self.fetch_and_apply(target).await
    }

    /// 调用方必须持有该订阅的刷新锁。
    async fn fetch_and_apply(&self, target: RefreshTarget) -> Result<RefreshFeedOutcome> {
        let fetch = self.feed_locks.begin_fetch();
        let source_output = match self.source.refresh(&target).await {
            Ok(output) => output,
            Err(error) => {
                FeedRefreshSourceOutput::Failed(RefreshFailure::fetch(error.to_string(), None))
            }
        };
        // 失败的抓取没有带来内容，不能让预抓取的结果因此作废。带来内容的落库在开始前就登记
        // 序号，不论成功、失败还是被取消：SQLite 适配器分几步提交（索引、正文、抓取状态），
        // 中途出错或被丢弃时前面的步骤可能已经落库。宁可让更早的预抓取结果作废（下次刷新会
        // 补上），也不让它覆盖可能已经写入的更新内容（独立评审）。调用方持有该订阅的锁，
        // 预抓取结果的检查与这里的登记不会交错。
        if target.ticket == RefreshTicket::UNTRACKED
            && !matches!(source_output, FeedRefreshSourceOutput::Failed(_))
        {
            self.feed_locks.record_commit(target.feed_id, fetch);
        }
        self.apply_source_output(target, source_output).await
    }

    async fn apply_source_output(
        &self,
        target: RefreshTarget,
        source_output: FeedRefreshSourceOutput,
    ) -> Result<RefreshFeedOutcome> {
        match source_output {
            FeedRefreshSourceOutput::NotModified(metadata) => {
                self.store
                    .commit(&target, RefreshCommit::NotModified { metadata: metadata.clone() })
                    .await?;
                // A superseded 304 changed nothing either; both read as "not modified".
                Ok(RefreshFeedOutcome {
                    feed_id: target.feed_id,
                    url: target.url.to_string(),
                    result: RefreshFeedResult::NotModified,
                })
            }
            FeedRefreshSourceOutput::Updated(update) => {
                let entry_count = update.feed.entries.len();
                let localization_entries =
                    build_localization_entries(&update.feed.entries, target.generation);
                let committed =
                    self.store.commit(&target, RefreshCommit::Updated { update }).await?;
                if committed.superseded {
                    // A newer refresh already published; this response is not the stored one.
                    return Ok(RefreshFeedOutcome {
                        feed_id: target.feed_id,
                        url: target.url.to_string(),
                        result: RefreshFeedResult::NotModified,
                    });
                }
                Ok(RefreshFeedOutcome {
                    feed_id: target.feed_id,
                    url: target.url.to_string(),
                    result: RefreshFeedResult::Updated {
                        entry_count,
                        inserted_count: committed.inserted_count,
                        localization_entries,
                    },
                })
            }
            FeedRefreshSourceOutput::Failed(failure) => {
                let committed = self
                    .store
                    .commit(&target, RefreshCommit::Failed { failure: failure.clone() })
                    .await?;
                if committed.superseded {
                    // A newer refresh already succeeded or was diagnosed; this failure is stale.
                    return Ok(RefreshFeedOutcome {
                        feed_id: target.feed_id,
                        url: target.url.to_string(),
                        result: RefreshFeedResult::NotModified,
                    });
                }
                Ok(RefreshFeedOutcome {
                    feed_id: target.feed_id,
                    url: target.url.to_string(),
                    result: RefreshFeedResult::source_failed(&failure),
                })
            }
        }
    }
}

fn build_localization_entries(
    entries: &[ParsedEntryData],
    generation: i64,
) -> Vec<RefreshLocalizedEntry> {
    entries
        .iter()
        .filter_map(|entry| {
            entry.content_html.as_ref().map(|content_html| RefreshLocalizedEntry {
                generation,
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

        async fn commit(
            &self,
            target: &RefreshTarget,
            commit: RefreshCommit,
        ) -> Result<crate::RefreshCommitOutcome> {
            self.commits.lock().expect("lock commits").push((target.feed_id, commit));
            Ok(Default::default())
        }
    }

    fn sample_target(feed_id: i64, url: &str) -> RefreshTarget {
        RefreshTarget {
            feed_id,
            generation: 0,
            url: Url::parse(url).expect("valid url"),
            etag: Some("etag".to_string()),
            last_modified: Some("last-modified".to_string()),
            ticket: Default::default(),
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
            result: super::RefreshFeedResult::Failed {
                kind: crate::ErrorKind::Network,
                message: "boom".to_string(),
            },
        };

        assert!(!outcome.is_success());
        assert_eq!(outcome.failure_message(), Some("boom"));
        let failure = outcome.failure_summary().expect("failure");
        assert_eq!((failure.feed_id, failure.url.as_str()), (9, "https://example.com/feed.xml"));
        assert_eq!((failure.name, failure.message.as_str()), (None, "boom"));
        assert_eq!(failure.kind, crate::ErrorKind::Network);
    }

    #[test]
    fn refresh_all_summary_counts_results_and_lists_failures() {
        let outcome = super::RefreshAllOutcome {
            feeds: vec![
                super::RefreshFeedOutcome {
                    feed_id: 1,
                    url: "https://example.com/one.xml".to_string(),
                    result: super::RefreshFeedResult::Updated {
                        entry_count: 7,
                        inserted_count: 2,
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
                    result: super::RefreshFeedResult::Failed {
                        kind: crate::ErrorKind::Network,
                        message: "boom".to_string(),
                    },
                },
            ],
        };

        let summary = outcome.summary();

        assert_eq!(summary.total_count, 3);
        assert_eq!(summary.inserted_count, 2);
        assert_eq!(summary.updated_count, 1);
        assert_eq!(summary.not_modified_count, 1);
        assert_eq!(summary.failed_count, 1);
        assert!(summary.has_failures());
        assert_eq!(summary.failures[0].feed_id, 3);
        assert_eq!(outcome.updated_count(), 1);
        assert_eq!(outcome.not_modified_count(), 1);
        assert_eq!(summary.failures[0].url, "https://example.com/three.xml");
        assert_eq!(summary.failures[0].message, "boom");
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
            super::RefreshFeedResult::Updated { entry_count, localization_entries, .. } => {
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
                outputs: Mutex::new(vec![FeedRefreshSourceOutput::Failed(RefreshFailure::fetch(
                    "抓取订阅失败",
                    None,
                ))]),
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

        async fn commit(
            &self,
            target: &RefreshTarget,
            _commit: RefreshCommit,
        ) -> Result<crate::RefreshCommitOutcome> {
            if target.feed_id == self.failing_feed_id {
                anyhow::bail!("提交失败");
            }
            Ok(Default::default())
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

        async fn commit(
            &self,
            target: &RefreshTarget,
            _commit: RefreshCommit,
        ) -> Result<crate::RefreshCommitOutcome> {
            self.record(&format!("commit:{}", target.feed_id));
            Ok(crate::RefreshCommitOutcome {
                inserted_count: u64::from(matches!(_commit, RefreshCommit::Updated { .. })),
                superseded: false,
            })
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
                Some(message) => {
                    return Err(rssr_domain::DomainError::Persistence(message.to_string()).into());
                }
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
        assert_eq!(outcome.summary().inserted_count, 0);
        for feed in &outcome.feeds {
            match &feed.result {
                // A storage cause is classified and shown as the fixed text; the detail is logged.
                super::RefreshFeedResult::Failed { kind, message } => {
                    assert_eq!(*kind, crate::ErrorKind::Storage);
                    assert_eq!(message, crate::ErrorKind::Storage.generic_message());
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
                    FeedRefreshSourceOutput::Failed(RefreshFailure::fetch("boom", None)),
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

    const WATCHDOG: std::time::Duration = std::time::Duration::from_secs(10);

    async fn within<T>(what: &str, future: impl std::future::Future<Output = T>) -> T {
        tokio::time::timeout(WATCHDOG, future)
            .await
            .unwrap_or_else(|_| panic!("watchdog: {what} did not finish"))
    }

    /// 同一订阅的第二次刷新要等第一次提交后才开始抓取；其他订阅不受影响（审计 A1）。
    /// 第二次请求直接轮询一次，确认它停在锁上；所有等待都有看门狗，回归成全局锁也不会挂起。
    #[tokio::test]
    async fn refreshes_of_one_feed_run_one_at_a_time_without_blocking_other_feeds() {
        struct GatedSource {
            calls: Mutex<Vec<i64>>,
            first_started: tokio::sync::Notify,
            release_first: tokio::sync::Notify,
        }

        #[async_trait::async_trait]
        impl FeedRefreshSourcePort for GatedSource {
            async fn refresh(&self, target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
                let first = {
                    let mut calls = self.calls.lock().expect("lock calls");
                    calls.push(target.feed_id);
                    calls.len() == 1
                };
                if first {
                    self.first_started.notify_one();
                    self.release_first.notified().await;
                }
                Ok(FeedRefreshSourceOutput::NotModified(RefreshHttpMetadata::default()))
            }
        }

        let source = Arc::new(GatedSource {
            calls: Mutex::new(Vec::new()),
            first_started: tokio::sync::Notify::new(),
            release_first: tokio::sync::Notify::new(),
        });
        let store = Arc::new(StoreStub {
            targets: vec![
                sample_target(1, "https://example.com/1.xml"),
                sample_target(2, "https://example.com/2.xml"),
            ],
            commits: Mutex::new(Vec::new()),
        });
        let service = RefreshService::new(source.clone(), store.clone());
        let first = tokio::spawn({
            let service = service.clone();
            async move { service.refresh_feed(1).await }
        });
        within("first fetch to start", source.first_started.notified()).await;

        let mut second = std::pin::pin!(service.refresh_feed(1));
        let polled =
            std::future::poll_fn(|cx| std::task::Poll::Ready(second.as_mut().poll(cx))).await;
        assert!(polled.is_pending(), "the second refresh of feed 1 must wait for the first");
        within("refresh of another feed", service.refresh_feed(2))
            .await
            .expect("another feed is not blocked");
        assert_eq!(*source.calls.lock().expect("lock calls"), [1, 2]);

        source.release_first.notify_one();
        within("first refresh", first).await.expect("join first").expect("first refresh");
        within("second refresh", second).await.expect("second refresh");
        assert_eq!(*source.calls.lock().expect("lock calls"), [1, 2, 1]);
        let committed = store
            .commits
            .lock()
            .expect("lock commits")
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        assert_eq!(committed, [2, 1, 1]);
    }

    fn update_titled(title: &str) -> FeedRefreshUpdate {
        FeedRefreshUpdate {
            metadata: RefreshHttpMetadata::default(),
            feed: ParsedFeedUpdate {
                title: Some(title.to_string()),
                site_url: None,
                description: None,
                entries: Vec::new(),
            },
        }
    }

    fn committed_titles(store: &StoreStub) -> Vec<String> {
        store
            .commits
            .lock()
            .expect("lock commits")
            .iter()
            .filter_map(|(_, commit)| match commit {
                RefreshCommit::Updated { update } => update.feed.title.clone(),
                _ => None,
            })
            .collect()
    }

    /// 订阅激活后，后台刷新抢先落库了更新的内容：预抓取的旧结果不能再覆盖它（独立评审）。
    /// 失败的刷新没有带来内容，不会让预抓取结果作废。
    #[tokio::test]
    async fn a_prepared_update_never_overwrites_a_later_refresh() {
        for (later_refresh, expected) in [
            (FeedRefreshSourceOutput::Updated(update_titled("V2 newer")), vec!["V2 newer"]),
            (
                FeedRefreshSourceOutput::Failed(RefreshFailure::fetch("offline", None)),
                vec!["V1 prepared"],
            ),
        ] {
            let store = Arc::new(StoreStub {
                targets: vec![sample_target(1, "https://example.com/1.xml")],
                commits: Mutex::new(Vec::new()),
            });
            let service = RefreshService::new(
                Arc::new(SourceStub { outputs: Mutex::new(vec![later_refresh]) }),
                store.clone(),
            );
            let ticket = service.prepared_update_ticket();
            // 激活之后、首刷之前，后台刷新先完成。
            service.refresh_feed(1).await.expect("background refresh");
            let first = service
                .apply_prepared_update(
                    sample_target(1, "https://example.com/1.xml"),
                    update_titled("V1 prepared"),
                    ticket,
                )
                .await;

            assert_eq!(committed_titles(&store), expected);
            assert!(first.is_success());
        }
    }

    /// 第一次写入 `Updated` 时的意外：`Stall` 写入后不再返回（调用方此时被取消，内容已经
    /// 落库）；`FailAfterWrite` 写入后报错（像 SQLite 适配器分步提交时后面一步失败）；
    /// `FailBeforeWrite` 什么也没写就报错。
    #[derive(Clone, Copy)]
    enum Interruption {
        Stall,
        FailAfterWrite,
        FailBeforeWrite,
    }

    struct InterruptingStore {
        inner: StoreStub,
        next_update: Mutex<Option<Interruption>>,
        stalled: tokio::sync::Notify,
    }

    #[async_trait::async_trait]
    impl RefreshStorePort for InterruptingStore {
        async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
            self.inner.list_targets().await
        }

        async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
            self.inner.get_target(feed_id).await
        }

        async fn commit(
            &self,
            target: &RefreshTarget,
            commit: RefreshCommit,
        ) -> Result<crate::RefreshCommitOutcome> {
            let interruption = match commit {
                RefreshCommit::Updated { .. } => self.next_update.lock().expect("lock").take(),
                _ => None,
            };
            match interruption {
                Some(Interruption::FailBeforeWrite) => anyhow::bail!("database is locked"),
                Some(Interruption::FailAfterWrite) => {
                    self.inner.commit(target, commit).await?;
                    anyhow::bail!("failed to store the fetch state")
                }
                Some(Interruption::Stall) => {
                    self.inner.commit(target, commit).await?;
                    self.stalled.notify_one();
                    std::future::pending().await
                }
                None => self.inner.commit(target, commit).await,
            }
        }
    }

    /// 刷新的落库被取消（例如发起它的界面关闭）或中途报错时，无法确定写进去了多少，按已
    /// 写入处理：更早领取序号的预抓取结果不再写入。代价是落库其实什么也没写时，预抓取的
    /// 首刷结果也被放弃，要等下一次刷新（独立评审）。
    #[tokio::test]
    async fn a_refresh_interrupted_while_committing_still_supersedes_an_older_prepared_update() {
        for (interruption, expected) in [
            (Interruption::Stall, vec!["V2 newer"]),
            (Interruption::FailAfterWrite, vec!["V2 newer"]),
            (Interruption::FailBeforeWrite, vec![]),
        ] {
            let store = Arc::new(InterruptingStore {
                inner: StoreStub {
                    targets: vec![sample_target(1, "https://example.com/1.xml")],
                    commits: Mutex::new(Vec::new()),
                },
                next_update: Mutex::new(Some(interruption)),
                stalled: tokio::sync::Notify::new(),
            });
            let service = RefreshService::new(
                Arc::new(SourceStub {
                    outputs: Mutex::new(vec![FeedRefreshSourceOutput::Updated(update_titled(
                        "V2 newer",
                    ))]),
                }),
                store.clone(),
            );
            let ticket = service.prepared_update_ticket();
            let refresh = tokio::spawn({
                let service = service.clone();
                async move { service.refresh_feed(1).await }
            });
            match interruption {
                Interruption::Stall => {
                    within("the commit to be written", store.stalled.notified()).await;
                    refresh.abort();
                    let cancelled = within("the refresh to stop", refresh).await;
                    assert!(cancelled.expect_err("the refresh was cancelled").is_cancelled());
                }
                Interruption::FailAfterWrite | Interruption::FailBeforeWrite => {
                    let finished = within("the refresh to fail", refresh).await;
                    assert!(finished.expect("join refresh").is_err());
                }
            }

            within(
                "the prepared update",
                service.apply_prepared_update(
                    sample_target(1, "https://example.com/1.xml"),
                    update_titled("V1 prepared"),
                    ticket,
                ),
            )
            .await;

            assert_eq!(committed_titles(&store.inner), expected);
        }
    }

    /// `refresh_all` 的目标列表是锁外快照：等锁之后要用存储里最新的校验器发请求；期间被
    /// 删除或重新订阅的订阅不在这一轮刷新（独立评审）。
    #[tokio::test]
    async fn refresh_all_requests_with_the_validators_stored_when_it_gets_the_lock() {
        struct MovingStore {
            listed: Vec<RefreshTarget>,
            current: Vec<RefreshTarget>,
        }

        #[async_trait::async_trait]
        impl RefreshStorePort for MovingStore {
            async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
                Ok(self.listed.clone())
            }

            async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
                Ok(self.current.iter().find(|target| target.feed_id == feed_id).cloned())
            }

            async fn commit(
                &self,
                _target: &RefreshTarget,
                _commit: RefreshCommit,
            ) -> Result<crate::RefreshCommitOutcome> {
                Ok(Default::default())
            }
        }

        struct RecordingSource(Mutex<Vec<(i64, Option<String>)>>);

        #[async_trait::async_trait]
        impl FeedRefreshSourcePort for RecordingSource {
            async fn refresh(&self, target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
                self.0.lock().expect("lock").push((target.feed_id, target.etag.clone()));
                Ok(FeedRefreshSourceOutput::NotModified(RefreshHttpMetadata::default()))
            }
        }

        let listed = vec![
            sample_target(1, "https://example.com/1.xml"),
            sample_target(2, "https://example.com/2.xml"),
            sample_target(3, "https://example.com/3.xml"),
        ];
        let mut current = listed.clone();
        current[0].etag = Some("stored-after-listing".into());
        current[1].generation += 1;
        current.remove(2);
        let source = Arc::new(RecordingSource(Mutex::new(Vec::new())));
        let outcome =
            RefreshService::new(source.clone(), Arc::new(MovingStore { listed, current }))
                .refresh_all(RefreshAllInput { max_concurrency: 1 })
                .await
                .expect("refresh all");

        assert_eq!(
            *source.0.lock().expect("lock"),
            [(1, Some("stored-after-listing".to_string()))]
        );
        assert_eq!(outcome.failures().len(), 2);
    }
}
