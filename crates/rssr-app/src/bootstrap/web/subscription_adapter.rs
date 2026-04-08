use std::sync::{Arc, Mutex};

use anyhow::Result;
use rssr_application::{AppStatePort, FeedService, RefreshService, SubscriptionWorkflow};
use rssr_domain::{
    DomainError, Entry, EntryNavigation, EntryQuery, EntryRepository, Feed, FeedRepository,
    FeedSummary, NewFeedSubscription, Result as DomainResult, normalize_feed_url,
};
use url::Url;

use super::{
    state::{PersistedFeed, PersistedState, save_state_snapshot},
    web_now_utc,
};

pub(super) fn build_subscription_workflow(
    state: Arc<Mutex<PersistedState>>,
    refresh_service: RefreshService,
) -> SubscriptionWorkflow {
    let store =
        Arc::new(BrowserSubscriptionStore::new(state, Arc::new(LocalStorageSnapshotWriter)));
    let feed_service = FeedService::new(store.clone(), store.clone());
    SubscriptionWorkflow::new(feed_service, refresh_service, store)
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
struct BrowserSubscriptionStore {
    state: Arc<Mutex<PersistedState>>,
    writer: Arc<dyn SnapshotWriter>,
}

impl BrowserSubscriptionStore {
    fn new(state: Arc<Mutex<PersistedState>>, writer: Arc<dyn SnapshotWriter>) -> Self {
        Self { state, writer }
    }

    fn write_snapshot(&self, state: PersistedState) -> DomainResult<()> {
        self.writer
            .write(state)
            .map_err(|error| DomainError::Persistence(format!("写入浏览器本地状态失败：{error}")))
    }

    fn map_feed(feed: &PersistedFeed) -> DomainResult<Feed> {
        Ok(Feed {
            id: feed.id,
            url: Url::parse(&feed.url)
                .map_err(|error| DomainError::Persistence(format!("订阅 URL 已损坏：{error}")))?,
            title: feed.title.clone(),
            site_url: parse_optional_url(&feed.site_url),
            description: feed.description.clone(),
            icon_url: parse_optional_url(&feed.icon_url),
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

fn parse_optional_url(raw: &Option<String>) -> Option<Url> {
    raw.as_deref().and_then(|raw| Url::parse(raw).ok())
}

#[async_trait::async_trait]
impl FeedRepository for BrowserSubscriptionStore {
    async fn upsert_subscription(&self, new_feed: &NewFeedSubscription) -> DomainResult<Feed> {
        let normalized_url = normalize_feed_url(&new_feed.url);
        let snapshot = {
            let mut state = self.state.lock().expect("lock state");
            let now = web_now_utc();

            if let Some(feed) =
                state.feeds.iter_mut().find(|feed| feed.url == normalized_url.as_str())
            {
                if let Some(title) = new_feed.title.as_ref() {
                    feed.title = (!title.is_empty()).then_some(title.clone());
                }
                if let Some(folder) = new_feed.folder.as_ref() {
                    feed.folder = (!folder.is_empty()).then_some(folder.clone());
                }
                feed.is_deleted = false;
                feed.updated_at = now;
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
            .ok_or_else(|| DomainError::Persistence("写入后未找到订阅".to_string()))
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
        let state = self.state.lock().expect("lock state");
        state.feeds.iter().filter(|feed| !feed.is_deleted).map(Self::map_feed).collect()
    }

    async fn get_feed(&self, feed_id: i64) -> DomainResult<Option<Feed>> {
        let state = self.state.lock().expect("lock state");
        state
            .feeds
            .iter()
            .find(|feed| feed.id == feed_id && !feed.is_deleted)
            .map(Self::map_feed)
            .transpose()
    }

    async fn list_summaries(&self) -> DomainResult<Vec<FeedSummary>> {
        Ok(Vec::new())
    }
}

#[async_trait::async_trait]
impl EntryRepository for BrowserSubscriptionStore {
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
impl AppStatePort for BrowserSubscriptionStore {
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
            self.writer.write(snapshot)?;
        }
        Ok(())
    }
}
