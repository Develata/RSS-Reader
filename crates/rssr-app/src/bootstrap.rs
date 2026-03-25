#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::sync::Arc;

    use anyhow::Context;
    use rssr_application::{EntryService, FeedService, ImportExportService, SettingsService};
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
                            entry_repository.clone(),
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
            let current_feeds = self.feed_repository.list_feeds().await?;
            for feed in feeds {
                let url = Url::parse(&feed.url).context("OPML 中存在无效订阅 URL")?;
                let existed = current_feeds.iter().any(|current| current.url == url);
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
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::{
        collections::{BTreeMap, HashSet},
        sync::{Arc, Mutex},
    };

    use anyhow::{Context, ensure};
    use feed_rs::model::{Entry as FeedRsEntry, Feed as FeedRsFeed, Text};
    use quick_xml::{
        Reader, Writer,
        encoding::Decoder,
        events::{BytesDecl, BytesEnd, BytesStart, Event},
    };
    use reqwest::{StatusCode, header};
    use rssr_domain::{
        ConfigFeed, ConfigPackage, Entry, EntryQuery, EntrySummary, FeedSummary, UserSettings,
    };
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256};
    use time::OffsetDateTime;
    use tokio::sync::OnceCell;
    use url::Url;
    use web_sys::window;

    static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();
    const STORAGE_KEY: &str = "rssr-web-state-v1";

    #[derive(Debug, Default, Serialize, Deserialize)]
    struct PersistedState {
        next_feed_id: i64,
        next_entry_id: i64,
        feeds: Vec<PersistedFeed>,
        entries: Vec<PersistedEntry>,
        settings: UserSettings,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct PersistedFeed {
        id: i64,
        url: String,
        title: Option<String>,
        site_url: Option<String>,
        description: Option<String>,
        icon_url: Option<String>,
        folder: Option<String>,
        etag: Option<String>,
        last_modified: Option<String>,
        last_fetched_at: Option<OffsetDateTime>,
        last_success_at: Option<OffsetDateTime>,
        fetch_error: Option<String>,
        is_deleted: bool,
        created_at: OffsetDateTime,
        updated_at: OffsetDateTime,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct PersistedEntry {
        id: i64,
        feed_id: i64,
        external_id: String,
        dedup_key: String,
        url: Option<String>,
        title: String,
        author: Option<String>,
        summary: Option<String>,
        content_html: Option<String>,
        content_text: Option<String>,
        published_at: Option<OffsetDateTime>,
        updated_at_source: Option<OffsetDateTime>,
        first_seen_at: OffsetDateTime,
        content_hash: Option<String>,
        is_read: bool,
        is_starred: bool,
        read_at: Option<OffsetDateTime>,
        starred_at: Option<OffsetDateTime>,
        created_at: OffsetDateTime,
        updated_at: OffsetDateTime,
    }

    #[derive(Debug, Clone)]
    struct ParsedFeed {
        title: Option<String>,
        site_url: Option<Url>,
        description: Option<String>,
        entries: Vec<ParsedEntry>,
    }

    #[derive(Debug, Clone)]
    struct ParsedEntry {
        external_id: String,
        dedup_key: String,
        url: Option<Url>,
        title: String,
        author: Option<String>,
        summary: Option<String>,
        content_html: Option<String>,
        content_text: Option<String>,
        published_at: Option<OffsetDateTime>,
        updated_at_source: Option<OffsetDateTime>,
    }

    pub struct AppServices {
        state: Mutex<PersistedState>,
        client: reqwest::Client,
    }

    impl AppServices {
        pub async fn shared() -> anyhow::Result<Arc<Self>> {
            APP_SERVICES
                .get_or_try_init(|| async {
                    Ok(Arc::new(Self {
                        state: Mutex::new(load_state().unwrap_or_default()),
                        client: reqwest::Client::new(),
                    }))
                })
                .await
                .map(Arc::clone)
        }

        pub fn default_settings() -> UserSettings {
            UserSettings::default()
        }

        pub async fn list_feeds(&self) -> anyhow::Result<Vec<FeedSummary>> {
            let state = self.state.lock().expect("lock state");
            let mut feeds = state
                .feeds
                .iter()
                .filter(|feed| !feed.is_deleted)
                .map(|feed| FeedSummary {
                    id: feed.id,
                    title: feed.title.clone().unwrap_or_else(|| feed.url.clone()),
                    unread_count: state
                        .entries
                        .iter()
                        .filter(|entry| entry.feed_id == feed.id && !entry.is_read)
                        .count() as u32,
                })
                .collect::<Vec<_>>();
            feeds.sort_by(|left, right| left.title.cmp(&right.title));
            Ok(feeds)
        }

        pub async fn list_entries(&self, query: &EntryQuery) -> anyhow::Result<Vec<EntrySummary>> {
            let state = self.state.lock().expect("lock state");
            let mut items = state
                .entries
                .iter()
                .filter(|entry| {
                    let Some(feed) = state.feeds.iter().find(|feed| feed.id == entry.feed_id)
                    else {
                        return false;
                    };
                    if feed.is_deleted {
                        return false;
                    }
                    if let Some(feed_id) = query.feed_id {
                        if entry.feed_id != feed_id {
                            return false;
                        }
                    }
                    if query.unread_only && entry.is_read {
                        return false;
                    }
                    if query.starred_only && !entry.is_starred {
                        return false;
                    }
                    if let Some(search) = &query.search_title {
                        if !entry.title.contains(search) {
                            return false;
                        }
                    }
                    true
                })
                .map(|entry| EntrySummary {
                    id: entry.id,
                    feed_id: entry.feed_id,
                    title: entry.title.clone(),
                    feed_title: state
                        .feeds
                        .iter()
                        .find(|feed| feed.id == entry.feed_id)
                        .and_then(|feed| feed.title.clone())
                        .unwrap_or_else(|| {
                            state
                                .feeds
                                .iter()
                                .find(|feed| feed.id == entry.feed_id)
                                .map(|feed| feed.url.clone())
                                .unwrap_or_default()
                        }),
                    published_at: entry.published_at,
                    is_read: entry.is_read,
                    is_starred: entry.is_starred,
                })
                .collect::<Vec<_>>();
            items.sort_by(|left, right| {
                right.published_at.cmp(&left.published_at).then(right.id.cmp(&left.id))
            });
            if let Some(limit) = query.limit {
                items.truncate(limit as usize);
            }
            Ok(items)
        }

        pub async fn get_entry(&self, entry_id: i64) -> anyhow::Result<Option<Entry>> {
            let state = self.state.lock().expect("lock state");
            Ok(state
                .entries
                .iter()
                .find(|entry| entry.id == entry_id)
                .map(to_domain_entry)
                .transpose()?)
        }

        pub async fn set_read(&self, entry_id: i64, is_read: bool) -> anyhow::Result<()> {
            let mut state = self.state.lock().expect("lock state");
            let now = OffsetDateTime::now_utc();
            let entry = state
                .entries
                .iter_mut()
                .find(|entry| entry.id == entry_id)
                .context("文章不存在")?;
            entry.is_read = is_read;
            entry.read_at = is_read.then_some(now);
            entry.updated_at = now;
            save_state(&state)
        }

        pub async fn set_starred(&self, entry_id: i64, is_starred: bool) -> anyhow::Result<()> {
            let mut state = self.state.lock().expect("lock state");
            let now = OffsetDateTime::now_utc();
            let entry = state
                .entries
                .iter_mut()
                .find(|entry| entry.id == entry_id)
                .context("文章不存在")?;
            entry.is_starred = is_starred;
            entry.starred_at = is_starred.then_some(now);
            entry.updated_at = now;
            save_state(&state)
        }

        pub async fn load_settings(&self) -> anyhow::Result<UserSettings> {
            Ok(self.state.lock().expect("lock state").settings.clone())
        }

        pub async fn save_settings(&self, settings: &UserSettings) -> anyhow::Result<()> {
            validate_settings(settings)?;
            let mut state = self.state.lock().expect("lock state");
            state.settings = settings.clone();
            save_state(&state)
        }

        pub async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<()> {
            let url = Url::parse(raw_url).context("订阅 URL 不合法")?;
            let mut state = self.state.lock().expect("lock state");
            let now = OffsetDateTime::now_utc();
            if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.url == url.as_str()) {
                feed.is_deleted = false;
                feed.updated_at = now;
            } else {
                state.next_feed_id += 1;
                let feed_id = state.next_feed_id;
                state.feeds.push(PersistedFeed {
                    id: feed_id,
                    url: url.to_string(),
                    title: None,
                    site_url: None,
                    description: None,
                    icon_url: None,
                    folder: None,
                    etag: None,
                    last_modified: None,
                    last_fetched_at: None,
                    last_success_at: None,
                    fetch_error: None,
                    is_deleted: false,
                    created_at: now,
                    updated_at: now,
                });
            }
            let feed_id =
                state.feeds.iter().find(|feed| feed.url == url.as_str()).expect("feed exists").id;
            save_state(&state)?;
            drop(state);
            self.refresh_feed(feed_id).await
        }

        pub async fn refresh_all(&self) -> anyhow::Result<()> {
            let feed_ids = {
                let state = self.state.lock().expect("lock state");
                state
                    .feeds
                    .iter()
                    .filter(|feed| !feed.is_deleted)
                    .map(|feed| feed.id)
                    .collect::<Vec<_>>()
            };
            let mut errors = Vec::new();
            for feed_id in feed_ids {
                if let Err(error) = self.refresh_feed(feed_id).await {
                    errors.push(error.to_string());
                }
            }
            if !errors.is_empty() {
                anyhow::bail!("部分订阅刷新失败: {}", errors.join(" | "));
            }
            Ok(())
        }

        pub async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<()> {
            let (url, etag, last_modified) = {
                let state = self.state.lock().expect("lock state");
                let feed = state
                    .feeds
                    .iter()
                    .find(|feed| feed.id == feed_id && !feed.is_deleted)
                    .context("订阅不存在")?;
                (feed.url.clone(), feed.etag.clone(), feed.last_modified.clone())
            };

            let mut request = self.client.get(&url).header(
                header::ACCEPT,
                "application/atom+xml, application/rss+xml, application/xml, text/xml;q=0.9, */*;q=0.1",
            );
            if let Some(etag) = &etag {
                request = request.header(header::IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = &last_modified {
                request = request.header(header::IF_MODIFIED_SINCE, last_modified);
            }

            let response = request.send().await.context("发送 feed 抓取请求失败")?;
            let metadata = (
                response
                    .headers()
                    .get(header::ETAG)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned),
                response
                    .headers()
                    .get(header::LAST_MODIFIED)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned),
            );

            if response.status() == StatusCode::NOT_MODIFIED {
                let mut state = self.state.lock().expect("lock state");
                let now = OffsetDateTime::now_utc();
                let feed =
                    state.feeds.iter_mut().find(|feed| feed.id == feed_id).context("订阅不存在")?;
                feed.etag = metadata.0;
                feed.last_modified = metadata.1;
                feed.last_fetched_at = Some(now);
                feed.last_success_at = Some(now);
                feed.fetch_error = None;
                feed.updated_at = now;
                return save_state(&state);
            }

            let body = response
                .error_for_status()
                .context("feed 抓取返回非成功状态")?
                .text()
                .await
                .context("读取 feed 响应正文失败")?;
            let parsed = parse_feed(&body).context("解析订阅失败")?;

            let mut state = self.state.lock().expect("lock state");
            let now = OffsetDateTime::now_utc();
            let feed =
                state.feeds.iter_mut().find(|feed| feed.id == feed_id).context("订阅不存在")?;
            if parsed.title.is_some() {
                feed.title = parsed.title;
            }
            if parsed.site_url.is_some() {
                feed.site_url = parsed.site_url.map(|url| url.to_string());
            }
            if parsed.description.is_some() {
                feed.description = parsed.description;
            }
            feed.etag = metadata.0;
            feed.last_modified = metadata.1;
            feed.last_fetched_at = Some(now);
            feed.last_success_at = Some(now);
            feed.fetch_error = None;
            feed.updated_at = now;

            upsert_entries(&mut state, feed_id, parsed.entries)?;
            save_state(&state)
        }

        pub async fn export_config_json(&self) -> anyhow::Result<String> {
            Ok(serde_json::to_string_pretty(&self.export_config()?)?)
        }

        pub async fn import_config_json(&self, raw: &str) -> anyhow::Result<()> {
            let package: ConfigPackage = serde_json::from_str(raw)?;
            validate_config_package(&package)?;

            let mut state = self.state.lock().expect("lock state");
            let current_urls = state
                .feeds
                .iter()
                .filter(|feed| !feed.is_deleted)
                .map(|feed| (feed.id, feed.url.clone()))
                .collect::<Vec<_>>();
            let mut imported_urls = HashSet::new();

            for feed in package.feeds {
                let url = Url::parse(&feed.url)
                    .with_context(|| format!("无效的订阅 URL：{}", feed.url))?;
                imported_urls.insert(url.to_string());
                let now = OffsetDateTime::now_utc();
                if let Some(existing) =
                    state.feeds.iter_mut().find(|current| current.url == url.as_str())
                {
                    existing.title = import_field(feed.title, true);
                    existing.folder = import_field(feed.folder, true);
                    existing.is_deleted = false;
                    existing.updated_at = now;
                } else {
                    state.next_feed_id += 1;
                    let feed_id = state.next_feed_id;
                    state.feeds.push(PersistedFeed {
                        id: feed_id,
                        url: url.to_string(),
                        title: feed.title,
                        site_url: None,
                        description: None,
                        icon_url: None,
                        folder: feed.folder,
                        etag: None,
                        last_modified: None,
                        last_fetched_at: None,
                        last_success_at: None,
                        fetch_error: None,
                        is_deleted: false,
                        created_at: now,
                        updated_at: now,
                    });
                }
            }

            let removed_feed_ids = current_urls
                .into_iter()
                .filter_map(|(id, url)| (!imported_urls.contains(&url)).then_some(id))
                .collect::<Vec<_>>();
            for feed_id in &removed_feed_ids {
                if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == *feed_id) {
                    feed.is_deleted = true;
                }
            }
            state.entries.retain(|entry| !removed_feed_ids.contains(&entry.feed_id));
            state.settings = package.settings;
            save_state(&state)
        }

        pub async fn export_opml(&self) -> anyhow::Result<String> {
            encode_opml(&self.export_config()?.feeds)
        }

        pub async fn import_opml(&self, raw: &str) -> anyhow::Result<()> {
            let feeds = decode_opml(raw)?;
            let mut state = self.state.lock().expect("lock state");
            for feed in feeds {
                let url = Url::parse(&feed.url)
                    .with_context(|| format!("无效的订阅 URL：{}", feed.url))?;
                let now = OffsetDateTime::now_utc();
                if let Some(existing) =
                    state.feeds.iter_mut().find(|current| current.url == url.as_str())
                {
                    existing.title = import_field(feed.title, true);
                    existing.folder = import_field(feed.folder, true);
                    existing.is_deleted = false;
                    existing.updated_at = now;
                } else {
                    state.next_feed_id += 1;
                    let feed_id = state.next_feed_id;
                    state.feeds.push(PersistedFeed {
                        id: feed_id,
                        url: url.to_string(),
                        title: feed.title,
                        site_url: None,
                        description: None,
                        icon_url: None,
                        folder: feed.folder,
                        etag: None,
                        last_modified: None,
                        last_fetched_at: None,
                        last_success_at: None,
                        fetch_error: None,
                        is_deleted: false,
                        created_at: now,
                        updated_at: now,
                    });
                }
            }
            save_state(&state)
        }

        pub async fn push_remote_config(
            &self,
            endpoint: &str,
            remote_path: &str,
        ) -> anyhow::Result<()> {
            let url = remote_url(endpoint, remote_path)?;
            self.client
                .put(url)
                .header("content-type", "application/json")
                .body(self.export_config_json().await?)
                .send()
                .await
                .context("上传配置到 WebDAV 失败")?
                .error_for_status()
                .context("WebDAV 上传失败")?;
            Ok(())
        }

        pub async fn pull_remote_config(
            &self,
            endpoint: &str,
            remote_path: &str,
        ) -> anyhow::Result<bool> {
            let response = self
                .client
                .get(remote_url(endpoint, remote_path)?)
                .send()
                .await
                .context("从 WebDAV 下载配置失败")?;
            if response.status() == StatusCode::NOT_FOUND {
                return Ok(false);
            }
            let raw = response.error_for_status().context("WebDAV 下载失败")?.text().await?;
            self.import_config_json(&raw).await?;
            Ok(true)
        }

        fn export_config(&self) -> anyhow::Result<ConfigPackage> {
            let state = self.state.lock().expect("lock state");
            Ok(ConfigPackage {
                version: 1,
                exported_at: OffsetDateTime::now_utc(),
                feeds: state
                    .feeds
                    .iter()
                    .filter(|feed| !feed.is_deleted)
                    .map(|feed| ConfigFeed {
                        url: feed.url.clone(),
                        title: feed.title.clone(),
                        folder: feed.folder.clone(),
                    })
                    .collect(),
                settings: state.settings.clone(),
            })
        }
    }

    fn load_state() -> anyhow::Result<PersistedState> {
        let Some(storage) = window().and_then(|window| window.local_storage().ok()).flatten()
        else {
            return Ok(PersistedState::default());
        };
        match storage
            .get_item(STORAGE_KEY)
            .map_err(|_| anyhow::anyhow!("读取浏览器本地存储失败"))?
        {
            Some(raw) => Ok(serde_json::from_str(&raw).context("解析浏览器本地状态失败")?),
            None => Ok(PersistedState::default()),
        }
    }

    fn save_state(state: &PersistedState) -> anyhow::Result<()> {
        let Some(storage) = window().and_then(|window| window.local_storage().ok()).flatten()
        else {
            return Ok(());
        };
        storage
            .set_item(STORAGE_KEY, &serde_json::to_string(state)?)
            .map_err(|_| anyhow::anyhow!("写入浏览器本地存储失败"))?;
        Ok(())
    }

    fn to_domain_entry(entry: &PersistedEntry) -> anyhow::Result<Entry> {
        Ok(Entry {
            id: entry.id,
            feed_id: entry.feed_id,
            external_id: entry.external_id.clone(),
            dedup_key: entry.dedup_key.clone(),
            url: entry.url.as_ref().map(|raw| Url::parse(raw)).transpose()?,
            title: entry.title.clone(),
            author: entry.author.clone(),
            summary: entry.summary.clone(),
            content_html: entry.content_html.clone(),
            content_text: entry.content_text.clone(),
            published_at: entry.published_at,
            updated_at_source: entry.updated_at_source,
            first_seen_at: entry.first_seen_at,
            content_hash: entry.content_hash.clone(),
            is_read: entry.is_read,
            is_starred: entry.is_starred,
            read_at: entry.read_at,
            starred_at: entry.starred_at,
            created_at: entry.created_at,
            updated_at: entry.updated_at,
        })
    }

    fn upsert_entries(
        state: &mut PersistedState,
        feed_id: i64,
        entries: Vec<ParsedEntry>,
    ) -> anyhow::Result<()> {
        for entry in entries {
            let content_hash = hash_content(
                entry.content_html.as_deref(),
                entry.content_text.as_deref(),
                Some(&entry.title),
            );
            let now = OffsetDateTime::now_utc();
            if let Some(existing) = state
                .entries
                .iter_mut()
                .find(|current| current.feed_id == feed_id && current.dedup_key == entry.dedup_key)
            {
                existing.external_id = entry.external_id;
                if let Some(url) = entry.url.as_ref() {
                    existing.url = Some(url.to_string());
                }
                existing.title = entry.title;
                existing.author = entry.author;
                existing.summary = entry.summary;
                if entry.content_html.is_some() {
                    existing.content_html = entry.content_html;
                }
                if entry.content_text.is_some() {
                    existing.content_text = entry.content_text;
                }
                existing.published_at = entry.published_at.or(existing.published_at);
                existing.updated_at_source = entry.updated_at_source.or(existing.updated_at_source);
                existing.content_hash = content_hash;
                existing.updated_at = now;
            } else {
                state.next_entry_id += 1;
                state.entries.push(PersistedEntry {
                    id: state.next_entry_id,
                    feed_id,
                    external_id: entry.external_id,
                    dedup_key: entry.dedup_key,
                    url: entry.url.map(|url| url.to_string()),
                    title: entry.title,
                    author: entry.author,
                    summary: entry.summary,
                    content_html: entry.content_html,
                    content_text: entry.content_text,
                    published_at: entry.published_at,
                    updated_at_source: entry.updated_at_source,
                    first_seen_at: now,
                    content_hash,
                    is_read: false,
                    is_starred: false,
                    read_at: None,
                    starred_at: None,
                    created_at: now,
                    updated_at: now,
                });
            }
        }
        Ok(())
    }

    fn parse_feed(raw: &str) -> anyhow::Result<ParsedFeed> {
        normalize_feed(feed_rs::parser::parse(raw.as_bytes()).context("解析 RSS/Atom feed 失败")?)
    }

    fn normalize_feed(feed: FeedRsFeed) -> anyhow::Result<ParsedFeed> {
        let title = text_value(feed.title.as_ref());
        let site_url = feed.links.first().and_then(|link| Url::parse(link.href.as_str()).ok());
        let description = feed.description.as_ref().map(text_content);
        let mut entries = Vec::new();
        for entry in feed.entries {
            if let Some(entry) = normalize_entry(entry)? {
                entries.push(entry);
            }
        }
        Ok(ParsedFeed { title, site_url, description, entries })
    }

    fn normalize_entry(entry: FeedRsEntry) -> anyhow::Result<Option<ParsedEntry>> {
        let title =
            text_value(entry.title.as_ref()).unwrap_or_else(|| "Untitled entry".to_string());
        let url = entry.links.first().and_then(|link| Url::parse(link.href.as_str()).ok());
        let author = entry.authors.first().map(|author| author.name.clone());
        let summary = entry.summary.as_ref().map(text_content);
        let content_html = entry.content.as_ref().and_then(|content| content.body.clone());
        let content_text = summary.clone();
        if content_html.is_none() && content_text.is_none() {
            return Ok(None);
        }
        let published_at = entry.published.map(to_offset_datetime);
        let updated_at_source = entry.updated.map(to_offset_datetime);
        let stable_source_id = normalize_source_entry_id(&entry.id, url.as_ref());
        let external_id = if stable_source_id.is_empty() {
            url.as_ref()
                .map(|url| url.to_string())
                .unwrap_or_else(|| dedup_key_fallback(&title, published_at))
        } else {
            stable_source_id.clone()
        };
        let dedup_key = if !stable_source_id.is_empty() {
            stable_source_id
        } else if let Some(url) = &url {
            url.to_string()
        } else {
            dedup_key_fallback(&title, published_at)
        };

        Ok(Some(ParsedEntry {
            external_id,
            dedup_key,
            url,
            title,
            author,
            summary,
            content_html,
            content_text,
            published_at,
            updated_at_source,
        }))
    }

    fn dedup_key_fallback(title: &str, published_at: Option<OffsetDateTime>) -> String {
        let timestamp = published_at
            .and_then(|value| value.format(&time::format_description::well_known::Rfc3339).ok())
            .unwrap_or_else(|| "unknown".to_string());
        let normalized_title = title.trim().to_lowercase();
        let mut hasher = Sha256::new();
        hasher.update(normalized_title.as_bytes());
        hasher.update(timestamp.as_bytes());
        format!("title-ts:{:x}", hasher.finalize())
    }

    fn normalize_source_entry_id(raw: &str, url: Option<&Url>) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        if url.is_some() && looks_like_synthetic_hash(trimmed) {
            return String::new();
        }
        trimmed.to_string()
    }

    fn looks_like_synthetic_hash(value: &str) -> bool {
        matches!(value.len(), 32 | 40 | 64) && value.chars().all(|ch| ch.is_ascii_hexdigit())
    }

    fn text_value(text: Option<&Text>) -> Option<String> {
        text.map(text_content).and_then(|value| {
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        })
    }

    fn text_content(text: &Text) -> String {
        text.content.clone()
    }

    fn to_offset_datetime<Tz>(value: chrono::DateTime<Tz>) -> OffsetDateTime
    where
        Tz: chrono::TimeZone,
        Tz::Offset: Send + Sync,
    {
        OffsetDateTime::from_unix_timestamp(value.timestamp()).expect("valid unix timestamp")
    }

    fn hash_content(html: Option<&str>, text: Option<&str>, title: Option<&str>) -> Option<String> {
        let mut hasher = Sha256::new();
        let mut used = false;
        for part in [title, text, html] {
            if let Some(part) = part {
                hasher.update(part.as_bytes());
                used = true;
            }
        }
        used.then(|| format!("{:x}", hasher.finalize()))
    }

    fn validate_config_package(package: &ConfigPackage) -> anyhow::Result<()> {
        ensure!(package.version >= 1, "配置包版本必须大于等于 1");
        validate_settings(&package.settings)?;
        let mut seen_urls = HashSet::new();
        for feed in &package.feeds {
            let mut normalized =
                Url::parse(&feed.url).with_context(|| format!("无效的订阅 URL：{}", feed.url))?;
            normalized.set_fragment(None);
            ensure!(
                seen_urls.insert(normalized.to_string()),
                "配置包中包含重复的 feed URL：{}",
                feed.url
            );
        }
        Ok(())
    }

    fn validate_settings(settings: &UserSettings) -> anyhow::Result<()> {
        ensure!(settings.refresh_interval_minutes >= 1, "刷新间隔必须大于等于 1 分钟");
        ensure!(
            (0.8..=1.5).contains(&settings.reader_font_scale),
            "阅读字号缩放必须在 0.8 到 1.5 之间"
        );
        Ok(())
    }

    fn import_field(value: Option<String>, existed: bool) -> Option<String> {
        if existed { value.or(Some(String::new())) } else { value }
    }

    fn remote_url(endpoint: &str, remote_path: &str) -> anyhow::Result<Url> {
        let mut collection = Url::parse(endpoint).context("无效的 WebDAV endpoint")?;
        if !collection.path().ends_with('/') {
            collection.set_path(&format!("{}/", collection.path()));
        }
        collection.join(remote_path.trim_start_matches('/')).context("拼接 WebDAV 远端路径失败")
    }

    fn encode_opml(feeds: &[ConfigFeed]) -> anyhow::Result<String> {
        let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
        let mut opml = BytesStart::new("opml");
        opml.push_attribute(("version", "2.0"));
        writer.write_event(Event::Start(opml))?;
        writer.write_event(Event::Start(BytesStart::new("body")))?;

        let mut grouped: BTreeMap<Option<String>, Vec<&ConfigFeed>> = BTreeMap::new();
        for feed in feeds {
            grouped.entry(feed.folder.clone()).or_default().push(feed);
        }

        for (folder, group_feeds) in grouped {
            if let Some(folder) = folder.as_deref() {
                let mut outline = BytesStart::new("outline");
                outline.push_attribute(("text", folder));
                outline.push_attribute(("title", folder));
                writer.write_event(Event::Start(outline))?;
                for feed in group_feeds {
                    write_feed_outline(&mut writer, feed)?;
                }
                writer.write_event(Event::End(BytesEnd::new("outline")))?;
            } else {
                for feed in group_feeds {
                    write_feed_outline(&mut writer, feed)?;
                }
            }
        }

        writer.write_event(Event::End(BytesEnd::new("body")))?;
        writer.write_event(Event::End(BytesEnd::new("opml")))?;
        String::from_utf8(writer.into_inner()).context("OPML 输出不是有效 UTF-8")
    }

    fn write_feed_outline(writer: &mut Writer<Vec<u8>>, feed: &ConfigFeed) -> anyhow::Result<()> {
        let title = feed.title.as_deref().unwrap_or(&feed.url);
        let mut outline = BytesStart::new("outline");
        outline.push_attribute(("text", title));
        outline.push_attribute(("title", title));
        outline.push_attribute(("type", "rss"));
        outline.push_attribute(("xmlUrl", feed.url.as_str()));
        writer.write_event(Event::Empty(outline))?;
        Ok(())
    }

    fn decode_opml(raw: &str) -> anyhow::Result<Vec<ConfigFeed>> {
        let mut reader = Reader::from_str(raw);
        reader.config_mut().trim_text(true);
        let mut feeds = Vec::new();
        let mut folder_stack: Vec<Option<String>> = Vec::new();
        let mut outline_depths: Vec<bool> = Vec::new();

        loop {
            match reader.read_event()? {
                Event::Start(event) if event.name().as_ref() == b"outline" => {
                    let outline = OutlineAttrs::from_event(&event, reader.decoder())?;
                    if let Some(url) = outline.xml_url {
                        feeds.push(ConfigFeed {
                            url,
                            title: outline.title.or(outline.text),
                            folder: current_folder(&folder_stack),
                        });
                        outline_depths.push(false);
                    } else {
                        folder_stack.push(outline.title.or(outline.text));
                        outline_depths.push(true);
                    }
                }
                Event::Empty(event) if event.name().as_ref() == b"outline" => {
                    let outline = OutlineAttrs::from_event(&event, reader.decoder())?;
                    if let Some(url) = outline.xml_url {
                        feeds.push(ConfigFeed {
                            url,
                            title: outline.title.or(outline.text),
                            folder: current_folder(&folder_stack),
                        });
                    }
                }
                Event::End(event) if event.name().as_ref() == b"outline" => {
                    if outline_depths.pop().unwrap_or(false) {
                        folder_stack.pop();
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }

        Ok(feeds)
    }

    fn current_folder(folder_stack: &[Option<String>]) -> Option<String> {
        folder_stack.iter().rev().flatten().next().cloned()
    }

    struct OutlineAttrs {
        text: Option<String>,
        title: Option<String>,
        xml_url: Option<String>,
    }

    impl OutlineAttrs {
        fn from_event(event: &BytesStart<'_>, decoder: Decoder) -> anyhow::Result<Self> {
            let mut text = None;
            let mut title = None;
            let mut xml_url = None;
            for attribute in event.attributes() {
                let attribute = attribute?;
                let value = attribute.decode_and_unescape_value(decoder)?.into_owned();
                match attribute.key.as_ref() {
                    b"text" => text = Some(value),
                    b"title" => title = Some(value),
                    b"xmlUrl" => xml_url = Some(value),
                    _ => {}
                }
            }
            Ok(Self { text, title, xml_url })
        }
    }
}

pub use imp::AppServices;
