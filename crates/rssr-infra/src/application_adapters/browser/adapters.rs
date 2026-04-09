use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use reqwest::{StatusCode, header};
use rssr_application::{
    AppStatePort, FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate,
    FeedRemovalCleanupPort, OpmlCodecPort, ParsedEntryData, ParsedFeedUpdate, RefreshCommit,
    RefreshFailure, RefreshHttpMetadata, RefreshStorePort, RefreshTarget, RemoteConfigStore,
};
use rssr_domain::{
    DomainError, Entry, EntryNavigation, EntryQuery, EntryRepository, EntrySummary, Feed,
    FeedRepository, FeedSummary, NewFeedSubscription, SettingsRepository, UserSettings,
    normalize_feed_url,
};

use super::{
    config::{decode_opml, encode_opml, remote_url},
    feed::{ParsedEntry, ParsedFeed, parse_feed, web_fetch_feed_response},
    now_utc,
    query::{
        get_entry as query_get_entry, list_entries as query_list_entries,
        list_feeds as query_list_feeds, reader_navigation as query_reader_navigation,
    },
    state::{
        BrowserState, PersistedEntryFlag, PersistedFeed, last_opened_feed_id, save_app_state_slice,
        save_entry_flag_patch, save_state_snapshot, upsert_entries,
    },
};

#[derive(Clone)]
pub struct BrowserFeedRepository {
    state: Arc<Mutex<BrowserState>>,
}

impl BrowserFeedRepository {
    pub fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl FeedRepository for BrowserFeedRepository {
    async fn upsert_subscription(
        &self,
        new_feed: &NewFeedSubscription,
    ) -> rssr_domain::Result<Feed> {
        let normalized_url = normalize_feed_url(&new_feed.url);
        let normalized_title = normalize_optional_text(new_feed.title.clone());
        let normalized_folder = normalize_optional_text(new_feed.folder.clone());

        let (feed, snapshot) = {
            let mut state = self.state.lock().expect("lock state");
            let now = now_utc();

            if let Some(feed) =
                state.core.feeds.iter_mut().find(|feed| feed.url == normalized_url.as_str())
            {
                if new_feed.title.is_some() {
                    feed.title = normalized_title.clone();
                }
                if new_feed.folder.is_some() {
                    feed.folder = normalized_folder.clone();
                }
                feed.is_deleted = false;
                feed.updated_at = now;
                (feed.clone(), state.clone())
            } else {
                state.core.next_feed_id += 1;
                let persisted = PersistedFeed {
                    id: state.core.next_feed_id,
                    url: normalized_url.to_string(),
                    title: normalized_title,
                    site_url: None,
                    description: None,
                    icon_url: None,
                    folder: normalized_folder,
                    etag: None,
                    last_modified: None,
                    last_fetched_at: None,
                    last_success_at: None,
                    fetch_error: None,
                    is_deleted: false,
                    created_at: now,
                    updated_at: now,
                };
                state.core.feeds.push(persisted.clone());
                (persisted, state.clone())
            }
        };

        save_state_snapshot(snapshot).map_err(map_persistence_error)?;
        persisted_feed_to_domain(&feed)
    }

    async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> rssr_domain::Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let feed = state
                .core
                .feeds
                .iter_mut()
                .find(|feed| feed.id == feed_id)
                .ok_or(DomainError::NotFound)?;
            feed.is_deleted = is_deleted;
            feed.updated_at = now_utc();
            state.clone()
        };

        save_state_snapshot(snapshot).map_err(map_persistence_error)
    }

    async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
        let state = self.state.lock().expect("lock state");
        state
            .core
            .feeds
            .iter()
            .filter(|feed| !feed.is_deleted)
            .map(persisted_feed_to_domain)
            .collect()
    }

    async fn get_feed(&self, feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
        let state = self.state.lock().expect("lock state");
        state
            .core
            .feeds
            .iter()
            .find(|feed| feed.id == feed_id && !feed.is_deleted)
            .map(persisted_feed_to_domain)
            .transpose()
    }

    async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
        let state = self.state.lock().expect("lock state");
        Ok(query_list_feeds(&state))
    }
}

#[derive(Clone)]
pub struct BrowserEntryRepository {
    state: Arc<Mutex<BrowserState>>,
}

