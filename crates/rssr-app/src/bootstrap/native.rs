use std::sync::{
    Arc, PoisonError,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use anyhow::Context;
use rssr_application::{
    AddSubscriptionInput, AddSubscriptionLifecycleInput, AppUseCases, DEFAULT_REFRESH_CONCURRENCY,
    RefreshAllInput, RefreshAllOutcome, RefreshFeedOutcome, RefreshFeedResult,
    RefreshLocalizedEntry, RemoteConfigPullOutcome, RemoteConfigPushOutcome,
    SERIAL_REFRESH_CONCURRENCY,
};
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use rssr_domain::UserSettings;
use rssr_infra::{
    application_adapters::cleanup_pending_entry_content,
    composition::compose_native_sqlite_use_cases,
    config_sync::webdav::WebDavConfigSync,
    db::{
        entry_repository::{
            LocalizedEntryUpdate, SqliteEntryRepository, compute_entry_content_hash,
        },
        sqlite_native::NativeSqliteBackend,
        storage_backend::StorageBackend,
    },
    fetch::BodyAssetLocalizer,
};
use time::OffsetDateTime;
use tokio::sync::OnceCell;

use super::{
    AddSubscriptionOutcome, AutoRefreshPort, HostCapabilities, ReaderAssetLocalizationOutcome,
    ReaderAssetPort, RefreshAllExecutionOutcome, RefreshFeedExecutionOutcome, RefreshPort,
    RemoteConfigPort,
};

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

pub struct AppServices {
    use_cases: AppUseCases,
    image_localization_worker: ImageLocalizationWorker,
    auto_refresh_started: AtomicBool,
    refresh_flight: super::refresh_flight::RefreshFlight,
    /// 启动时按实际生效的 journal 模式确定的刷新并发度。
    ///
    /// 不能假定 WAL 启用成功：`PRAGMA journal_mode=WAL` 在不支持共享内存的文件系统上
    /// 会静默留在回滚日志模式，而那种模式下并发写者只能互相干等、还会阻塞读。
    refresh_concurrency: usize,
}

#[derive(Clone)]
struct AutoRefreshCapability {
    host: Arc<AppServices>,
}

#[derive(Clone)]
struct RefreshCapability {
    host: Arc<AppServices>,
}

#[derive(Clone)]
struct ReaderAssetCapability {
    host: Arc<AppServices>,
}

#[derive(Clone)]
struct RemoteConfigCapability {
    host: Arc<AppServices>,
}

#[derive(Clone)]
struct ImageLocalizationWorker {
    entry_repository: Arc<SqliteEntryRepository>,
    background_asset_localizer: BodyAssetLocalizer,
    reader_asset_localizer: BodyAssetLocalizer,
    background: Arc<BackgroundLocalizationQueue>,
}

/// 后台图片本地化的并发与排队约束（审计 C2）。
struct BackgroundLocalizationQueue {
    slots: tokio::sync::Semaphore,
    pending_feeds: std::sync::Mutex<std::collections::HashSet<i64>>,
}

/// 某个订阅已有排队或进行中的后台本地化；任务结束（包括被取消）时释放。
struct PendingFeed {
    queue: Arc<BackgroundLocalizationQueue>,
    feed_id: i64,
}

impl PendingFeed {
    fn claim(queue: &Arc<BackgroundLocalizationQueue>, feed_id: i64) -> Option<Self> {
        let mut pending = queue.pending_feeds.lock().unwrap_or_else(PoisonError::into_inner);
        pending.insert(feed_id).then(|| Self { queue: Arc::clone(queue), feed_id })
    }
}

impl Drop for PendingFeed {
    fn drop(&mut self) {
        let mut pending = self.queue.pending_feeds.lock().unwrap_or_else(PoisonError::into_inner);
        pending.remove(&self.feed_id);
    }
}

impl AppServices {
    const AUTO_REFRESH_RETRY_DELAY: Duration = Duration::from_secs(30);

    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async {
                let native_backend = NativeSqliteBackend::from_default_location()
                    .context("确定本地数据库位置失败")?;
                tracing::info!(
                    backend = native_backend.label(),
                    database = %native_backend.database_label(),
                    content_database = %native_backend.content_database_label().unwrap_or_else(|_| "<unavailable>".to_string()),
                    "初始化桌面端本地数据库"
                );
                let index_pool = native_backend.connect().await.context("连接本地索引数据库失败")?;
                native_backend.migrate(&index_pool).await.context("执行索引数据库迁移失败")?;
                let content_pool =
                    native_backend.connect_content().await.context("连接本地正文数据库失败")?;
                native_backend
                    .migrate_content(&content_pool)
                    .await
                    .context("执行正文数据库迁移失败")?;

                match cleanup_pending_entry_content(&index_pool, &content_pool).await {
                    Ok(removed_rows) if removed_rows > 0 => {
                        tracing::info!(removed_rows, "已清理待回收正文缓存");
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!(%error, "正文缓存清理失败，将在下次启动重试");
                    }
                }

                // 实测一次 journal 模式再决定并发度：WAL 没启用成功时退回串行，
                // 否则并发写者会互相干等并把前台查询一起卡住。
                let journal_mode = rssr_infra::db::effective_journal_mode(&index_pool)
                    .await
                    .unwrap_or_else(|_| "unknown".to_string());
                let refresh_concurrency = if journal_mode == "wal" {
                    DEFAULT_REFRESH_CONCURRENCY
                } else {
                    tracing::warn!(
                        journal_mode = %journal_mode,
                        "数据库未运行在 WAL 模式（可能位于不支持共享内存的文件系统），                         刷新退回串行以避免写者互相阻塞"
                    );
                    SERIAL_REFRESH_CONCURRENCY
                };
                tracing::info!(journal_mode = %journal_mode, refresh_concurrency, "本地数据库就绪");

                let composition = compose_native_sqlite_use_cases(index_pool, content_pool);

                Ok(Arc::new(Self {
                    use_cases: composition.use_cases,
                    image_localization_worker: ImageLocalizationWorker::new(
                        composition.entry_repository,
                    ),
                    auto_refresh_started: AtomicBool::new(false),
                    refresh_flight: super::refresh_flight::RefreshFlight::default(),
                    refresh_concurrency,
                }))
            })
            .await
            .map(Arc::clone)
    }

    pub fn default_settings() -> UserSettings {
        UserSettings::default()
    }

    pub(crate) fn use_cases(&self) -> AppUseCases {
        self.use_cases.clone()
    }

    pub(crate) fn host_capabilities(self: &Arc<Self>) -> HostCapabilities {
        HostCapabilities {
            auto_refresh: Arc::new(AutoRefreshCapability { host: Arc::clone(self) }),
            refresh: Arc::new(RefreshCapability { host: Arc::clone(self) }),
            reader_assets: Arc::new(ReaderAssetCapability { host: Arc::clone(self) }),
            remote_config: Arc::new(RemoteConfigCapability { host: Arc::clone(self) }),
        }
    }
}

