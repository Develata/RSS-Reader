use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use anyhow::Context;
use rssr_application::{
    AddSubscriptionInput, AppStateService, EntryService, FeedService, ImportExportService,
    RefreshAllInput, RefreshAllOutcome, RefreshFeedOutcome, RefreshFeedResult,
    RefreshLocalizedEntry, RefreshService, RemoveSubscriptionInput, SettingsService,
    SubscriptionWorkflow,
};
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use rssr_domain::{EntriesWorkspaceState, Entry, EntryQuery, FeedSummary, UserSettings};
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
    entry_repository: Arc<SqliteEntryRepository>,
    app_state_service: AppStateService,
    feed_service: FeedService,
    entry_service: EntryService,
    settings_service: SettingsService,
    refresh_service: RefreshService,
    subscription_workflow: SubscriptionWorkflow,
    import_export_service: ImportExportService,
    body_asset_localizer: BodyAssetLocalizer,
    auto_refresh_started: AtomicBool,
}

impl AppServices {
    const MAX_BACKGROUND_LOCALIZED_ENTRIES: usize = 5;
    const LOCALIZE_TIMEOUT: Duration = Duration::from_secs(5);
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
                let feed_service =
                    FeedService::new(feed_repository.clone(), entry_repository.clone());
                let refresh_service = RefreshService::new(
                    Arc::new(InfraFeedRefreshSource::new(FetchClient::new(), FeedParser::new())),
                    Arc::new(SqliteRefreshStore::new(
                        feed_repository.clone(),
                        entry_repository.clone(),
                    )),
                );

                Ok(Arc::new(Self {
                    feed_service: feed_service.clone(),
                    entry_service: EntryService::new(entry_repository.clone()),
                    settings_service: SettingsService::new(settings_repository.clone()),
                    refresh_service: refresh_service.clone(),
                    subscription_workflow: SubscriptionWorkflow::new(
                        feed_service,
                        refresh_service,
                        app_state_adapter.clone(),
                    ),
                    import_export_service: ImportExportService::new_with_feed_removal_cleanup(
                        feed_repository.clone(),
                        entry_repository.clone(),
                        settings_repository,
                        Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
                        app_state_adapter,
                    ),
                    entry_repository,
                    app_state_service: AppStateService::new(app_state_repository),
                    body_asset_localizer: BodyAssetLocalizer::new(),
                    auto_refresh_started: AtomicBool::new(false),
                }))
            })
            .await
            .map(Arc::clone)
    }

    pub fn default_settings() -> UserSettings {
        UserSettings::default()
    }

    pub async fn list_feeds(&self) -> anyhow::Result<Vec<FeedSummary>> {
        self.feed_service.list_feeds().await
    }

    pub async fn list_entries(
        &self,
        query: &EntryQuery,
    ) -> anyhow::Result<Vec<rssr_domain::EntrySummary>> {
        self.entry_service.list_entries(query).await
    }

    pub async fn get_entry(&self, entry_id: i64) -> anyhow::Result<Option<Entry>> {
        self.entry_service.get_entry(entry_id).await
    }

    pub async fn reader_navigation(
        &self,
        current_entry_id: i64,
    ) -> anyhow::Result<ReaderNavigation> {
        self.entry_service.reader_navigation(current_entry_id).await
    }

    pub async fn set_read(&self, entry_id: i64, is_read: bool) -> anyhow::Result<()> {
        self.entry_service.set_read(entry_id, is_read).await
    }

    pub async fn set_starred(&self, entry_id: i64, is_starred: bool) -> anyhow::Result<()> {
        self.entry_service.set_starred(entry_id, is_starred).await
    }

    pub async fn load_settings(&self) -> anyhow::Result<UserSettings> {
        self.settings_service.load().await
    }

    pub async fn save_settings(&self, settings: &UserSettings) -> anyhow::Result<()> {
        self.settings_service.save(settings).await
    }

    pub async fn load_last_opened_feed_id(&self) -> anyhow::Result<Option<i64>> {
        self.app_state_service.load_last_opened_feed_id().await
    }

    pub async fn remember_last_opened_feed_id(&self, feed_id: i64) -> anyhow::Result<()> {
        self.app_state_service.save_last_opened_feed_id(Some(feed_id)).await
    }

    pub async fn load_entries_workspace_state(&self) -> anyhow::Result<EntriesWorkspaceState> {
        self.app_state_service.load_entries_workspace().await
    }

    pub async fn save_entries_workspace_state(
        &self,
        entries_workspace: EntriesWorkspaceState,
    ) -> anyhow::Result<()> {
        self.app_state_service.save_entries_workspace(entries_workspace).await
    }

    pub fn ensure_auto_refresh_started(self: &Arc<Self>) {
        if self.auto_refresh_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let services = Arc::clone(self);
        tokio::spawn(async move {
            let mut last_refresh_started_at = None;

            loop {
                let settings = match services.load_settings().await {
                    Ok(settings) => settings,
                    Err(error) => {
                        tracing::warn!(error = %error, "读取自动刷新设置失败，稍后重试");
                        tokio::time::sleep(Self::AUTO_REFRESH_RETRY_DELAY).await;
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
                    if let Err(error) = services.refresh_all().await {
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

    pub async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<()> {
        let outcome = self
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

    pub async fn remove_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        self.subscription_workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id, purge_entries: true })
            .await
            .context("删除订阅失败")
    }

    pub async fn refresh_all(self: &Arc<Self>) -> anyhow::Result<()> {
        let outcome = self
            .refresh_service
            .refresh_all(RefreshAllInput { max_concurrency: Self::REFRESH_ALL_CONCURRENCY })
            .await?;
        self.handle_refresh_all_outcome(outcome)
    }

    pub async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        let outcome = self.refresh_service.refresh_feed(feed_id).await?;
        self.handle_refresh_outcome(outcome)
    }

    fn spawn_background_image_localization(
        &self,
        feed_id: i64,
        entries: Vec<RefreshLocalizedEntry>,
    ) {
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

    pub async fn export_config_json(&self) -> anyhow::Result<String> {
        self.import_export_service.export_config_json().await
    }

    pub async fn import_config_json(&self, raw: &str) -> anyhow::Result<()> {
        self.import_export_service.import_config_json(raw).await
    }

    pub async fn export_opml(&self) -> anyhow::Result<String> {
        self.import_export_service.export_opml().await
    }

    pub async fn import_opml(&self, raw: &str) -> anyhow::Result<()> {
        self.import_export_service.import_opml(raw).await
    }

    pub async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<()> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        self.import_export_service.push_remote_config(&remote).await
    }

    pub async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<bool> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        self.import_export_service.pull_remote_config(&remote).await
    }

    fn handle_refresh_all_outcome(&self, outcome: RefreshAllOutcome) -> anyhow::Result<()> {
        let mut errors = Vec::new();

        for feed in outcome.feeds {
            match feed.result {
                RefreshFeedResult::Updated { localization_entries, .. } => {
                    tracing::debug!(feed_id = feed.feed_id, "刷新订阅成功");
                    self.spawn_background_image_localization(feed.feed_id, localization_entries);
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
                self.spawn_background_image_localization(outcome.feed_id, localization_entries);
                Ok(())
            }
            RefreshFeedResult::NotModified => Ok(()),
            RefreshFeedResult::Failed { message } => anyhow::bail!("{}: {message}", outcome.url),
        }
    }
}