impl BrowserEntryRepository {
    pub fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl EntryRepository for BrowserEntryRepository {
    async fn list_entries(&self, query: &EntryQuery) -> rssr_domain::Result<Vec<EntrySummary>> {
        let state = self.state.lock().expect("lock state");
        Ok(query_list_entries(&state, query))
    }

    async fn get_entry(&self, entry_id: i64) -> rssr_domain::Result<Option<Entry>> {
        let state = self.state.lock().expect("lock state");
        query_get_entry(&state, entry_id).map_err(map_persistence_error)
    }

    async fn reader_navigation(
        &self,
        current_entry_id: i64,
    ) -> rssr_domain::Result<EntryNavigation> {
        let state = self.state.lock().expect("lock state");
        Ok(query_reader_navigation(&state, current_entry_id))
    }

    async fn set_read(&self, entry_id: i64, is_read: bool) -> rssr_domain::Result<()> {
        let entry = {
            let mut state = self.state.lock().expect("lock state");
            let now = now_utc();
            let entry = state.entry_flags.entries.iter_mut().find(|entry| entry.id == entry_id);

            if let Some(entry) = entry {
                entry.is_read = is_read;
                entry.read_at = is_read.then_some(now);
                entry.clone()
            } else {
                if !state.core.entries.iter().any(|entry| entry.id == entry_id) {
                    return Err(DomainError::NotFound);
                }
                let flag = PersistedEntryFlag {
                    id: entry_id,
                    is_read,
                    is_starred: false,
                    read_at: is_read.then_some(now),
                    starred_at: None,
                };
                state.entry_flags.entries.push(flag.clone());
                flag
            }
        };

        save_entry_flag_patch(entry).map_err(map_persistence_error)
    }

    async fn set_starred(&self, entry_id: i64, is_starred: bool) -> rssr_domain::Result<()> {
        let entry = {
            let mut state = self.state.lock().expect("lock state");
            let now = now_utc();
            let entry = state.entry_flags.entries.iter_mut().find(|entry| entry.id == entry_id);

            if let Some(entry) = entry {
                entry.is_starred = is_starred;
                entry.starred_at = is_starred.then_some(now);
                entry.clone()
            } else {
                if !state.core.entries.iter().any(|entry| entry.id == entry_id) {
                    return Err(DomainError::NotFound);
                }
                let flag = PersistedEntryFlag {
                    id: entry_id,
                    is_read: false,
                    is_starred,
                    read_at: None,
                    starred_at: is_starred.then_some(now),
                };
                state.entry_flags.entries.push(flag.clone());
                flag
            }
        };

        save_entry_flag_patch(entry).map_err(map_persistence_error)
    }

    async fn delete_for_feed(&self, feed_id: i64) -> rssr_domain::Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let removed_entry_ids = state
                .core
                .entries
                .iter()
                .filter(|entry| entry.feed_id == feed_id)
                .map(|entry| entry.id)
                .collect::<Vec<_>>();
            state.core.entries.retain(|entry| entry.feed_id != feed_id);
            state.entry_flags.entries.retain(|entry| !removed_entry_ids.contains(&entry.id));
            state.clone()
        };

        save_state_snapshot(snapshot).map_err(map_persistence_error)
    }
}

#[derive(Clone)]
pub struct BrowserAppStateAdapter {
    state: Arc<Mutex<BrowserState>>,
}

impl BrowserAppStateAdapter {
    pub fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state }
    }

    pub fn load_last_opened_feed_id(&self) -> Result<Option<i64>> {
        Ok(last_opened_feed_id(&self.state.lock().expect("lock state")))
    }

    pub fn save_last_opened_feed_id(&self, feed_id: Option<i64>) -> Result<()> {
        let last_opened_feed_id = {
            let mut state = self.state.lock().expect("lock state");
            state.app_state.last_opened_feed_id = feed_id;
            state.app_state.last_opened_feed_id
        };

        save_app_state_slice(last_opened_feed_id)
    }

    fn clear_last_opened_feed_if_matches_impl(&self, feed_id: i64) -> Result<()> {
        let last_opened_feed_id = {
            let mut state = self.state.lock().expect("lock state");
            if state.app_state.last_opened_feed_id != Some(feed_id) {
                return Ok(());
            }
            state.app_state.last_opened_feed_id = None;
            state.app_state.last_opened_feed_id
        };

        save_app_state_slice(last_opened_feed_id)
    }
}

