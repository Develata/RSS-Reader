use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use anyhow::Context;
use rssr_application::{EntryService, FeedService, ImportExportService, SettingsService};
use rssr_domain::{
    Entry, EntryQuery, EntryRepository, FeedRepository, FeedSummary, NewFeedSubscription,
    UserSettings, normalize_feed_url,
};
use rssr_infra::{
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
    fetch::{BodyAssetLocalizer, FetchClient, FetchRequest, FetchResult},
    opml::OpmlCodec,
    parser::{FeedParser, feed_parser::ParsedEntry},
};
use time::OffsetDateTime;
use tokio::sync::OnceCell;
use url::Url;

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

#[derive(Debug, Clone, Copy, Default)]
pub struct ReaderNavigation {
    pub previous_unread_entry_id: Option<i64>,
    pub next_unread_entry_id: Option<i64>,
    pub previous_feed_entry_id: Option<i64>,
    pub next_feed_entry_id: Option<i64>,
}

pub struct AppServices {
    feed_repository: Arc<SqliteFeedRepository>,
    entry_repository: Arc<SqliteEntryRepository>,
    app_state_repository: Arc<SqliteAppStateRepository>,
    feed_service: FeedService,
    entry_service: EntryService,
    settings_service: SettingsService,
    import_export_service: ImportExportService,
    fetch_client: FetchClient,
    body_asset_localizer: BodyAssetLocalizer,
    parser: FeedParser,
    opml_codec: OpmlCodec,
    auto_refresh_started: AtomicBool,
}

impl AppServices {
    const MAX_BACKGROUND_LOCALIZED_ENTRIES: usize = 5;
    const LOCALIZE_TIMEOUT: Duration = Duration::from_secs(5);
    const AUTO_REFRESH_POLL_INTERVAL: Duration = Duration::from_secs(30);

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

