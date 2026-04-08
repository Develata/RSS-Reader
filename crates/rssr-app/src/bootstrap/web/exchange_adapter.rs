use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use reqwest::StatusCode;
use rssr_application::{
    FeedRemovalCleanupPort, ImportExportService, OpmlCodecPort, RemoteConfigStore,
};
use rssr_domain::{
    DomainError, Entry, EntryNavigation, EntryQuery, EntryRepository, Feed, FeedRepository,
    FeedSummary, NewFeedSubscription, Result as DomainResult, SettingsRepository, UserSettings,
    normalize_feed_url,
};
use tokio::sync::oneshot;
use url::Url;
use wasm_bindgen_futures::spawn_local;

use super::{
    config::{decode_opml, encode_opml, remote_url},
    state::{PersistedFeed, PersistedState, save_state_snapshot},
    web_now_utc,
};

pub(super) fn build_import_export_service(
    state: Arc<Mutex<PersistedState>>,
) -> ImportExportService {
    let store =
        Arc::new(BrowserImportExportStore::new(state, Arc::new(LocalStorageSnapshotWriter)));
    ImportExportService::new_with_feed_removal_cleanup(
        store.clone(),
        store.clone(),
        store.clone(),
        Arc::new(BrowserOpmlCodec),
        store,
    )
}

trait SnapshotWriter: Send + Sync {
    fn write(&self, state: PersistedState) -> Result<()>;
}

struct LocalStorageSnapshotWriter;

impl SnapshotWriter for LocalStorageSnapshotWriter {
    fn write(&self, state: PersistedState) -> Result<()> {
        save_state_snapshot(state)
    }
}

#[derive(Clone)]
struct BrowserImportExportStore {
    state: Arc<Mutex<PersistedState>>,
    writer: Arc<dyn SnapshotWriter>,
}

impl BrowserImportExportStore {
    fn new(state: Arc<Mutex<PersistedState>>, writer: Arc<dyn SnapshotWriter>) -> Self {
        Self { state, writer }
    }

    fn write_snapshot(&self, state: PersistedState) -> DomainResult<()> {
        self.writer
            .write(state)
            .map_err(|error| DomainError::Persistence(format!("写入浏览器本地状态失败：{error}")))
    }

    fn map_feed(feed: &PersistedFeed) -> DomainResult<Feed> {
        let url = Url::parse(&feed.url).map_err(|error| {
            DomainError::Persistence(format!("订阅 URL 已损坏：{} ({error})", feed.url))
        })?;

        Ok(Feed {
            id: feed.id,
            url,
            title: feed.title.clone(),
            site_url: parse_optional_feed_url(feed.id, "site_url", &feed.site_url),
            description: feed.description.clone(),
            icon_url: parse_optional_feed_url(feed.id, "icon_url", &feed.icon_url),
            folder: feed.folder.clone(),
            etag: feed.etag.clone(),
            last_modified: feed.last_modified.clone(),
            last_fetched_at: feed.last_fetched_at,
            last_success_at: feed.last_success_at,
            fetch_error: feed.fetch_error.clone(),
            is_deleted: feed.is_deleted,
            created_at: feed.created_at,
            updated_at: feed.updated_at,
        })
    }
}

fn parse_optional_feed_url(feed_id: i64, field_name: &str, raw: &Option<String>) -> Option<Url> {
    raw.as_deref().and_then(|raw| match Url::parse(raw) {
        Ok(url) => Some(url),
        Err(error) => {
            tracing::warn!(
                feed_id,
                field = field_name,
                raw_url = raw,
                error = %error,
                "浏览器本地状态中的 feed 附加 URL 已损坏，已忽略该字段"
            );
            None
        }
    })
}

#[async_trait::async_trait]
impl FeedRepository for BrowserImportExportStore {
    async fn upsert_subscription(&self, new_feed: &NewFeedSubscription) -> DomainResult<Feed> {
        let normalized_url = normalize_feed_url(&new_feed.url);
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let now = web_now_utc();

            if let Some(existing) =
                state.feeds.iter_mut().find(|feed| feed.url == normalized_url.as_str())
            {
                if let Some(title) = new_feed.title.as_ref() {
                    existing.title = (!title.is_empty()).then_some(title.clone());
                }
                if let Some(folder) = new_feed.folder.as_ref() {
                    existing.folder = (!folder.is_empty()).then_some(folder.clone());
                }
                existing.is_deleted = false;
                existing.updated_at = now;
            } else {
                state.next_feed_id += 1;
                let feed_id = state.next_feed_id;
                state.feeds.push(PersistedFeed {
                    id: feed_id,
                    url: normalized_url.to_string(),
                    title: new_feed.title.clone(),
                    site_url: None,
                    description: None,
                    icon_url: None,
                    folder: new_feed.folder.clone(),
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

            state.clone()
        };

        let feed = snapshot
            .feeds
            .iter()
            .find(|feed| feed.url == normalized_url.as_str() && !feed.is_deleted)
            .ok_or_else(|| DomainError::Persistence("订阅写入后未找到对应记录".to_string()))
            .and_then(Self::map_feed)?;
        self.write_snapshot(snapshot)?;
        Ok(feed)
    }

    async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> DomainResult<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let feed = state
                .feeds
                .iter_mut()
                .find(|feed| feed.id == feed_id)
                .ok_or(DomainError::NotFound)?;
            feed.is_deleted = is_deleted;
            feed.updated_at = web_now_utc();
            state.clone()
        };
        self.write_snapshot(snapshot)
    }