impl AutoRefreshPort for AutoRefreshCapability {
    fn ensure_started(&self) {
        if self.host.auto_refresh_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let host = Arc::clone(&self.host);
        let refresh = RefreshCapability { host: host.clone() };
        tokio::spawn(async move {
            let mut last_refresh_started_at = None;

            loop {
                let settings = match host.use_cases.settings_service.load().await {
                    Ok(settings) => settings,
                    Err(error) => {
                        tracing::warn!(error = %error, "读取自动刷新设置失败，稍后重试");
                        tokio::time::sleep(AppServices::AUTO_REFRESH_RETRY_DELAY).await;
                        continue;
                    }
                };

                let now = OffsetDateTime::now_utc();
                if super::should_trigger_auto_refresh(
                    last_refresh_started_at,
                    settings.refresh_interval_minutes,
                    now,
                ) {
                    tracing::info!(
                        refresh_interval_minutes = settings.refresh_interval_minutes,
                        "触发后台自动刷新全部订阅"
                    );
                    if let Err(error) = refresh.refresh_all().await {
                        tracing::warn!(error = ?error, "后台自动刷新失败");
                    }
                    last_refresh_started_at = Some(now);
                }

                let wait_for = super::auto_refresh_wait_duration(
                    last_refresh_started_at,
                    settings.refresh_interval_minutes,
                    OffsetDateTime::now_utc(),
                );
                tokio::time::sleep(wait_for).await;
            }
        });
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RefreshPort for RefreshCapability {
    async fn add_subscription(
        &self,
        raw_url: &str,
        fallback_site_url: Option<url::Url>,
    ) -> anyhow::Result<AddSubscriptionOutcome> {
        let workflow = &self.host.use_cases.subscription_workflow;
        let prepared = match workflow.prepare_subscription(raw_url).await? {
            rssr_application::PrepareSubscriptionOutcome::Ready(prepared) => {
                prepared.with_fallback_site_url(fallback_site_url)
            }
            rssr_application::PrepareSubscriptionOutcome::NeedsSelection {
                page_url,
                candidates,
            } => {
                return Ok(AddSubscriptionOutcome::NeedsSelection { page_url, candidates });
            }
        };
        let outcome = workflow
            .add_prepared_subscription(
                AddSubscriptionLifecycleInput {
                    subscription: AddSubscriptionInput {
                        url: raw_url.to_string(),
                        title: None,
                        folder: None,
                    },
                    refresh_after_add: true,
                },
                prepared,
            )
            .await
            .context("保存订阅失败")?;
        let refresh = outcome.first_refresh.expect("refresh_after_add produces refresh outcome");
        let outcome = self.handle_refresh_feed_outcome(refresh);
        match outcome.failure_message {
            Some(message) => Ok(AddSubscriptionOutcome::SavedRefreshFailed { message }),
            None => Ok(AddSubscriptionOutcome::SavedAndRefreshed),
        }
    }

    async fn refresh_all(&self) -> anyhow::Result<RefreshAllExecutionOutcome> {
        self.host
            .refresh_flight
            .run(async {
                let outcome = self
                    .host
                    .use_cases
                    .refresh_service
                    .refresh_all(RefreshAllInput { max_concurrency: self.host.refresh_concurrency })
                    .await?;
                self.handle_refresh_all_outcome(outcome)
            })
            .await
    }

    async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<RefreshFeedExecutionOutcome> {
        let outcome = self.host.use_cases.refresh_service.refresh_feed(feed_id).await?;
        Ok(self.handle_refresh_feed_outcome(outcome))
    }
}

impl RefreshCapability {
    fn handle_refresh_all_outcome(
        &self,
        outcome: RefreshAllOutcome,
    ) -> anyhow::Result<RefreshAllExecutionOutcome> {
        let summary = outcome.summary();
        let failure_lines = summary.joined_failure_lines();

        for feed in outcome.feeds {
            match feed.result {
                RefreshFeedResult::Updated { localization_entries, .. } => {
                    tracing::debug!(feed_id = feed.feed_id, "刷新订阅成功");
                    self.host.image_localization_worker.spawn(feed.feed_id, localization_entries);
                }
                RefreshFeedResult::NotModified => {
                    tracing::debug!(feed_id = feed.feed_id, "订阅未变化");
                }
                RefreshFeedResult::Failed { message } => {
                    tracing::warn!(feed_id = feed.feed_id, error = %message, "刷新订阅失败");
                }
            }
        }

        Ok(RefreshAllExecutionOutcome {
            failure_message: failure_lines,
            inserted_count: summary.inserted_count,
            total_count: summary.total_count,
            failed_count: summary.failed_count,
        })
    }

    fn handle_refresh_feed_outcome(
        &self,
        outcome: RefreshFeedOutcome,
    ) -> RefreshFeedExecutionOutcome {
        let inserted_count = outcome.inserted_count();
        let failure_message = outcome.failure_line();
        match outcome.result {
            RefreshFeedResult::Updated { localization_entries, .. } => {
                self.host.image_localization_worker.spawn(outcome.feed_id, localization_entries);
                RefreshFeedExecutionOutcome { inserted_count, failure_message: None }
            }
            RefreshFeedResult::NotModified => {
                RefreshFeedExecutionOutcome { inserted_count, failure_message: None }
            }
            RefreshFeedResult::Failed { message } => {
                tracing::warn!(feed_id = outcome.feed_id, error = %message, "刷新订阅失败");
                RefreshFeedExecutionOutcome {
                    inserted_count,
                    failure_message: Some(
                        failure_message.unwrap_or_else(|| "刷新订阅失败".to_string()),
                    ),
                }
            }
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl ReaderAssetPort for ReaderAssetCapability {
    async fn localize_entry_assets(
        &self,
        entry_id: i64,
    ) -> anyhow::Result<ReaderAssetLocalizationOutcome> {
        let localized =
            self.host.image_localization_worker.localize_entry_on_demand(entry_id).await?;
        Ok(ReaderAssetLocalizationOutcome { localized })
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigPort for RemoteConfigCapability {
    async fn push(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<RemoteConfigPushOutcome> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        self.host.use_cases.import_export_service.push_remote_config(&remote).await
    }

    async fn pull(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<RemoteConfigPullOutcome> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        self.host.use_cases.import_export_service.pull_remote_config(&remote).await
    }
}

impl ImageLocalizationWorker {
    /// 每次刷新为一个订阅最多尝试本地化的条目数，计数规则见 [`localize_feed_entries`]（审计 C2）。
    const MAX_BACKGROUND_ATTEMPTS: usize = 5;
    /// 一个订阅一轮后台本地化的总时长上限。
    const BACKGROUND_BUDGET: Duration = Duration::from_secs(180);
    /// 同时进行后台本地化的订阅数。
    const MAX_CONCURRENT_FEEDS: usize = 2;

    fn new(entry_repository: Arc<SqliteEntryRepository>) -> Self {
        Self {
            entry_repository,
            background_asset_localizer: BodyAssetLocalizer::new(),
            reader_asset_localizer: BodyAssetLocalizer::for_reader_entry(),
            background: Arc::new(BackgroundLocalizationQueue {
                slots: tokio::sync::Semaphore::new(Self::MAX_CONCURRENT_FEEDS),
                pending_feeds: Default::default(),
            }),
        }
    }

    fn spawn(&self, feed_id: i64, entries: Vec<RefreshLocalizedEntry>) {
        // 同一订阅已有排队或进行中的后台本地化时不再追加：多轮自动刷新不会叠加任务与正文副本，
        // 这次没轮到的条目下次刷新再处理。先到的列表即使过时也只会少做、不会写错：写回按源正文
        // 哈希与 generation 校验。
        let Some(pending) = PendingFeed::claim(&self.background, feed_id) else {
            return;
        };
        let entry_repository = self.entry_repository.clone();
        let localizer = self.background_asset_localizer.clone();
        let background = self.background.clone();

        tokio::spawn(async move {
            let _pending = pending;
            let Ok(_slot) = background.slots.acquire().await else {
                return;
            };
            let round = localize_feed_entries(
                &entry_repository,
                &localizer,
                feed_id,
                entries,
                Self::MAX_BACKGROUND_ATTEMPTS,
            );
            if tokio::time::timeout(Self::BACKGROUND_BUDGET, round).await.is_err() {
                tracing::warn!(
                    feed_id,
                    budget_secs = Self::BACKGROUND_BUDGET.as_secs(),
                    "后台正文图片本地化达到本轮时长上限，剩余文章留到下次刷新"
                );
            }
        });
    }

    async fn localize_entry_on_demand(&self, entry_id: i64) -> anyhow::Result<bool> {
        // Capture the feed generation before starting network work. If deletion/re-add happens
        // while images are being localized, the final write is rejected as stale.
        let Some((feed_id, generation)) = self
            .entry_repository
            .active_entry_generation(entry_id)
            .await
            .context("读取当前文章所属订阅 generation 失败")?
        else {
            return Ok(false);
        };
        let Some(entry) =
            self.entry_repository.get_entry(entry_id).await.context("读取当前文章失败")?
        else {
            return Ok(false);
        };
        if entry.feed_id != feed_id {
            return Ok(false);
        }

        let Some(original_html) = entry.content_html.clone() else {
            return Ok(false);
        };

        // 写回比较的是读到的存储哈希本身（可能是旧版本留下的 NULL），不能用重新计算的值代替，
        // 否则 NULL 行永远写不回（独立评审）。
        let expected_content_hash = entry.content_hash.clone();

        let localizer = self.reader_asset_localizer.clone();
        let localized_html = match tokio::time::timeout(
            localization_timeout(&localizer),
            localizer.localize_html_images_reporting(&original_html, entry.url.as_ref()),
        )
        .await
        {
            // 一张图都没换成本地副本时不写回：规范化阅读页显示时会做，写回只会让这篇文章被
            // 当成“已本地化”而不再重试（审计 C1 复审）。
            Ok(outcome) if outcome.localized_images > 0 => outcome.html,
            Ok(_) => return Ok(false),
            Err(_) => {
                tracing::warn!(
                    entry_id,
                    feed_id = entry.feed_id,
                    entry_url = ?entry.url,
                    reason = "timeout",
                    timeout_secs = localization_timeout(&localizer).as_secs(),
                    "当前阅读文章正文图片按需本地化超时，保留原始 HTML"
                );
                return Ok(false);
            }
        };

        let updated = self
            .entry_repository
            .update_localized_html_if_hash_matches_for_generation(
                feed_id,
                generation,
                &LocalizedEntryUpdate {
                    dedup_key: &entry.dedup_key,
                    expected_content_hash: expected_content_hash.as_deref(),
                    base_html: &original_html,
                    localized_html: &localized_html,
                },
            )
            .await
            .context("写回按需本地化后的正文失败")?;

        Ok(updated)
    }
}

/// 依次尝试本地化一个订阅的文章，返回计入预算的尝试篇数（审计 C2）。
///
/// 为一篇文章发起过图片请求就计一次尝试，不论成败或超时，达到 `max_attempts` 即停。不计数的：
/// 正文已是这份源正文的派生版本（源未变的刷新会保留它，见审计 C1）而跳过的，以及无图、正文超出
/// 预算等根本没有发请求的。图片持续失败的文章每轮仍会占用尝试次数，直到源正文变化。
async fn localize_feed_entries(
    entry_repository: &SqliteEntryRepository,
    localizer: &BodyAssetLocalizer,
    feed_id: i64,
    entries: Vec<RefreshLocalizedEntry>,
    max_attempts: usize,
) -> usize {
    let mut attempts = 0_usize;
    for entry in entries {
        if attempts >= max_attempts {
            break;
        }
        let original_html = entry.content_html;
        let Some(expected_content_hash) = compute_entry_content_hash(
            Some(&original_html),
            entry.content_text.as_deref(),
            Some(&entry.title),
            entry.url.as_ref(),
        ) else {
            continue;
        };
        match entry_repository
            .has_derived_body(feed_id, &entry.dedup_key, &expected_content_hash, &original_html)
            .await
        {
            Ok(true) => continue,
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(
                    feed_id,
                    dedup_key = %entry.dedup_key,
                    error = %error,
                    "读取已存正文失败，跳过后台图片本地化"
                );
                continue;
            }
        }

        let localized_html = match tokio::time::timeout(
            localization_timeout(localizer),
            localizer.localize_html_images_reporting(&original_html, entry.url.as_ref()),
        )
        .await
        {
            Ok(outcome) => {
                if outcome.requested_images {
                    attempts += 1;
                }
                // 没有图片换成本地副本就不写回，见 `localize_entry_on_demand`。
                if outcome.localized_images == 0 {
                    continue;
                }
                outcome.html
            }
            Err(_) => {
                // 超时只可能发生在发出图片请求之后。
                attempts += 1;
                tracing::warn!(
                    feed_id,
                    entry_url = ?entry.url,
                    reason = "timeout",
                    timeout_secs = localization_timeout(localizer).as_secs(),
                    "后台正文图片本地化超时，跳过当前文章"
                );
                continue;
            }
        };

        let update = LocalizedEntryUpdate {
            dedup_key: &entry.dedup_key,
            expected_content_hash: Some(&expected_content_hash),
            base_html: &original_html,
            localized_html: &localized_html,
        };
        match entry_repository
            .update_localized_html_if_hash_matches_for_generation(
                feed_id,
                entry.generation,
                &update,
            )
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                tracing::debug!(
                    feed_id,
                    dedup_key = %entry.dedup_key,
                    "跳过后台正文图片本地化写回：文章内容已被更新"
                );
            }
            Err(error) => {
                tracing::warn!(
                    feed_id,
                    dedup_key = %entry.dedup_key,
                    error = %error,
                    "写回后台本地化后的正文失败"
                );
            }
        }
    }
    attempts
}

fn localization_timeout(localizer: &BodyAssetLocalizer) -> Duration {
    Duration::from_secs(
        localizer.image_request_timeout().as_secs() * localizer.max_images_per_entry() as u64 + 5,
    )
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
        thread,
    };

    use rssr_domain::{FeedRepository, NewFeedSubscription};
    use rssr_infra::{
        db::{
            entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository,
            migrate, sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
        },
        parser::feed_parser::ParsedEntry,
    };
    use url::Url;

    use super::{BodyAssetLocalizer, ImageLocalizationWorker, localize_feed_entries};

    #[tokio::test]
    async fn localize_entry_on_demand_rewrites_current_entry_html() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local image server");
        let addr = listener.local_addr().expect("listener addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept image request");
            let mut buf = [0_u8; 4096];
            let _ = stream.read(&mut buf).expect("read request");
            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Connection: close\r\n",
                "Content-Type: image/png\r\n",
                "Content-Length: 4\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).expect("write headers");
            stream.write_all(&[1_u8, 2_u8, 3_u8, 4_u8]).expect("write image bytes");
            stream.flush().expect("flush response");
        });

        let backend = NativeSqliteBackend::new("sqlite::memory:");
        let pool = backend.connect().await.expect("connect sqlite memory");
        migrate(&pool).await.expect("migrate sqlite");

        let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
        let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
        let feed = feed_repository
            .upsert_subscription(&NewFeedSubscription {
                site_url: None,
                url: Url::parse("https://example.com/feed.xml").expect("feed url"),
                title: Some("Example".to_string()),
                folder: None,
            })
            .await
            .expect("create feed");

        let article_url = Url::parse(&format!("http://{addr}/article")).expect("article url");
        entry_repository
            .upsert_entries(
                feed.id,
                &[ParsedEntry {
                    external_id: "entry-1".to_string(),
                    dedup_key: "entry-1".to_string(),
                    url: Some(article_url.clone()),
                    title: "Entry".to_string(),
                    author: None,
                    summary: Some("summary".to_string()),
                    content_html: Some(r#"<p><img src="/hero.png" alt="hero"></p>"#.to_string()),
                    content_text: Some("summary".to_string()),
                    published_at: None,
                    updated_at_source: None,
                }],
            )
            .await
            .expect("insert entry");

        let worker = ImageLocalizationWorker::new(entry_repository.clone());

        let updated = worker.localize_entry_on_demand(1).await.expect("localize current entry");
        assert!(updated, "expected current reader entry to be rewritten");

        let entry = entry_repository
            .get_entry(1)
            .await
            .expect("load localized entry")
            .expect("entry exists");
        let html = entry.content_html.expect("localized html");
        assert!(html.contains("data:image/png;base64,"));
        assert!(!html.contains("/hero.png"));

        server.join().expect("join local image server");
    }

    /// 后台本地化按尝试计数（审计 C2）：图片全部失败（例如防盗链 403）时，一轮最多为
    /// `max_attempts` 篇发请求，而不是把订阅里的每一篇都试一遍；排在前面的无图文章不发请求，
    /// 也不占用尝试次数。
    #[tokio::test]
    async fn background_localization_stops_after_its_attempt_budget() {
        let (addr, requests, server) = forbidden_image_server().await;

        let pool = NativeSqliteBackend::new("sqlite::memory:").connect().await.expect("connect");
        migrate(&pool).await.expect("migrate sqlite");
        let entry_repository = SqliteEntryRepository::new(pool);
        let article = Url::parse(&format!("http://{addr}/article")).expect("article url");
        let entries = (0..13)
            .map(|index| rssr_application::RefreshLocalizedEntry {
                generation: 0,
                dedup_key: format!("entry-{index}"),
                url: Some(article.clone()),
                title: format!("Entry {index}"),
                content_html: if index < 5 {
                    format!("<p>Text only {index}</p>")
                } else {
                    format!(r#"<p><img src="http://{addr}/image-{index}.png"></p>"#)
                },
                content_text: None,
            })
            .collect();

        let attempts =
            localize_feed_entries(&entry_repository, &BodyAssetLocalizer::new(), 1, entries, 3)
                .await;
        server.abort();

        assert_eq!(attempts, 3);
        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    /// 图片全部失败时，正文只是被规范化（相对地址补全等），不写回：否则这篇文章会被当成已经
    /// 本地化，之后的刷新不再重试（独立评审）。
    #[tokio::test]
    async fn failed_background_localization_leaves_the_source_body_for_a_retry() {
        let (addr, requests, server) = forbidden_image_server().await;
        let pool = NativeSqliteBackend::new("sqlite::memory:").connect().await.expect("connect");
        migrate(&pool).await.expect("migrate sqlite");
        let feed_repository = SqliteFeedRepository::new(pool.clone());
        let entry_repository = SqliteEntryRepository::new(pool);
        let feed = feed_repository
            .upsert_subscription(&NewFeedSubscription {
                site_url: None,
                url: Url::parse("https://example.com/feed.xml").expect("feed url"),
                title: None,
                folder: None,
            })
            .await
            .expect("create feed");
        let article = Url::parse(&format!("http://{addr}/posts/1")).expect("article url");
        let source_html = r#"<p><img src="/image.png"></p>"#;
        entry_repository
            .upsert_entries(
                feed.id,
                &[ParsedEntry {
                    external_id: "entry-1".to_string(),
                    dedup_key: "entry-1".to_string(),
                    url: Some(article.clone()),
                    title: "Entry".to_string(),
                    author: None,
                    summary: None,
                    content_html: Some(source_html.to_string()),
                    content_text: None,
                    published_at: None,
                    updated_at_source: None,
                }],
            )
            .await
            .expect("insert entry");
        let (_, generation) =
            entry_repository.active_entry_generation(1).await.expect("generation").expect("active");
        let entry = rssr_application::RefreshLocalizedEntry {
            generation,
            dedup_key: "entry-1".to_string(),
            url: Some(article.clone()),
            title: "Entry".to_string(),
            content_html: source_html.to_string(),
            content_text: None,
        };

        let attempts = localize_feed_entries(
            &entry_repository,
            &BodyAssetLocalizer::new(),
            feed.id,
            vec![entry],
            5,
        )
        .await;
        server.abort();

        assert_eq!((attempts, requests.load(std::sync::atomic::Ordering::SeqCst)), (1, 1));
        let stored = entry_repository.get_entry(1).await.expect("load").expect("entry");
        assert_eq!(stored.content_html.as_deref(), Some(source_html));
    }

    /// 本地图片服务器：对每个请求都回 403，并记下请求数。
    async fn forbidden_image_server()
    -> (std::net::SocketAddr, Arc<std::sync::atomic::AtomicUsize>, tokio::task::JoinHandle<()>)
    {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("listener addr");
        let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let server = tokio::spawn({
            let requests = Arc::clone(&requests);
            async move {
                loop {
                    let Ok((mut stream, _)) = listener.accept().await else { return };
                    requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let mut buf = [0_u8; 4096];
                    let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
                    let _ = tokio::io::AsyncWriteExt::write_all(
                        &mut stream,
                        b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                    )
                    .await;
                }
            }
        });
        (addr, requests, server)
    }
}
