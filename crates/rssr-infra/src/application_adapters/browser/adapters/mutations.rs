use anyhow::{Context, Result};
use rssr_application::{
    ConfigReplacementFeed, ConfigReplacementOutcome, ConfigReplacementPlan, ConfigReplacementPort,
    SubscriptionRemovalPort,
};

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

                let mut changes = Changes::CORE;
                if state.app_state.last_opened_feed_id == Some(feed_id) {
                    state.app_state.last_opened_feed_id = None;
                    changes = changes | Changes::APP_STATE;
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

                    let flags_before = state.entry_flags.entries.len();
                    state.entry_flags.entries.retain(|entry| !removed_ids.contains(&entry.id));
                    if state.entry_flags.entries.len() != flags_before {
                        changes = changes | Changes::FLAGS;
                    }

                    let content_before = state.entry_content.entries.len();
                    state.entry_content.entries.retain(|entry| entry.feed_id != feed_id);
                    if state.entry_content.entries.len() != content_before {
                        changes = changes | Changes::CONTENT;
                    }
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
    async fn replace_config(
        &self,
        plan: ConfigReplacementPlan,
    ) -> Result<ConfigReplacementOutcome> {
        self.store
            .update(move |state| {
                let desired_urls = plan
                    .feeds
                    .iter()
                    .map(|feed| feed.url.as_str())
                    .collect::<std::collections::HashSet<_>>();
                let removed = state
                    .core
                    .feeds
                    .iter()
                    .filter(|feed| !feed.is_deleted && !desired_urls.contains(feed.url.as_str()))
                    .map(|feed| feed.id)
                    .collect::<std::collections::HashSet<_>>();
                let removed_feed_count = removed.len();
                let settings_updated = state.core.settings != plan.settings;

                for feed in &plan.feeds {
                    upsert_config_feed(state, feed);
                }

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
                state.core.settings = plan.settings;

                let mut changes = Changes::CORE;
                let flags_before = state.entry_flags.entries.len();
                state.entry_flags.entries.retain(|entry| !removed_entry_ids.contains(&entry.id));
                if state.entry_flags.entries.len() != flags_before {
                    changes = changes | Changes::FLAGS;
                }

                let content_before = state.entry_content.entries.len();
                state.entry_content.entries.retain(|entry| !removed.contains(&entry.feed_id));
                if state.entry_content.entries.len() != content_before {
                    changes = changes | Changes::CONTENT;
                }

                if state
                    .app_state
                    .last_opened_feed_id
                    .is_some_and(|feed_id| removed.contains(&feed_id))
                {
                    state.app_state.last_opened_feed_id = None;
                    changes = changes | Changes::APP_STATE;
                }

                Ok((ConfigReplacementOutcome { removed_feed_count, settings_updated }, changes))
            })
            .await
            .map_err(map_store_error)
            .map_err(Into::into)
    }
}

fn upsert_config_feed(
    state: &mut crate::application_adapters::browser::state::BrowserState,
    new_feed: &ConfigReplacementFeed,
) {
    let normalized_title = normalize_optional_text(new_feed.title.clone());
    let normalized_folder = normalize_optional_text(new_feed.folder.clone());
    let now = now_utc();

    if let Some(feed) = state.core.feeds.iter_mut().find(|feed| feed.url == new_feed.url.as_str()) {
        feed.title = normalized_title;
        feed.folder = normalized_folder;
        feed.is_deleted = false;
        feed.updated_at = now;
        return;
    }

    state.core.next_feed_id += 1;
    state.core.feeds.push(PersistedFeed {
        id: state.core.next_feed_id,
        url: new_feed.url.to_string(),
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
    });
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}
