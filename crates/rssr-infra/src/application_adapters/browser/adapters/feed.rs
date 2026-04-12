use std::sync::{Arc, Mutex};

use rssr_domain::{
    DomainError, Feed, FeedRepository, FeedSummary, NewFeedSubscription, normalize_feed_url,
};

use crate::application_adapters::browser::{
    now_utc,
    query::list_feeds as query_list_feeds,
    state::{BrowserState, PersistedFeed, save_state_snapshot},
};

use super::shared::map_persistence_error;

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

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}
