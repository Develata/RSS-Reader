use std::{
    collections::HashSet,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

#[path = "web/config.rs"]
mod config;
#[path = "web/feed.rs"]
mod feed;
#[path = "web/query.rs"]
mod query;
#[path = "web/refresh.rs"]
mod refresh;
#[path = "web/state.rs"]
mod state;

use anyhow::Context;
use js_sys::Date;
use reqwest::StatusCode;
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use rssr_domain::{
    ConfigFeed, ConfigPackage, Entry, EntryQuery, EntrySummary, FeedSummary, ReadFilter,
    StarredFilter, UserSettings, normalize_feed_url,
};
use time::OffsetDateTime;
use tokio::sync::OnceCell;
use url::Url;

use self::{
    config::{
        decode_opml, encode_opml, import_field, remote_url, validate_config_package,
        validate_settings,
    },
    query::{
        get_entry as query_get_entry, list_entries as query_list_entries,
        list_feeds as query_list_feeds, reader_navigation as query_reader_navigation,
    },
    refresh::{
        ensure_auto_refresh_started as start_auto_refresh, refresh_all as run_refresh_all,
        refresh_feed as run_refresh_feed,
    },
    state::{PersistedFeed, PersistedState, load_state, save_state_snapshot},
};

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

pub struct AppServices {
    state: Mutex<PersistedState>,
    client: reqwest::Client,
    auto_refresh_started: AtomicBool,
}

impl AppServices {
    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async {
                let loaded = load_state();
                if let Some(warning) = loaded.warning.as_deref() {
                    tracing::warn!(warning = warning, "Web 本地状态恢复时发现异常");
                }
                Ok(Arc::new(Self {
                    state: Mutex::new(loaded.state),
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
        Ok(query_list_feeds(&state))
    }

    pub async fn list_entries(&self, query: &EntryQuery) -> anyhow::Result<Vec<EntrySummary>> {
        let state = self.state.lock().expect("lock state");
        Ok(query_list_entries(&state, query))
    }

    pub async fn get_entry(&self, entry_id: i64) -> anyhow::Result<Option<Entry>> {
        let state = self.state.lock().expect("lock state");
        query_get_entry(&state, entry_id)
    }

    pub async fn reader_navigation(
        &self,
        current_entry_id: i64,
    ) -> anyhow::Result<ReaderNavigation> {
        let state = self.state.lock().expect("lock state");
        Ok(query_reader_navigation(&state, current_entry_id))
    }

    pub async fn set_read(&self, entry_id: i64, is_read: bool) -> anyhow::Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let now = web_now_utc();
            let entry = state
                .entries
                .iter_mut()
                .find(|entry| entry.id == entry_id)
                .context("文章不存在")?;
            entry.is_read = is_read;
            entry.read_at = is_read.then_some(now);
            entry.updated_at = now;
            state.clone()
        };
        save_state_snapshot(snapshot)
    }

    pub async fn set_starred(&self, entry_id: i64, is_starred: bool) -> anyhow::Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let now = web_now_utc();
            let entry = state
                .entries
                .iter_mut()
                .find(|entry| entry.id == entry_id)
                .context("文章不存在")?;
            entry.is_starred = is_starred;
            entry.starred_at = is_starred.then_some(now);
            entry.updated_at = now;
            state.clone()
        };
        save_state_snapshot(snapshot)
    }

    pub async fn load_settings(&self) -> anyhow::Result<UserSettings> {
        Ok(self.state.lock().expect("lock state").settings.clone())
    }

    pub async fn save_settings(&self, settings: &UserSettings) -> anyhow::Result<()> {
        validate_settings(settings)?;
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            state.settings = settings.clone();
            state.clone()
        };
        save_state_snapshot(snapshot)
    }

    pub async fn load_last_opened_feed_id(&self) -> anyhow::Result<Option<i64>> {
        Ok(self.state.lock().expect("lock state").last_opened_feed_id)
    }

    pub async fn remember_last_opened_feed_id(&self, feed_id: i64) -> anyhow::Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            state.last_opened_feed_id = Some(feed_id);
            state.clone()
        };
        save_state_snapshot(snapshot)
    }

    pub fn ensure_auto_refresh_started(self: &Arc<Self>) {
        start_auto_refresh(self);
    }

    pub async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<()> {
        let url = normalize_feed_url(&Url::parse(raw_url).context("订阅 URL 不合法")?);
        let feed_id = {
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
            let snapshot = state.clone();
            drop(state);
            save_state_snapshot(snapshot)?;
            feed_id
        };
        self.refresh_feed(feed_id).await.context("首次刷新订阅失败")
    }

    pub async fn remove_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        let snapshot = {
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
            state.clone()
        };
        save_state_snapshot(snapshot)
    }

    pub async fn refresh_all(&self) -> anyhow::Result<()> {
        run_refresh_all(self).await
    }

    pub async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        run_refresh_feed(self, feed_id).await
    }

    pub async fn export_config_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(&self.export_config()?)?)
    }

    pub async fn import_config_json(&self, raw: &str) -> anyhow::Result<()> {
        let package: ConfigPackage = serde_json::from_str(raw)?;
        validate_config_package(&package)?;

        let snapshot = {
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
                    &Url::parse(&feed.url)
                        .with_context(|| format!("无效的订阅 URL：{}", feed.url))?,
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
                .filter_map(|(id, url)| match Url::parse(&url) {
                    Ok(parsed) => {
                        let normalized = normalize_feed_url(&parsed);
                        (!imported_urls.contains(normalized.as_str())).then_some(id)
                    }
                    Err(error) => {
                        tracing::warn!(
                            feed_id = id,
                            invalid_url = %url,
                            error = %error,
                            "导入配置时发现损坏的已持久化订阅 URL，已将其标记为移除"
                        );
                        Some(id)
                    }
                })
                .collect::<Vec<_>>();
            for feed_id in &removed_feed_ids {
                if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == *feed_id) {
                    feed.is_deleted = true;
                }
            }
            state.entries.retain(|entry| !removed_feed_ids.contains(&entry.feed_id));
            state.settings = package.settings;
            state.clone()
        };
        save_state_snapshot(snapshot)
    }

    pub async fn export_opml(&self) -> anyhow::Result<String> {
        encode_opml(&self.export_config()?.feeds)
    }

    pub async fn import_opml(&self, raw: &str) -> anyhow::Result<()> {
        let feeds = decode_opml(raw)?;
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            for feed in feeds {
                let url = normalize_feed_url(
                    &Url::parse(&feed.url)
                        .with_context(|| format!("无效的订阅 URL：{}", feed.url))?,
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
            state.clone()
        };
        save_state_snapshot(snapshot)
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

fn web_now_utc() -> OffsetDateTime {
    let millis = Date::now() as i128;
    OffsetDateTime::from_unix_timestamp_nanos(millis * 1_000_000)
        .expect("browser timestamp should fit in OffsetDateTime")
}

#[cfg(test)]
mod tests {
    use super::query::title_matches_search;

    #[test]
    fn web_title_search_is_case_insensitive() {
        assert!(title_matches_search("Roche Scales NVIDIA AI Factories", "sca"));
        assert!(title_matches_search("Roche Scales NVIDIA AI Factories", "SCA"));
        assert!(!title_matches_search("Roche Scales NVIDIA AI Factories", "xyz"));
    }
}
