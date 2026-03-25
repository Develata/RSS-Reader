use std::sync::Arc;

use anyhow::Context;
use rssr_application::{
    entry_service::EntryService, feed_service::FeedService, settings_service::SettingsService,
};
use rssr_domain::{Entry, EntryQuery, FeedRepository, FeedSummary, NewFeedSubscription};
use rssr_infra::{
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository,
        settings_repository::SqliteSettingsRepository, sqlite_native::NativeSqliteBackend,
        sqlite_web::WebSqliteBackend, storage_backend::StorageBackend,
    },
    fetch::{FetchClient, FetchRequest, FetchResult},
    parser::FeedParser,
};
use tokio::sync::OnceCell;
use url::Url;

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

pub struct AppServices {
    feed_repository: Arc<SqliteFeedRepository>,
    entry_repository: Arc<SqliteEntryRepository>,
    feed_service: FeedService,
    entry_service: EntryService,
    #[allow(dead_code)]
    settings_service: SettingsService,
    fetch_client: FetchClient,
    parser: FeedParser,
}

impl AppServices {
    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async {
                let backend: Box<dyn StorageBackend> = if cfg!(target_arch = "wasm32") {
                    Box::new(WebSqliteBackend::default())
                } else {
                    Box::new(NativeSqliteBackend::new("sqlite:rss-reader.db?mode=rwc"))
                };

                let pool = backend.connect().await.context("连接本地数据库失败")?;
                backend.migrate(&pool).await.context("执行数据库迁移失败")?;

                let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
                let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
                let settings_repository = Arc::new(SqliteSettingsRepository::new(pool));

                Ok(Arc::new(Self {
                    feed_service: FeedService::new(feed_repository.clone()),
                    entry_service: EntryService::new(entry_repository.clone()),
                    settings_service: SettingsService::new(settings_repository),
                    feed_repository,
                    entry_repository,
                    fetch_client: FetchClient::new(),
                    parser: FeedParser::new(),
                }))
            })
            .await
            .map(Arc::clone)
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

    pub async fn set_read(&self, entry_id: i64, is_read: bool) -> anyhow::Result<()> {
        self.entry_service.set_read(entry_id, is_read).await
    }

    pub async fn set_starred(&self, entry_id: i64, is_starred: bool) -> anyhow::Result<()> {
        self.entry_service.set_starred(entry_id, is_starred).await
    }

    pub async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<()> {
        let url = Url::parse(raw_url).context("订阅 URL 不合法")?;
        let feed = self
            .feed_service
            .add_subscription(&NewFeedSubscription { url, title: None, folder: None })
            .await
            .context("保存订阅失败")?;
        self.refresh_feed(feed.id).await.context("首次刷新订阅失败")?;
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

        let response = self
            .fetch_client
            .fetch(&FetchRequest {
                url: feed.url.to_string(),
                etag: feed.etag.clone(),
                last_modified: feed.last_modified.clone(),
            })
            .await
            .with_context(|| format!("抓取订阅失败: {}", feed.url))?;

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
                let parsed = self.parser.parse(&body).context("解析订阅失败")?;
                self.feed_repository
                    .update_feed_metadata(feed.id, &parsed)
                    .await
                    .context("更新订阅元数据失败")?;
                self.entry_repository
                    .upsert_entries(feed.id, &parsed.entries)
                    .await
                    .context("写入文章失败")?;
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
        }

        Ok(())
    }
}