#[async_trait::async_trait]
impl AppStatePort for BrowserAppStateAdapter {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
        self.clear_last_opened_feed_if_matches_impl(feed_id)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl FeedRemovalCleanupPort for BrowserAppStateAdapter {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
        self.clear_last_opened_feed_if_matches_impl(feed_id)
    }
}

#[derive(Clone)]
pub struct BrowserSettingsRepository {
    state: Arc<Mutex<BrowserState>>,
}

impl BrowserSettingsRepository {
    pub fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl SettingsRepository for BrowserSettingsRepository {
    async fn load(&self) -> rssr_domain::Result<UserSettings> {
        Ok(self.state.lock().expect("lock state").core.settings.clone())
    }

    async fn save(&self, settings: &UserSettings) -> rssr_domain::Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            state.core.settings = settings.clone();
            state.clone()
        };

        save_state_snapshot(snapshot).map_err(map_persistence_error)
    }
}

#[derive(Clone, Default)]
pub struct BrowserOpmlCodec;

impl OpmlCodecPort for BrowserOpmlCodec {
    fn encode(&self, feeds: &[rssr_domain::ConfigFeed]) -> Result<String> {
        encode_opml(feeds)
    }

    fn decode(&self, raw: &str) -> Result<Vec<rssr_domain::ConfigFeed>> {
        decode_opml(raw)
    }
}

#[derive(Clone)]
pub struct BrowserRemoteConfigStore {
    client: reqwest::Client,
    endpoint: String,
    remote_path: String,
}

impl BrowserRemoteConfigStore {
    pub fn new(client: reqwest::Client, endpoint: &str, remote_path: &str) -> Self {
        Self { client, endpoint: endpoint.to_string(), remote_path: remote_path.to_string() }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigStore for BrowserRemoteConfigStore {
    async fn upload_config(&self, raw: &str) -> Result<()> {
        self.client
            .put(remote_url(&self.endpoint, &self.remote_path)?)
            .header("content-type", "application/json")
            .body(raw.to_string())
            .send()
            .await
            .context("上传配置到 WebDAV 失败")?
            .error_for_status()
            .context("WebDAV 上传失败")?;
        Ok(())
    }

    async fn download_config(&self) -> Result<Option<String>> {
        let response = self
            .client
            .get(remote_url(&self.endpoint, &self.remote_path)?)
            .send()
            .await
            .context("从 WebDAV 下载配置失败")?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let raw = response.error_for_status().context("WebDAV 下载失败")?.text().await?;
        Ok(Some(raw))
    }
}

#[derive(Clone)]
pub struct BrowserFeedRefreshSource {
    client: reqwest::Client,
}

impl BrowserFeedRefreshSource {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl FeedRefreshSourcePort for BrowserFeedRefreshSource {
    async fn refresh(&self, target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
        let response = match web_fetch_feed_response(&self.client, target.url.as_str()).await {
            Ok(response) => response,
            Err(error) => {
                return Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                    message: format!("抓取订阅失败: {error}"),
                    metadata: None,
                }));
            }
        };

        let metadata = RefreshHttpMetadata {
            etag: response
                .headers()
                .get(header::ETAG)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            last_modified: response
                .headers()
                .get(header::LAST_MODIFIED)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
        };

        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(FeedRefreshSourceOutput::NotModified(metadata));
        }

        let body = match response.error_for_status() {
            Ok(response) => response.text().await.context("读取 feed 响应正文失败")?,
            Err(error) => {
                return Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                    message: format!("feed 抓取返回非成功状态: {error}"),
                    metadata: Some(metadata),
                }));
            }
        };

        match parse_feed(&body) {
            Ok(parsed) => Ok(FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
                metadata,
                feed: map_parsed_feed(parsed),
            })),
            Err(error) => Ok(FeedRefreshSourceOutput::Failed(RefreshFailure {
                message: format!("解析订阅失败: {error}"),
                metadata: Some(metadata),
            })),
        }
    }
}

#[derive(Clone)]
pub struct BrowserRefreshStore {
    state: Arc<Mutex<BrowserState>>,
}

