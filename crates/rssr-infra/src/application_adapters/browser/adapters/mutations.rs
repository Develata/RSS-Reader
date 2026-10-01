use anyhow::{Context, Result};
use rssr_application::{ConfigReplacementPlan, ConfigReplacementPort, SubscriptionRemovalPort};
use rssr_domain::NewFeedSubscription;

use crate::application_adapters::browser::{
    now_utc,
    state::{BrowserStore, Changes, PersistedFeed},
};

use super::shared::map_store_error;

#[derive(Clone)]
pub struct BrowserPersistenceMutations {
    store: BrowserStore,
}

impl BrowserPersistenceMutations {
    pub fn new(store: BrowserStore) -> Self {
        Self { store }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl SubscriptionRemovalPort for BrowserPersistenceMutations {
    async fn remove_subscription(&self, feed_id: i64, purge_entries: bool) -> Result<()> {
        self.store
            .update(move |state| {
                let feed = state
                    .core
                    .feeds
                    .iter_mut()
                    .find(|feed| feed.id == feed_id)
                    .context("订阅不存在")?;
                feed.is_deleted = true;
                feed.updated_at = now_utc();

                let mut changes = Changes::CORE | Changes::APP_STATE;
                if state.app_state.last_opened_feed_id == Some(feed_id) {
                    state.app_state.last_opened_feed_id = None;
                }

                if purge_entries {
                    let removed_ids = state
                        .core
                        .entries
                        .iter()
                        .filter(|entry| entry.feed_id == feed_id)
                        .map(|entry| entry.id)
                        .collect::<std::collections::HashSet<_>>();
                    state.core.entries.retain(|entry| entry.feed_id != feed_id);
                    state.entry_flags.entries.retain(|entry| !removed_ids.contains(&entry.id));
                    state.entry_content.entries.retain(|entry| entry.feed_id != feed_id);
                    changes = changes | Changes::FLAGS | Changes::CONTENT;
                }

                Ok(((), changes))
            })
            .await
            .map_err(map_store_error)
            .map_err(Into::into)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl ConfigReplacementPort for BrowserPersistenceMutations {
    async fn replace_config(&self, plan: ConfigReplacementPlan) -> Result<()> {
        self.store
            .update(move |state| {
                for new_feed in &plan.upserts {
                    upsert_subscription(state, new_feed);
                }

                let removed =
                    plan.removed_feed_ids.iter().copied().collect::<std::collections::HashSet<_>>();
                let removed_entry_ids = state
                    .core
                    .entries
                    .iter()
                    .filter(|entry| removed.contains(&entry.feed_id))
                    .map(|entry| entry.id)
                    .collect::<std::collections::HashSet<_>>();

                for feed in &mut state.core.feeds {
                    if removed.contains(&feed.id) {
                        feed.is_deleted = true;
                        feed.updated_at = now_utc();
                    }
                }
                state.core.entries.retain(|entry| !removed.contains(&entry.feed_id));
                state.entry_flags.entries.retain(|entry| !removed_entry_ids.contains(&entry.id));
                state.entry_content.entries.retain(|entry| !removed.contains(&entry.feed_id));
                state.core.settings = plan.settings;

                if state
                    .app_state
                    .last_opened_feed_id
                    .is_some_and(|feed_id| removed.contains(&feed_id))
                {
                    state.app_state.last_opened_feed_id = None;
                }

                Ok(((), Changes::CORE | Changes::APP_STATE | Changes::FLAGS | Changes::CONTENT))
            })
            .await
            .map_err(map_store_error)
            .map_err(Into::into)
    }
}

fn upsert_subscription(
    state: &mut crate::application_adapters::browser::state::BrowserState,
    new_feed: &NewFeedSubscription,
) {
    let normalized_title = normalize_optional_text(new_feed.title.clone());
    let normalized_folder = normalize_optional_text(new_feed.folder.clone());
    let now = now_utc();

    if let Some(feed) = state.core.feeds.iter_mut().find(|feed| feed.url == new_feed.url.as_str()) {
        if new_feed.title.is_some() {
            feed.title = normalized_title;
        }
        if new_feed.folder.is_some() {
            feed.folder = normalized_folder;
        }
        if let Some(site_url) = &new_feed.site_url {
            feed.site_url = Some(site_url.to_string());
        }
        feed.is_deleted = false;
        feed.updated_at = now;
        return;
    }

    state.core.next_feed_id += 1;
    state.core.feeds.push(PersistedFeed {
        id: state.core.next_feed_id,
        url: new_feed.url.to_string(),
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
    });
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}