                Ok(Arc::new(Self {
                    feed_service: FeedService::new(feed_repository.clone()),
                    entry_service: EntryService::new(entry_repository.clone()),
                    settings_service: SettingsService::new(settings_repository.clone()),
                    import_export_service: ImportExportService::new(
                        feed_repository.clone(),
                        entry_repository.clone(),
                        settings_repository,
                    ),
                    feed_repository,
                    entry_repository,
                    app_state_repository,
                    fetch_client: FetchClient::new(),
                    body_asset_localizer: BodyAssetLocalizer::new(),
                    parser: FeedParser::new(),
                    opml_codec: OpmlCodec::new(),
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
        let Some(current_entry) = self.entry_service.get_entry(current_entry_id).await? else {
            return Ok(ReaderNavigation::default());
        };

        let global_entries = self.entry_service.list_entries(&EntryQuery::default()).await?;
        let mut navigation = ReaderNavigation::default();

        if let Some(index) = global_entries.iter().position(|entry| entry.id == current_entry_id) {
            navigation.previous_unread_entry_id = global_entries[..index]
                .iter()
                .rev()
                .find(|entry| !entry.is_read)
                .map(|entry| entry.id);
            navigation.next_unread_entry_id = global_entries[index + 1..]
                .iter()
                .find(|entry| !entry.is_read)
                .map(|entry| entry.id);
        }

        let feed_entries = self
            .entry_service
            .list_entries(&EntryQuery {
                feed_id: Some(current_entry.feed_id),
                ..EntryQuery::default()
            })
            .await?;
        if let Some(index) = feed_entries.iter().position(|entry| entry.id == current_entry_id) {
            navigation.previous_feed_entry_id = index
                .checked_sub(1)
                .and_then(|value| feed_entries.get(value))
                .map(|entry| entry.id);
            navigation.next_feed_entry_id = feed_entries.get(index + 1).map(|entry| entry.id);
        }

        Ok(navigation)
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
        self.app_state_repository.load_last_opened_feed_id().await.map_err(Into::into)
    }

    pub async fn remember_last_opened_feed_id(&self, feed_id: i64) -> anyhow::Result<()> {
        self.app_state_repository.save_last_opened_feed_id(Some(feed_id)).await.map_err(Into::into)
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
                        tokio::time::sleep(Self::AUTO_REFRESH_POLL_INTERVAL).await;
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

                tokio::time::sleep(Self::AUTO_REFRESH_POLL_INTERVAL).await;
            }
        });
    }

    pub async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<()> {
        let url = normalize_feed_url(&Url::parse(raw_url).context("订阅 URL 不合法")?);
        let feed = self
            .feed_service
            .add_subscription(&NewFeedSubscription { url, title: None, folder: None })
            .await
            .context("保存订阅失败")?;
        self.refresh_feed(feed.id).await.context("首次刷新订阅失败")?;
        Ok(())
    }

    pub async fn remove_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        self.entry_repository.delete_for_feed(feed_id).await.context("删除订阅文章失败")?;
        self.feed_service.remove_subscription(feed_id).await.context("删除订阅失败")?;
        if self.load_last_opened_feed_id().await? == Some(feed_id) {
            self.app_state_repository
                .save_last_opened_feed_id(None)
                .await
                .context("清理上次打开的订阅记录失败")?;
        }
        Ok(())
    }

    pub async fn refresh_all(&self) -> anyhow::Result<()> {
        let feeds = self.feed_repository.list_feeds().await.context("读取订阅列表失败")?;
        let mut errors = Vec::new();
        for feed in feeds {
            if let Err(error) = self.refresh_feed(feed.id).await {
                tracing::warn!(feed_id = feed.id, error = %error, "刷新订阅失败");
                errors.push(format!("{}: {error}", feed.url));
            }
        }

        if !errors.is_empty() {
            anyhow::bail!("部分订阅刷新失败: {}", errors.join(" | "));
        }

        Ok(())
    }

    pub async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        let feed = self
            .feed_repository
            .get_feed(feed_id)
            .await
            .context("读取订阅失败")?
            .context("订阅不存在")?;

        let response = match self
            .fetch_client
            .fetch(&FetchRequest {
                url: feed.url.to_string(),
                etag: feed.etag.clone(),
                last_modified: feed.last_modified.clone(),
            })
            .await
        {
            Ok(response) => response,
            Err(error) => {
                let message = format!("抓取订阅失败: {error}");
                let _ = self
                    .feed_repository
                    .update_fetch_state(feed.id, None, None, Some(&message), false)
                    .await;
                return Err(error).with_context(|| format!("抓取订阅失败: {}", feed.url));
            }
        };

        match response {
            FetchResult::NotModified(metadata) => {
                self.feed_repository
                    .update_fetch_state(
                        feed.id,
                        metadata.etag.as_deref(),
                        metadata.last_modified.as_deref(),
                        None,
                        true,
                    )
                    .await
                    .context("更新订阅抓取状态失败")?;
            }
            FetchResult::Fetched { body, metadata } => {
                let parsed = match self.parser.parse(&body) {
                    Ok(parsed) => parsed,
                    Err(error) => {
                        let message = format!("解析订阅失败: {error}");
                        let _ = self
                            .feed_repository
                            .update_fetch_state(
                                feed.id,
                                metadata.etag.as_deref(),
                                metadata.last_modified.as_deref(),
                                Some(&message),
                                false,
                            )
                            .await;
                        return Err(error).context("解析订阅失败");
                    }
                };
                let entries_for_localize = parsed.entries.clone();
                if let Err(error) =
                    self.feed_repository.update_feed_metadata(feed.id, &parsed).await
                {
                    let message = format!("更新订阅元数据失败: {error}");
                    let _ = self
                        .feed_repository
                        .update_fetch_state(
                            feed.id,
                            metadata.etag.as_deref(),
                            metadata.last_modified.as_deref(),
                            Some(&message),
                            false,
                        )
                        .await;
                    return Err(error).context("更新订阅元数据失败");
                }
                if let Err(error) =
                    self.entry_repository.upsert_entries(feed.id, &parsed.entries).await
                {
                    let message = format!("写入文章失败: {error}");
                    let _ = self
                        .feed_repository
                        .update_fetch_state(
                            feed.id,
                            metadata.etag.as_deref(),
                            metadata.last_modified.as_deref(),
                            Some(&message),
                            false,
                        )
                        .await;
                    return Err(error).context("写入文章失败");
                }
                self.feed_repository
                    .update_fetch_state(
                        feed.id,
                        metadata.etag.as_deref(),
                        metadata.last_modified.as_deref(),
                        None,
                        true,
                    )
                    .await
                    .context("更新订阅抓取状态失败")?;

                self.spawn_background_image_localization(feed.id, entries_for_localize);
            }
        }

        Ok(())
    }

    fn spawn_background_image_localization(&self, feed_id: i64, entries: Vec<ParsedEntry>) {
        let entry_repository = self.entry_repository.clone();
        let localizer = self.body_asset_localizer.clone();

        tokio::spawn(async move {
            let mut localized_count = 0_usize;

            for entry in entries.into_iter() {
                if localized_count >= Self::MAX_BACKGROUND_LOCALIZED_ENTRIES {
                    break;
                }

                let Some(original_html) = entry.content_html.clone() else {
                    continue;
                };

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
        let package = self.import_export_service.export_config().await?;
        self.opml_codec.encode(&package.feeds)
    }

    pub async fn import_opml(&self, raw: &str) -> anyhow::Result<()> {
        let feeds = self.opml_codec.decode(raw)?;
        let current_feeds = self.feed_repository.list_feeds().await?;
        for feed in feeds {
            let url =
                normalize_feed_url(&Url::parse(&feed.url).context("OPML 中存在无效订阅 URL")?);
            let existed =
                current_feeds.iter().any(|current| normalize_feed_url(&current.url) == url);
            self.feed_service
                .add_subscription(&NewFeedSubscription {
                    url,
                    title: import_field(feed.title, existed),
                    folder: import_field(feed.folder, existed),
                })
                .await?;
        }
        Ok(())
    }

    pub async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<()> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        let raw = self.import_export_service.export_config_json().await?;
        remote.upload_text(&raw).await
    }

    pub async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<bool> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        match remote.download_text().await? {
            Some(raw) => {
                self.import_export_service.import_config_json(&raw).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

fn import_field(value: Option<String>, existed: bool) -> Option<String> {
    if existed { value.or(Some(String::new())) } else { value }
}
