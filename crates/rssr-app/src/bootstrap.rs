#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::sync::Arc;

    use anyhow::Context;
    use rssr_application::{
        entry_service::EntryService, feed_service::FeedService,
        import_export_service::ImportExportService, settings_service::SettingsService,
    };
    use rssr_domain::{
        Entry, EntryQuery, FeedRepository, FeedSummary, NewFeedSubscription, UserSettings,
    };
    use rssr_infra::{
        config_sync::webdav::WebDavConfigSync,
        db::{
            entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository,
            settings_repository::SqliteSettingsRepository, sqlite_native::NativeSqliteBackend,
            storage_backend::StorageBackend,
        },
        fetch::{FetchClient, FetchRequest, FetchResult},
        opml::OpmlCodec,
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
        settings_service: SettingsService,
        import_export_service: ImportExportService,
        fetch_client: FetchClient,
        parser: FeedParser,
        opml_codec: OpmlCodec,
    }

    impl AppServices {
        pub async fn shared() -> anyhow::Result<Arc<Self>> {
            APP_SERVICES
                .get_or_try_init(|| async {
                    let backend: Box<dyn StorageBackend> =
                        Box::new(NativeSqliteBackend::new("sqlite:rss-reader.db?mode=rwc"));

                    let pool = backend.connect().await.context("连接本地数据库失败")?;
                    backend.migrate(&pool).await.context("执行数据库迁移失败")?;

                    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
                    let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
                    let settings_repository = Arc::new(SqliteSettingsRepository::new(pool));

                    Ok(Arc::new(Self {
                        feed_service: FeedService::new(feed_repository.clone()),
                        entry_service: EntryService::new(entry_repository.clone()),
                        settings_service: SettingsService::new(settings_repository.clone()),
                        import_export_service: ImportExportService::new(
                            feed_repository.clone(),
                            settings_repository,
                        ),
                        feed_repository,
                        entry_repository,
                        fetch_client: FetchClient::new(),
                        parser: FeedParser::new(),
                        opml_codec: OpmlCodec::new(),
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
            for feed in feeds {
                let url = Url::parse(&feed.url).context("OPML 中存在无效订阅 URL")?;
                self.feed_service
                    .add_subscription(&NewFeedSubscription {
                        url,
                        title: feed.title,
                        folder: feed.folder,
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
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::sync::Arc;

    use rssr_domain::{Entry, EntryQuery, FeedSummary, UserSettings};
    use tokio::sync::OnceCell;

    static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

    pub struct AppServices;

    impl AppServices {
        pub async fn shared() -> anyhow::Result<Arc<Self>> {
            Ok(Arc::clone(APP_SERVICES.get_or_init(|| async { Arc::new(Self) }).await))
        }

        pub fn default_settings() -> UserSettings {
            UserSettings::default()
        }

        pub async fn list_feeds(&self) -> anyhow::Result<Vec<FeedSummary>> {
            Ok(Vec::new())
        }

        pub async fn list_entries(
            &self,
            _query: &EntryQuery,
        ) -> anyhow::Result<Vec<rssr_domain::EntrySummary>> {
            Ok(Vec::new())
        }

        pub async fn get_entry(&self, _entry_id: i64) -> anyhow::Result<Option<Entry>> {
            Ok(None)
        }

        pub async fn set_read(&self, _entry_id: i64, _is_read: bool) -> anyhow::Result<()> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }

        pub async fn set_starred(&self, _entry_id: i64, _is_starred: bool) -> anyhow::Result<()> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }

        pub async fn load_settings(&self) -> anyhow::Result<UserSettings> {
            Ok(UserSettings::default())
        }

        pub async fn save_settings(&self, _settings: &UserSettings) -> anyhow::Result<()> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }

        pub async fn add_subscription(&self, _raw_url: &str) -> anyhow::Result<()> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }

        pub async fn refresh_all(&self) -> anyhow::Result<()> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }

        pub async fn export_config_json(&self) -> anyhow::Result<String> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }

        pub async fn import_config_json(&self, _raw: &str) -> anyhow::Result<()> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }

        pub async fn export_opml(&self) -> anyhow::Result<String> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }

        pub async fn import_opml(&self, _raw: &str) -> anyhow::Result<()> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }

        pub async fn push_remote_config(
            &self,
            _endpoint: &str,
            _remote_path: &str,
        ) -> anyhow::Result<()> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }

        pub async fn pull_remote_config(
            &self,
            _endpoint: &str,
            _remote_path: &str,
        ) -> anyhow::Result<bool> {
            anyhow::bail!("Web 平台存储后端尚未完成接线")
        }
    }
}

pub use imp::AppServices;
