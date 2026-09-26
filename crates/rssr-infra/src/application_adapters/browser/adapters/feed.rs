use rssr_domain::{
    DomainError, Feed, FeedRepository, FeedSummary, NewFeedSubscription, normalize_feed_url,
};

use crate::application_adapters::browser::{
    now_utc,
    query::list_feeds as query_list_feeds,
    state::{BrowserStore, Changes, PersistedFeed},
};

use super::shared::{map_persistence_error, map_store_error};

#[derive(Clone)]
pub struct BrowserFeedRepository {
    store: BrowserStore,
}

impl BrowserFeedRepository {
    pub fn new(store: BrowserStore) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl FeedRepository for BrowserFeedRepository {
    async fn upsert_subscription(
        &self,
        new_feed: &NewFeedSubscription,
    ) -> rssr_domain::Result<Feed> {
        let new_feed = new_feed.clone();
        let normalized_url = normalize_feed_url(&new_feed.url);
        let normalized_title = normalize_optional_text(new_feed.title.clone());
        let normalized_folder = normalize_optional_text(new_feed.folder.clone());

        self.store
            .update(move |state| {
                let now = now_utc();

                let feed = if let Some(feed) =
                    state.core.feeds.iter_mut().find(|feed| feed.url == normalized_url.as_str())
                {
                    if new_feed.title.is_some() {
                        feed.title = normalized_title.clone();
                    }
                    if new_feed.folder.is_some() {
                        feed.folder = normalized_folder.clone();
                    }
                    if let Some(site_url) = &new_feed.site_url {
                        feed.site_url = Some(site_url.to_string());
                    }
                    feed.is_deleted = false;
                    feed.updated_at = now;
                    feed.clone()
                } else {
                    state.core.next_feed_id += 1;
                    let persisted = PersistedFeed {
                        id: state.core.next_feed_id,
                        url: normalized_url.to_string(),
                        title: normalized_title,
                        site_url: new_feed.site_url.as_ref().map(ToString::to_string),
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
                    persisted
                };

                Ok((persisted_feed_to_domain(&feed)?, Changes::CORE))
            })
            .await
            .map_err(map_store_error)
    }

    async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> rssr_domain::Result<()> {
        self.store
            .update(move |state| {
                let feed = state
                    .core
                    .feeds
                    .iter_mut()
                    .find(|feed| feed.id == feed_id)
                    .ok_or(DomainError::NotFound)?;
                feed.is_deleted = is_deleted;
                feed.updated_at = now_utc();
                Ok(((), Changes::CORE))
            })
            .await
            .map_err(map_store_error)
    }

    async fn list_feeds(&self) -> rssr_domain::Result<Vec<Feed>> {
        self.store
            .read(|state| {
                Ok(state
                    .core
                    .feeds
                    .iter()
                    .filter(|feed| !feed.is_deleted)
                    .map(persisted_feed_to_domain)
                    .collect::<rssr_domain::Result<Vec<_>>>()?)
            })
            .await
            .map_err(map_store_error)
    }

    async fn get_feed(&self, feed_id: i64) -> rssr_domain::Result<Option<Feed>> {
        self.store
            .read(move |state| {
                Ok(state
                    .core
                    .feeds
                    .iter()
                    .find(|feed| feed.id == feed_id && !feed.is_deleted)
                    .map(persisted_feed_to_domain)
                    .transpose()?)
            })
            .await
            .map_err(map_store_error)
    }

    async fn list_summaries(&self) -> rssr_domain::Result<Vec<FeedSummary>> {
        self.store.read(|state| Ok(query_list_feeds(state))).await.map_err(map_store_error)
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
