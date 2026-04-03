use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[path = "web/config.rs"]
mod config;
#[path = "web/feed.rs"]
mod feed;
#[path = "web/state.rs"]
mod state;

use anyhow::Context;
use js_sys::Date;
use reqwest::{StatusCode, header};
use rssr_domain::{
    ConfigFeed, ConfigPackage, Entry, EntryQuery, EntrySummary, FeedSummary, ReadFilter,
    StarredFilter, UserSettings, normalize_feed_url,
};
use time::OffsetDateTime;
use tokio::sync::OnceCell;
use url::Url;
use wasm_bindgen_futures::spawn_local;

use self::{
    config::{
        decode_opml, encode_opml, import_field, remote_url, validate_config_package,
        validate_settings,
    },
    feed::{ParsedEntry, ParsedFeed, parse_feed, web_fetch_feed_response},
    state::{
        PersistedEntry, PersistedFeed, PersistedState, load_state, save_state, to_domain_entry,
        upsert_entries,
    },
};

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

#[derive(Debug, Clone, Copy, Default)]
pub struct ReaderNavigation {
    pub previous_unread_entry_id: Option<i64>,
    pub next_unread_entry_id: Option<i64>,
    pub previous_feed_entry_id: Option<i64>,
    pub next_feed_entry_id: Option<i64>,
}

pub struct AppServices {
    state: Mutex<PersistedState>,
    client: reqwest::Client,
    auto_refresh_started: AtomicBool,
}

