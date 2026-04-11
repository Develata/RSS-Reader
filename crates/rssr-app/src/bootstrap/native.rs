use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use anyhow::Context;
use rssr_application::{
    AddSubscriptionInput, AppCompositionInput, AppUseCases, RefreshAllInput, RefreshAllOutcome,
    RefreshFeedOutcome, RefreshFeedResult, RefreshLocalizedEntry,
};
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use rssr_domain::UserSettings;
use rssr_infra::{
    application_adapters::{
        InfraFeedRefreshSource, InfraOpmlCodec, SqliteAppStateAdapter, SqliteRefreshStore,
    },
    config_sync::webdav::WebDavConfigSync,
    db::{
        app_state_repository::SqliteAppStateRepository,
        entry_repository::{
            LocalizedEntryUpdate, SqliteEntryRepository, compute_entry_content_hash,
        },
        feed_repository::SqliteFeedRepository,
        settings_repository::SqliteSettingsRepository,
        sqlite_native::NativeSqliteBackend,
        storage_backend::StorageBackend,
    },
    fetch::{BodyAssetLocalizer, FetchClient},
    opml::OpmlCodec,
    parser::FeedParser,
};
use time::OffsetDateTime;
use tokio::sync::OnceCell;

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

pub struct AppServices {
    use_cases: AppUseCases,
    image_localization_worker: ImageLocalizationWorker,
    auto_refresh_started: AtomicBool,
}

#[derive(Clone)]
pub(crate) struct AutoRefreshCapability {
    host: Arc<AppServices>,
}

#[derive(Clone)]
pub(crate) struct RefreshCapability {
    host: Arc<AppServices>,
}

#[derive(Clone)]
pub(crate) struct RemoteConfigCapability {
    host: Arc<AppServices>,
}

#[derive(Clone)]
struct ImageLocalizationWorker {
    entry_repository: Arc<SqliteEntryRepository>,
    body_asset_localizer: BodyAssetLocalizer,
}

