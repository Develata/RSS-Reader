use std::sync::{
    Arc,
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
                    image_localization_worker: ImageLocalizationWorker {
                        entry_repository: composition.entry_repository,
                        background_asset_localizer: BodyAssetLocalizer::new(),
                        reader_asset_localizer: BodyAssetLocalizer::for_reader_entry(),
                    },
                    auto_refresh_started: AtomicBool::new(false),
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
    async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<AddSubscriptionOutcome> {
        let outcome = self
            .host
            .use_cases
            .subscription_workflow
            .add_subscription_lifecycle(AddSubscriptionLifecycleInput {
                subscription: AddSubscriptionInput {
                    url: raw_url.to_string(),
                    title: None,
                    folder: None,
                },
                refresh_after_add: true,
            })
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
        let outcome = self
            .host
            .use_cases
            .refresh_service
            .refresh_all(RefreshAllInput { max_concurrency: self.host.refresh_concurrency })
            .await?;
        self.handle_refresh_all_outcome(outcome)
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
        let failure_lines = outcome.joined_failure_lines();

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

        Ok(RefreshAllExecutionOutcome { failure_message: failure_lines })
    }

    fn handle_refresh_feed_outcome(
        &self,
        outcome: RefreshFeedOutcome,
    ) -> RefreshFeedExecutionOutcome {
        let failure_message = outcome.failure_line();
        match outcome.result {
            RefreshFeedResult::Updated { localization_entries, .. } => {
                self.host.image_localization_worker.spawn(outcome.feed_id, localization_entries);
                RefreshFeedExecutionOutcome { failure_message: None }
            }
            RefreshFeedResult::NotModified => RefreshFeedExecutionOutcome { failure_message: None },
            RefreshFeedResult::Failed { message } => {
                tracing::warn!(feed_id = outcome.feed_id, error = %message, "刷新订阅失败");
                RefreshFeedExecutionOutcome {
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
    const MAX_BACKGROUND_LOCALIZED_ENTRIES: usize = 5;

    fn spawn(&self, feed_id: i64, entries: Vec<RefreshLocalizedEntry>) {
        let entry_repository = self.entry_repository.clone();
        let localizer = self.background_asset_localizer.clone();

        tokio::spawn(async move {
            let mut localized_count = 0_usize;

            for entry in entries.into_iter() {
                if localized_count >= Self::MAX_BACKGROUND_LOCALIZED_ENTRIES {
                    break;
                }

                let original_html = entry.content_html.clone();

                let Some(expected_content_hash) = compute_entry_content_hash(
                    Some(&original_html),
                    entry.content_text.as_deref(),
                    Some(&entry.title),
                ) else {
                    continue;
                };

                let localized_html = match tokio::time::timeout(
                    localization_timeout(&localizer),
                    localizer.localize_html_images(&original_html, entry.url.as_ref()),
                )
                .await
                {
                    Ok(Ok(localized_html)) if localized_html != original_html => localized_html,
                    Ok(Ok(_)) => continue,
                    Ok(Err(error)) => {
                        tracing::warn!(
                            feed_id,
                            entry_url = ?entry.url,
                            error = %error,
                            "后台正文图片本地化失败，保留原始 HTML"
                        );
                        continue;
                    }
                    Err(_) => {
                        tracing::warn!(
                            feed_id,
                            entry_url = ?entry.url,
                            reason = "timeout",
                            timeout_secs = localization_timeout(&localizer).as_secs(),
                            "后台正文图片本地化超时，跳过当前文章"
                        );
                        continue;
                    }
                };

                let Some(localized_content_hash) = compute_entry_content_hash(
                    Some(&localized_html),
                    entry.content_text.as_deref(),
                    Some(&entry.title),
                ) else {
                    continue;
                };

                let update = LocalizedEntryUpdate {
                    dedup_key: &entry.dedup_key,
                    expected_content_hash: &expected_content_hash,
                    localized_html: &localized_html,
                    localized_content_hash: &localized_content_hash,
                };

                match entry_repository.update_localized_html_if_hash_matches(feed_id, &update).await
                {
                    Ok(true) => {
                        localized_count += 1;
                    }
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
        });
    }

    async fn localize_entry_on_demand(&self, entry_id: i64) -> anyhow::Result<bool> {
        let Some(entry) =
            self.entry_repository.get_entry(entry_id).await.context("读取当前文章失败")?
        else {
            return Ok(false);
        };

        let Some(original_html) = entry.content_html.clone() else {
            return Ok(false);
        };

        let Some(expected_content_hash) = entry.content_hash.clone().or_else(|| {
            compute_entry_content_hash(
                Some(&original_html),
                entry.content_text.as_deref(),
                Some(&entry.title),
            )
        }) else {
            return Ok(false);
        };

        let localizer = self.reader_asset_localizer.clone();
        let localized_html = match tokio::time::timeout(
            localization_timeout(&localizer),
            localizer.localize_html_images(&original_html, entry.url.as_ref()),
        )
        .await
        {
            Ok(Ok(localized_html)) if localized_html != original_html => localized_html,
            Ok(Ok(_)) => return Ok(false),
            Ok(Err(error)) => {
                tracing::warn!(
                    entry_id,
                    feed_id = entry.feed_id,
                    entry_url = ?entry.url,
                    error = %error,
                    "当前阅读文章正文图片按需本地化失败，保留原始 HTML"
                );
                return Ok(false);
            }
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

        let Some(localized_content_hash) = compute_entry_content_hash(
            Some(&localized_html),
            entry.content_text.as_deref(),
            Some(&entry.title),
        ) else {
            return Ok(false);
        };

        let updated = self
            .entry_repository
            .update_localized_html_if_hash_matches(
                entry.feed_id,
                &LocalizedEntryUpdate {
                    dedup_key: &entry.dedup_key,
                    expected_content_hash: &expected_content_hash,
                    localized_html: &localized_html,
                    localized_content_hash: &localized_content_hash,
                },
            )
            .await
            .context("写回按需本地化后的正文失败")?;

        Ok(updated)
    }
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

    use super::{BodyAssetLocalizer, ImageLocalizationWorker};

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

        let worker = ImageLocalizationWorker {
            entry_repository: entry_repository.clone(),
            background_asset_localizer: BodyAssetLocalizer::new(),
            reader_asset_localizer: BodyAssetLocalizer::for_reader_entry(),
        };

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
}