impl AppServices {
    const AUTO_REFRESH_POLL_INTERVAL: Duration = Duration::from_secs(30);

    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async {
                Ok(Arc::new(Self {
                    state: Mutex::new(load_state().unwrap_or_default()),
                    client: reqwest::Client::new(),
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
        let state = self.state.lock().expect("lock state");
        let mut feeds = state
            .feeds
            .iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| FeedSummary {
                id: feed.id,
                title: feed.title.clone().unwrap_or_else(|| feed.url.clone()),
                url: feed.url.clone(),
                unread_count: state
                    .entries
                    .iter()
                    .filter(|entry| entry.feed_id == feed.id && !entry.is_read)
                    .count() as u32,
                entry_count: state.entries.iter().filter(|entry| entry.feed_id == feed.id).count()
                    as u32,
                last_fetched_at: feed.last_fetched_at,
                last_success_at: feed.last_success_at,
                fetch_error: feed.fetch_error.clone(),
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
                let Some(feed) = state.feeds.iter().find(|feed| feed.id == entry.feed_id) else {
                    return false;
                };
                if feed.is_deleted {
                    return false;
                }
                if let Some(feed_id) = query.feed_id
                    && entry.feed_id != feed_id
                {
                    return false;
                }
                if !query.feed_ids.is_empty() && !query.feed_ids.contains(&entry.feed_id) {
                    return false;
                }
                match query.read_filter {
                    ReadFilter::All => {}
                    ReadFilter::UnreadOnly if entry.is_read => return false,
                    ReadFilter::ReadOnly if !entry.is_read => return false,
                    _ => {}
                }
                match query.starred_filter {
                    StarredFilter::All => {}
                    StarredFilter::StarredOnly if !entry.is_starred => return false,
                    StarredFilter::UnstarredOnly if entry.is_starred => return false,
                    _ => {}
                }
                if let Some(search) = &query.search_title
                    && !title_matches_search(&entry.title, search)
                {
                    return false;
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

    pub async fn reader_navigation(
        &self,
        current_entry_id: i64,
    ) -> anyhow::Result<ReaderNavigation> {
        let Some(current_entry) = self.get_entry(current_entry_id).await? else {
            return Ok(ReaderNavigation::default());
        };

        let global_entries = self.list_entries(&EntryQuery::default()).await?;
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
        let mut state = self.state.lock().expect("lock state");
        let now = web_now_utc();
        let entry =
            state.entries.iter_mut().find(|entry| entry.id == entry_id).context("文章不存在")?;
        entry.is_read = is_read;
        entry.read_at = is_read.then_some(now);
        entry.updated_at = now;
        save_state(&state)
    }

    pub async fn set_starred(&self, entry_id: i64, is_starred: bool) -> anyhow::Result<()> {
        let mut state = self.state.lock().expect("lock state");
        let now = web_now_utc();
        let entry =
            state.entries.iter_mut().find(|entry| entry.id == entry_id).context("文章不存在")?;
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

    pub async fn load_last_opened_feed_id(&self) -> anyhow::Result<Option<i64>> {
        Ok(self.state.lock().expect("lock state").last_opened_feed_id)
    }

    pub async fn remember_last_opened_feed_id(&self, feed_id: i64) -> anyhow::Result<()> {
        let mut state = self.state.lock().expect("lock state");
        state.last_opened_feed_id = Some(feed_id);
        save_state(&state)
    }

    pub fn ensure_auto_refresh_started(self: &Arc<Self>) {
        if self.auto_refresh_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let services = Arc::clone(self);
        spawn_local(async move {
            let mut last_refresh_started_at = None;

            loop {
                let settings = match services.load_settings().await {
                    Ok(settings) => settings,
                    Err(error) => {
                        tracing::warn!(error = %error, "读取自动刷新设置失败，稍后重试");
                        gloo_timers::future::sleep(Self::AUTO_REFRESH_POLL_INTERVAL).await;
                        continue;
                    }
                };

                let now = web_now_utc();
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

                gloo_timers::future::sleep(Self::AUTO_REFRESH_POLL_INTERVAL).await;
            }
        });
    }

    pub async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<()> {
        let url = normalize_feed_url(&Url::parse(raw_url).context("订阅 URL 不合法")?);
        let mut state = self.state.lock().expect("lock state");
        let now = web_now_utc();
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
        self.refresh_feed(feed_id).await.context("首次刷新订阅失败")
    }

    pub async fn remove_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        let mut state = self.state.lock().expect("lock state");
        let feed = state
            .feeds
            .iter_mut()
            .find(|feed| feed.id == feed_id && !feed.is_deleted)
            .context("订阅不存在")?;
        feed.is_deleted = true;
        feed.updated_at = web_now_utc();
        state.entries.retain(|entry| entry.feed_id != feed_id);
        if state.last_opened_feed_id == Some(feed_id) {
            state.last_opened_feed_id = None;
        }
        save_state(&state)
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
        let url = {
            let state = self.state.lock().expect("lock state");
            let feed = state
                .feeds
                .iter()
                .find(|feed| feed.id == feed_id && !feed.is_deleted)
                .context("订阅不存在")?;
            feed.url.clone()
        };

        let response = match web_fetch_feed_response(&self.client, &url).await {
            Ok(response) => response,
            Err(error) => {
                let mut state = self.state.lock().expect("lock state");
                let now = web_now_utc();
                if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == feed_id) {
                    feed.last_fetched_at = Some(now);
                    feed.fetch_error = Some(format!("抓取订阅失败: {error}"));
                    feed.updated_at = now;
                    let _ = save_state(&state);
                }
                return Err(error);
            }
        };
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
            let now = web_now_utc();
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

        let body = match response.error_for_status() {
            Ok(response) => match response.text().await {
                Ok(body) => body,
                Err(error) => {
                    let mut state = self.state.lock().expect("lock state");
                    let now = web_now_utc();
                    if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == feed_id) {
                        feed.last_fetched_at = Some(now);
                        feed.fetch_error = Some(format!("读取 feed 响应正文失败: {error}"));
                        feed.updated_at = now;
                        let _ = save_state(&state);
                    }
                    return Err(error).context("读取 feed 响应正文失败");
                }
            },
            Err(error) => {
                let mut state = self.state.lock().expect("lock state");
                let now = web_now_utc();
                if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == feed_id) {
                    feed.last_fetched_at = Some(now);
                    feed.fetch_error = Some(format!("feed 抓取返回非成功状态: {error}"));
                    feed.updated_at = now;
                    let _ = save_state(&state);
                }
                return Err(error).context("feed 抓取返回非成功状态");
            }
        };
        let parsed = match parse_feed(&body) {
            Ok(parsed) => parsed,
            Err(error) => {
                let mut state = self.state.lock().expect("lock state");
                let now = web_now_utc();
                if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == feed_id) {
                    feed.last_fetched_at = Some(now);
                    feed.fetch_error = Some(format!("解析订阅失败: {error}"));
                    feed.updated_at = now;
                    let _ = save_state(&state);
                }
                return Err(error).context("解析订阅失败");
            }
        };

        let mut state = self.state.lock().expect("lock state");
        let now = web_now_utc();
        let feed = state.feeds.iter_mut().find(|feed| feed.id == feed_id).context("订阅不存在")?;
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
            let url = normalize_feed_url(
                &Url::parse(&feed.url).with_context(|| format!("无效的订阅 URL：{}", feed.url))?,
            );
            imported_urls.insert(url.to_string());
            let now = web_now_utc();
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
            .filter_map(|(id, url)| {
                let normalized = normalize_feed_url(
                    &Url::parse(&url).expect("persisted feed url should stay valid"),
                );
                (!imported_urls.contains(normalized.as_str())).then_some(id)
            })
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
            let url = normalize_feed_url(
                &Url::parse(&feed.url).with_context(|| format!("无效的订阅 URL：{}", feed.url))?,
            );
            let now = web_now_utc();
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
            exported_at: web_now_utc(),
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

fn title_matches_search(title: &str, search: &str) -> bool {
    title.to_lowercase().contains(&search.to_lowercase())
}

fn web_now_utc() -> OffsetDateTime {
    let millis = Date::now() as i128;
    OffsetDateTime::from_unix_timestamp_nanos(millis * 1_000_000)
        .expect("browser timestamp should fit in OffsetDateTime")
}

#[cfg(test)]
mod tests {
    use super::title_matches_search;

    #[test]
    fn web_title_search_is_case_insensitive() {
        assert!(title_matches_search("Roche Scales NVIDIA AI Factories", "sca"));
        assert!(title_matches_search("Roche Scales NVIDIA AI Factories", "SCA"));
        assert!(!title_matches_search("Roche Scales NVIDIA AI Factories", "xyz"));
    }
}