impl AppServices {
    const AUTO_REFRESH_RETRY_DELAY: Duration = Duration::from_secs(30);
    const REFRESH_ALL_CONCURRENCY: usize = 4;

    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async {
                let native_backend = NativeSqliteBackend::from_default_location()
                    .context("确定本地数据库位置失败")?;
                tracing::info!(
                    backend = native_backend.label(),
                    database = %native_backend.database_label(),
                    "初始化桌面端本地数据库"
                );
                let backend: Box<dyn StorageBackend> = Box::new(native_backend);

                let pool = backend.connect().await.context("连接本地数据库失败")?;
                backend.migrate(&pool).await.context("执行数据库迁移失败")?;

                let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
                let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
                let settings_repository = Arc::new(SqliteSettingsRepository::new(pool.clone()));
                let app_state_repository = Arc::new(SqliteAppStateRepository::new(pool));
                let app_state_adapter =
                    Arc::new(SqliteAppStateAdapter::new(app_state_repository.clone()));
                let use_cases = AppUseCases::compose(AppCompositionInput {
                    feed_repository: feed_repository.clone(),
                    entry_repository: entry_repository.clone(),
                    settings_repository,
                    app_state: app_state_adapter,
                    refresh_source: Arc::new(InfraFeedRefreshSource::new(
                        FetchClient::new(),
                        FeedParser::new(),
                    )),
                    refresh_store: Arc::new(SqliteRefreshStore::new(
                        feed_repository,
                        entry_repository.clone(),
                    )),
                    opml_codec: Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
                });

                Ok(Arc::new(Self {
                    use_cases,
                    image_localization_worker: ImageLocalizationWorker {
                        entry_repository,
                        body_asset_localizer: BodyAssetLocalizer::new(),
                    },
                    auto_refresh_started: AtomicBool::new(false),
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

    pub(crate) fn auto_refresh(self: &Arc<Self>) -> AutoRefreshCapability {
        AutoRefreshCapability { host: Arc::clone(self) }
    }

    pub(crate) fn refresh(self: &Arc<Self>) -> RefreshCapability {
        RefreshCapability { host: Arc::clone(self) }
    }

    pub(crate) fn remote_config(self: &Arc<Self>) -> RemoteConfigCapability {
        RemoteConfigCapability { host: Arc::clone(self) }
    }
}

impl AutoRefreshCapability {
    pub(crate) fn ensure_started(&self) {
        if self.host.auto_refresh_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let host = Arc::clone(&self.host);
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
                    if let Err(error) = host.refresh().refresh_all().await {
                        tracing::warn!(error = %error, "后台自动刷新失败");
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

impl RefreshCapability {
    pub(crate) async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<()> {
        let outcome = self
            .host
            .use_cases
            .subscription_workflow
            .add_subscription_and_refresh(&AddSubscriptionInput {
                url: raw_url.to_string(),
                title: None,
                folder: None,
            })
            .await
            .context("保存订阅失败")?;
        self.handle_refresh_outcome(outcome.refresh).context("首次刷新订阅失败")
    }

    pub(crate) async fn refresh_all(&self) -> anyhow::Result<()> {
        let outcome = self
            .host
            .use_cases
            .refresh_service
            .refresh_all(RefreshAllInput {
                max_concurrency: AppServices::REFRESH_ALL_CONCURRENCY,
            })
            .await?;
        self.handle_refresh_all_outcome(outcome)
    }

    pub(crate) async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        let outcome = self.host.use_cases.refresh_service.refresh_feed(feed_id).await?;
        self.handle_refresh_outcome(outcome)
    }

    fn handle_refresh_all_outcome(&self, outcome: RefreshAllOutcome) -> anyhow::Result<()> {
        let mut errors = Vec::new();

        for feed in outcome.feeds {
            match feed.result {
                RefreshFeedResult::Updated { localization_entries, .. } => {
                    tracing::debug!(feed_id = feed.feed_id, "刷新订阅成功");
                    self.host
                        .image_localization_worker
                        .spawn(feed.feed_id, localization_entries);
                }
                RefreshFeedResult::NotModified => {
                    tracing::debug!(feed_id = feed.feed_id, "订阅未变化");
                }
                RefreshFeedResult::Failed { message } => {
                    tracing::warn!(feed_id = feed.feed_id, error = %message, "刷新订阅失败");
                    errors.push(format!("{}: {message}", feed.url));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!("部分订阅刷新失败: {}", errors.join(" | "))
        }
    }

    fn handle_refresh_outcome(&self, outcome: RefreshFeedOutcome) -> anyhow::Result<()> {
        match outcome.result {
            RefreshFeedResult::Updated { localization_entries, .. } => {
                self.host
                    .image_localization_worker
                    .spawn(outcome.feed_id, localization_entries);
                Ok(())
            }
            RefreshFeedResult::NotModified => Ok(()),
            RefreshFeedResult::Failed { message } => anyhow::bail!("{}: {message}", outcome.url),
        }
    }
}

impl RemoteConfigCapability {
    pub(crate) async fn push(&self, endpoint: &str, remote_path: &str) -> anyhow::Result<()> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        self.host.use_cases.import_export_service.push_remote_config(&remote).await
    }

    pub(crate) async fn pull(&self, endpoint: &str, remote_path: &str) -> anyhow::Result<bool> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        self.host.use_cases.import_export_service.pull_remote_config(&remote).await
    }
}

impl ImageLocalizationWorker {
    const MAX_BACKGROUND_LOCALIZED_ENTRIES: usize = 5;
    const LOCALIZE_TIMEOUT: Duration = Duration::from_secs(5);

    fn spawn(&self, feed_id: i64, entries: Vec<RefreshLocalizedEntry>) {
        let entry_repository = self.entry_repository.clone();
        let localizer = self.body_asset_localizer.clone();

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
                    Self::LOCALIZE_TIMEOUT,
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
                            timeout_secs = Self::LOCALIZE_TIMEOUT.as_secs(),
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
}
