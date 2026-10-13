//! JSON documents kept in `app_settings`: user settings and app state.

use anyhow::Result;
use rssr_application::AppStatePort;
use rssr_domain::{
    AppStateMutation, AppStateRepository, AppStateSnapshot, DomainError, Result as DomainResult,
    SettingsRepository, UserSettings,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::codec::now;
use super::error::StoreResult;
use super::executor::LibraryStore;

pub(crate) const USER_SETTINGS_KEY: &str = "user_settings";
pub(crate) const APP_STATE_KEY: &str = "app_state_v2";

pub(crate) fn load_raw(conn: &Connection, key: &str) -> StoreResult<Option<String>> {
    Ok(conn
        .prepare_cached("SELECT value FROM app_settings WHERE key = ?1")?
        .query_row([key], |row| row.get(0))
        .optional()?)
}

/// The stored document, or its default when none was saved yet.
pub(crate) fn load<T: DeserializeOwned + Default>(conn: &Connection, key: &str) -> StoreResult<T> {
    match load_raw(conn, key)? {
        Some(raw) => serde_json::from_str(&raw)
            .map_err(|error| DomainError::Persistence(format!("{key} 无法解析：{error}")).into()),
        None => Ok(T::default()),
    }
}

pub(crate) fn save<T: Serialize>(
    conn: &Connection,
    key: &str,
    value: &T,
    now: &str,
) -> StoreResult<()> {
    let raw = serde_json::to_string(value)
        .map_err(|error| DomainError::Persistence(format!("{key} 无法序列化：{error}")))?;
    conn.prepare_cached(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )?
    .execute(params![key, raw, now])?;
    Ok(())
}

/// Clears the last opened subscription if it is one of `removed_feed_ids`.
///
/// App state is a convenience: when it cannot be read (fields from a newer version, damage),
/// this step is skipped rather than failing a removal or an import (audit A5). Startup checks
/// that the last opened subscription still exists.
pub(crate) fn clear_last_opened_if_removed(
    conn: &Connection,
    removed_feed_ids: &[i64],
    now: &str,
) -> StoreResult<()> {
    if removed_feed_ids.is_empty() {
        return Ok(());
    }
    let mut state: AppStateSnapshot = match load(conn, APP_STATE_KEY) {
        Ok(state) => state,
        Err(error) => {
            tracing::warn!(%error, "应用状态无法解析，跳过最后打开订阅的清理");
            return Ok(());
        }
    };
    if state.last_opened_feed_id.is_none_or(|feed_id| !removed_feed_ids.contains(&feed_id)) {
        return Ok(());
    }
    state.last_opened_feed_id = None;
    save(conn, APP_STATE_KEY, &state, now)
}

/// [`SettingsRepository`] over the library.
#[derive(Clone)]
pub struct LibrarySettings {
    store: LibraryStore,
}

impl LibrarySettings {
    pub fn new(store: LibraryStore) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl SettingsRepository for LibrarySettings {
    async fn load(&self) -> DomainResult<UserSettings> {
        Ok(self.store.read(|conn| load(conn, USER_SETTINGS_KEY)).await?)
    }

    async fn save(&self, settings: &UserSettings) -> DomainResult<()> {
        let settings = settings.clone();
        Ok(self.store.write(move |tx| save(tx, USER_SETTINGS_KEY, &settings, &now())).await?)
    }
}

/// [`AppStateRepository`] and [`AppStatePort`] over the library.
#[derive(Clone)]
pub struct LibraryAppState {
    store: LibraryStore,
}

impl LibraryAppState {
    pub fn new(store: LibraryStore) -> Self {
        Self { store }
    }

    /// Reads, changes and writes the whole state in one write transaction, so concurrent
    /// updates of different fields never overwrite each other with a stale copy (audit A2).
    /// Nothing is written when the state does not change.
    async fn update_with(
        &self,
        mutate: impl FnOnce(&mut AppStateSnapshot) + Send + 'static,
    ) -> StoreResult<()> {
        self.store
            .write(move |tx| {
                let before: AppStateSnapshot = load(tx, APP_STATE_KEY)?;
                let mut state = before.clone();
                mutate(&mut state);
                if state != before {
                    save(tx, APP_STATE_KEY, &state, &now())?;
                }
                Ok(())
            })
            .await
    }
}

#[async_trait::async_trait]
impl AppStateRepository for LibraryAppState {
    async fn load(&self) -> DomainResult<AppStateSnapshot> {
        Ok(self.store.read(|conn| load(conn, APP_STATE_KEY)).await?)
    }

    async fn update(&self, mutate: AppStateMutation) -> DomainResult<()> {
        Ok(self.update_with(mutate).await?)
    }
}

#[async_trait::async_trait]
impl AppStatePort for LibraryAppState {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
        Ok(self
            .update_with(move |state| {
                if state.last_opened_feed_id == Some(feed_id) {
                    state.last_opened_feed_id = None;
                }
            })
            .await?)
    }
}
