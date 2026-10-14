//! Subscription activation and removal, and configuration replacement. Each is one write
//! transaction: in one library file, removing a subscription's entries removes their bodies
//! too (foreign-key cascade), so no cleanup is left for later.

use std::collections::HashSet;

use anyhow::{Context, Result};
use rssr_application::{
    ActivatedSubscription, ConfigReplacementFeed, ConfigReplacementOutcome, ConfigReplacementPlan,
    ConfigReplacementPort, SubscriptionActivationPort, SubscriptionRemovalPort,
};
use rssr_domain::{DomainError, NewFeedSubscription, UserSettings};
use rusqlite::{Connection, params};

use super::codec::now;
use super::documents::{self, USER_SETTINGS_KEY};
use super::entries;
use super::error::{StoreError, StoreResult};
use super::executor::LibraryStore;
use super::feeds;
use super::refresh::issue_ticket;

fn mark_removed(conn: &Connection, feed_id: i64, now: &str) -> StoreResult<()> {
    let changed = conn
        .prepare_cached("UPDATE feeds SET is_deleted = 1, updated_at = ?2 WHERE id = ?1")?
        .execute(params![feed_id, now])?;
    if changed == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

fn remove_subscription(
    conn: &Connection,
    feed_id: i64,
    purge_entries: bool,
    now: &str,
) -> StoreResult<()> {
    mark_removed(conn, feed_id, now)?;
    if purge_entries {
        entries::delete_for_feed(conn, feed_id)?;
    }
    documents::clear_last_opened_if_removed(conn, &[feed_id], now)
}

/// Saves one subscription from a configuration: its name is the chosen name (set when present,
/// cleared when absent); the source title stays with refreshes (audit B6).
fn upsert_config_feed(
    conn: &Connection,
    feed: &ConfigReplacementFeed,
    now: &str,
) -> StoreResult<()> {
    let title = feed.title.as_deref().map(str::trim).filter(|value| !value.is_empty());
    let folder = feed.folder.as_deref().map(str::trim).filter(|value| !value.is_empty());
    conn.prepare_cached(
        "INSERT INTO feeds (url, custom_title, folder, created_at, updated_at, site_url)
         VALUES (?1, ?2, ?3, ?4, ?4, NULL)
         ON CONFLICT(url) DO UPDATE SET
             custom_title = excluded.custom_title,
             folder = excluded.folder,
             site_url = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.site_url END,
             description = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.description END,
             icon_url = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.icon_url END,
             etag = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.etag END,
             last_modified = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.last_modified END,
             last_fetched_at = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.last_fetched_at END,
             last_success_at = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.last_success_at END,
             fetch_error = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.fetch_error END,
             generation = CASE WHEN feeds.is_deleted = 1 THEN feeds.generation + 1
                               ELSE feeds.generation END,
             is_deleted = 0,
             updated_at = excluded.updated_at",
    )?
    .execute(params![feed.url.as_str(), title, folder, now])?;
    Ok(())
}

/// Shown when a replacement finds the configuration changed since it was read.
pub(crate) const CONFIG_CHANGED: &str = "配置在读取后已被修改，请重新读取后再替换";

/// Makes the library's subscriptions and settings equal to `plan`, unless the configuration no
/// longer matches `plan.expected`. The removals are derived
/// inside the write transaction, from the state it locked.
fn replace_config(
    conn: &Connection,
    plan: &ConfigReplacementPlan,
    now: &str,
) -> StoreResult<ConfigReplacementOutcome> {
    // One read of the current configuration serves the conflict check and the removal diff.
    let current = feeds::config_rows(conn)?;
    let current_settings: UserSettings = documents::load(conn, USER_SETTINGS_KEY)?;
    if feeds::fingerprint_of(&current, &current_settings) != plan.expected {
        return Err(DomainError::Conflict(CONFIG_CHANGED.to_string()).into());
    }
    let desired: HashSet<&str> = plan.feeds.iter().map(|feed| feed.url.as_str()).collect();
    let removed_feed_ids: Vec<i64> = current
        .iter()
        .filter(|row| !desired.contains(row.url.as_str()))
        .map(|row| row.id)
        .collect();
    let settings_updated = current_settings != plan.settings;

    for feed in &plan.feeds {
        upsert_config_feed(conn, feed, now)?;
    }
    for &feed_id in &removed_feed_ids {
        mark_removed(conn, feed_id, now)?;
        entries::delete_for_feed(conn, feed_id)?;
    }
    documents::save(conn, USER_SETTINGS_KEY, &plan.settings, now)?;
    documents::clear_last_opened_if_removed(conn, &removed_feed_ids, now)?;
    Ok(ConfigReplacementOutcome {
        removed_feed_count: removed_feed_ids.len(),
        settings_updated,
        settings: plan.settings.clone(),
    })
}

/// Activation, removal and configuration replacement over the library.
#[derive(Clone)]
pub struct LibraryMutations {
    store: LibraryStore,
}

impl LibraryMutations {
    pub fn new(store: LibraryStore) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl SubscriptionActivationPort for LibraryMutations {
    async fn activate_subscription(
        &self,
        new_feed: NewFeedSubscription,
    ) -> Result<ActivatedSubscription> {
        // The add flow's prepared fetch gets its ticket here, before any other refresh can see
        // the subscription.
        let (row, ticket) = self
            .store
            .write(move |tx| {
                let row = feeds::activate_new_subscription(tx, &new_feed, &now())?;
                let ticket = issue_ticket(tx, row.feed.id)?;
                Ok((row, ticket))
            })
            .await
            .map_err(rssr_domain::DomainError::from)?;
        Ok(ActivatedSubscription { feed: row.feed, generation: row.generation, ticket })
    }
}

#[async_trait::async_trait]
impl SubscriptionRemovalPort for LibraryMutations {
    async fn remove_subscription(&self, feed_id: i64, purge_entries: bool) -> Result<()> {
        self.store
            .write(move |tx| remove_subscription(tx, feed_id, purge_entries, &now()))
            .await
            .map_err(DomainError::from)
            .context("删除订阅失败")
    }
}

#[async_trait::async_trait]
impl ConfigReplacementPort for LibraryMutations {
    async fn replace_config(
        &self,
        plan: ConfigReplacementPlan,
    ) -> Result<ConfigReplacementOutcome> {
        // As a domain error, so a conflict is recognizable through the context.
        self.store
            .write(move |tx| replace_config(tx, &plan, &now()))
            .await
            .map_err(DomainError::from)
            .context("替换配置失败")
    }
}