impl BrowserRefreshStore {
    pub fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl RefreshStorePort for BrowserRefreshStore {
    async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
        let state = self.state.lock().expect("lock state");
        state
            .core
            .feeds
            .iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| {
                Ok(RefreshTarget {
                    feed_id: feed.id,
                    url: rssr_domain::normalize_feed_url(
                        &url::Url::parse(&feed.url).map_err(map_persistence_error)?,
                    ),
                    etag: feed.etag.clone(),
                    last_modified: feed.last_modified.clone(),
                })
            })
            .collect()
    }

    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
        let state = self.state.lock().expect("lock state");
        state
            .core
            .feeds
            .iter()
            .find(|feed| feed.id == feed_id && !feed.is_deleted)
            .map(|feed| {
                Ok(RefreshTarget {
                    feed_id: feed.id,
                    url: rssr_domain::normalize_feed_url(
                        &url::Url::parse(&feed.url).map_err(map_persistence_error)?,
                    ),
                    etag: feed.etag.clone(),
                    last_modified: feed.last_modified.clone(),
                })
            })
            .transpose()
    }

    async fn commit(&self, feed_id: i64, commit: RefreshCommit) -> Result<()> {
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let now = now_utc();
            let feed = state
                .core
                .feeds
                .iter_mut()
                .find(|feed| feed.id == feed_id)
                .context("订阅不存在")?;

            match commit {
                RefreshCommit::NotModified { metadata } => {
                    feed.etag = metadata.etag;
                    feed.last_modified = metadata.last_modified;
                    feed.last_fetched_at = Some(now);
                    feed.last_success_at = Some(now);
                    feed.fetch_error = None;
                    feed.updated_at = now;
                }
                RefreshCommit::Updated { update } => {
                    if update.feed.title.is_some() {
                        feed.title = update.feed.title;
                    }
                    if update.feed.site_url.is_some() {
                        feed.site_url = update.feed.site_url.map(|url| url.to_string());
                    }
                    if update.feed.description.is_some() {
                        feed.description = update.feed.description;
                    }
                    feed.etag = update.metadata.etag;
                    feed.last_modified = update.metadata.last_modified;
                    feed.last_fetched_at = Some(now);
                    feed.last_success_at = Some(now);
                    feed.fetch_error = None;
                    feed.updated_at = now;
                    upsert_entries(
                        &mut state.core,
                        feed_id,
                        map_application_entries(update.feed.entries),
                    )?;
                }
                RefreshCommit::Failed { failure } => {
                    if let Some(metadata) = failure.metadata {
                        feed.etag = metadata.etag;
                        feed.last_modified = metadata.last_modified;
                    }
                    feed.last_fetched_at = Some(now);
                    feed.fetch_error = Some(failure.message);
                    feed.updated_at = now;
                }
            }

            state.clone()
        };

        save_state_snapshot(snapshot)
    }
}

fn persisted_feed_to_domain(feed: &PersistedFeed) -> rssr_domain::Result<Feed> {
    Ok(Feed {
        id: feed.id,
        url: url::Url::parse(&feed.url).map_err(map_persistence_error)?,
        title: feed.title.clone(),
        site_url: feed
            .site_url
            .as_ref()
            .map(|raw| url::Url::parse(raw).map_err(map_persistence_error))
            .transpose()?,
        description: feed.description.clone(),
        icon_url: feed
            .icon_url
            .as_ref()
            .map(|raw| url::Url::parse(raw).map_err(map_persistence_error))
            .transpose()?,
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

fn map_parsed_feed(parsed: ParsedFeed) -> ParsedFeedUpdate {
    ParsedFeedUpdate {
        title: parsed.title,
        site_url: parsed.site_url,
        description: parsed.description,
        entries: parsed.entries.into_iter().map(map_parsed_entry).collect(),
    }
}

fn map_parsed_entry(entry: ParsedEntry) -> ParsedEntryData {
    ParsedEntryData {
        external_id: entry.external_id,
        dedup_key: entry.dedup_key,
        url: entry.url,
        title: entry.title,
        author: entry.author,
        summary: entry.summary,
        content_html: entry.content_html,
        content_text: entry.content_text,
        published_at: entry.published_at,
        updated_at_source: entry.updated_at_source,
    }
}

fn map_application_entries(entries: Vec<ParsedEntryData>) -> Vec<ParsedEntry> {
    entries
        .into_iter()
        .map(|entry| ParsedEntry {
            external_id: entry.external_id,
            dedup_key: entry.dedup_key,
            url: entry.url,
            title: entry.title,
            author: entry.author,
            summary: entry.summary,
            content_html: entry.content_html,
            content_text: entry.content_text,
            published_at: entry.published_at,
            updated_at_source: entry.updated_at_source,
        })
        .collect()
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn map_persistence_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::Persistence(error.to_string())
}