    async fn list_feeds(&self) -> DomainResult<Vec<Feed>> {
        let mut parsed_feeds = Vec::new();
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let mut invalid_feed_ids = HashSet::new();
            for feed in state.feeds.iter().filter(|feed| !feed.is_deleted) {
                match Self::map_feed(feed) {
                    Ok(parsed) => parsed_feeds.push(parsed),
                    Err(error) => {
                        tracing::warn!(
                            feed_id = feed.id,
                            feed_url = %feed.url,
                            error = %error,
                            "浏览器本地状态中的订阅 URL 已损坏，导入导出前会清理该订阅"
                        );
                        invalid_feed_ids.insert(feed.id);
                    }
                }
            }

            if invalid_feed_ids.is_empty() {
                None
            } else {
                let now = web_now_utc();
                for feed in
                    state.feeds.iter_mut().filter(|feed| invalid_feed_ids.contains(&feed.id))
                {
                    feed.is_deleted = true;
                    feed.updated_at = now;
                }
                state.entries.retain(|entry| !invalid_feed_ids.contains(&entry.feed_id));
                if state
                    .last_opened_feed_id
                    .as_ref()
                    .is_some_and(|feed_id| invalid_feed_ids.contains(feed_id))
                {
                    state.last_opened_feed_id = None;
                }
                Some(state.clone())
            }
        };

        if let Some(snapshot) = snapshot {
            self.write_snapshot(snapshot)?;
        }

        Ok(parsed_feeds)
    }

    async fn get_feed(&self, feed_id: i64) -> DomainResult<Option<Feed>> {
        let state = self.state.lock().expect("lock state");
        let feed = state.feeds.iter().find(|feed| feed.id == feed_id && !feed.is_deleted);
        match feed {
            Some(feed) => Self::map_feed(feed).map(Some),
            None => Ok(None),
        }
    }

    async fn list_summaries(&self) -> DomainResult<Vec<FeedSummary>> {
        let state = self.state.lock().expect("lock state");
        Ok(state
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
            .collect())
    }
}

#[async_trait::async_trait]
impl EntryRepository for BrowserImportExportStore {
    async fn list_entries(
        &self,
        _query: &EntryQuery,
    ) -> DomainResult<Vec<rssr_domain::EntrySummary>> {
        Ok(Vec::new())
    }

    async fn get_entry(&self, _entry_id: i64) -> DomainResult<Option<Entry>> {
        Ok(None)
    }

    async fn reader_navigation(&self, _current_entry_id: i64) -> DomainResult<EntryNavigation> {
        Ok(EntryNavigation::default())
    }

    async fn set_read(&self, _entry_id: i64, _is_read: bool) -> DomainResult<()> {
        Ok(())
    }

    async fn set_starred(&self, _entry_id: i64, _is_starred: bool) -> DomainResult<()> {
        Ok(())
    }

    async fn delete_for_feed(&self, feed_id: i64) -> DomainResult<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            state.entries.retain(|entry| entry.feed_id != feed_id);
            state.clone()
        };
        self.write_snapshot(snapshot)
    }
}

#[async_trait::async_trait]
impl SettingsRepository for BrowserImportExportStore {
    async fn load(&self) -> DomainResult<UserSettings> {
        Ok(self.state.lock().expect("lock state").settings.clone())
    }

    async fn save(&self, settings: &UserSettings) -> DomainResult<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            state.settings = settings.clone();
            state.clone()
        };
        self.write_snapshot(snapshot)
    }
}

#[async_trait::async_trait]
impl FeedRemovalCleanupPort for BrowserImportExportStore {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            if state.last_opened_feed_id == Some(feed_id) {
                state.last_opened_feed_id = None;
                Some(state.clone())
            } else {
                None
            }
        };
        if let Some(snapshot) = snapshot {
            self.writer.write(snapshot).context("写入浏览器本地状态失败")?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct BrowserOpmlCodec;

impl OpmlCodecPort for BrowserOpmlCodec {
    fn encode(&self, feeds: &[rssr_domain::ConfigFeed]) -> Result<String> {
        encode_opml(feeds)
    }

    fn decode(&self, raw: &str) -> Result<Vec<rssr_domain::ConfigFeed>> {
        decode_opml(raw)
    }
}

pub(super) struct WebRemoteConfigStore {
    client: reqwest::Client,
    url: Url,
}

impl WebRemoteConfigStore {
    pub(super) fn new(client: reqwest::Client, endpoint: &str, remote_path: &str) -> Result<Self> {
        Ok(Self { client, url: remote_url(endpoint, remote_path)? })
    }
}

#[async_trait::async_trait]
impl RemoteConfigStore for WebRemoteConfigStore {
    async fn upload_config(&self, raw: &str) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        let client = self.client.clone();
        let url = self.url.clone();
        let raw = raw.to_string();
        spawn_local(async move {
            let result = async {
                client
                    .put(url)
                    .header("content-type", "application/json")
                    .body(raw)
                    .send()
                    .await
                    .context("上传配置到 WebDAV 失败")?
                    .error_for_status()
                    .context("WebDAV 上传失败")?;
                Ok(())
            }
            .await;
            let _ = sender.send(result);
        });

        match receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("Web 配置上传任务意外终止"),
        }
    }

    async fn download_config(&self) -> Result<Option<String>> {
        let (sender, receiver) = oneshot::channel();
        let client = self.client.clone();
        let url = self.url.clone();
        spawn_local(async move {
            let result = async {
                let response = client.get(url).send().await.context("从 WebDAV 下载配置失败")?;
                if response.status() == StatusCode::NOT_FOUND {
                    return Ok(None);
                }

                let raw = response
                    .error_for_status()
                    .context("WebDAV 下载失败")?
                    .text()
                    .await
                    .context("读取远端配置正文失败")?;
                Ok(Some(raw))
            }
            .await;
            let _ = sender.send(result);
        });

        match receiver.await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("Web 配置下载任务意外终止"),
        }
    }
}
