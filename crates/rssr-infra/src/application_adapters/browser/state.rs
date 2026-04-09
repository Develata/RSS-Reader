use anyhow::Context;
use js_sys::Date;
use rssr_domain::Entry;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;
use web_sys::{Storage, window};

use super::{
    feed::{ParsedEntry, hash_content},
    now_utc,
};

pub const STORAGE_KEY: &str = "rssr-web-state-v1";
pub const APP_STATE_STORAGE_KEY: &str = "rssr-web-app-state-v1";
pub const ENTRY_FLAGS_STORAGE_KEY: &str = "rssr-web-entry-flags-v1";

#[derive(Debug, Default, Clone)]
pub struct BrowserState {
    pub core: PersistedState,
    pub app_state: PersistedAppStateSlice,
    pub entry_flags: PersistedEntryFlagsSlice,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PersistedState {
    pub next_feed_id: i64,
    pub next_entry_id: i64,
    pub feeds: Vec<PersistedFeed>,
    pub entries: Vec<PersistedEntry>,
    pub settings: rssr_domain::UserSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedFeed {
    pub id: i64,
    pub url: String,
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub folder: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_fetched_at: Option<OffsetDateTime>,
    pub last_success_at: Option<OffsetDateTime>,
    pub fetch_error: Option<String>,
    pub is_deleted: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedEntry {
    pub id: i64,
    pub feed_id: i64,
    pub external_id: String,
    pub dedup_key: String,
    pub url: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub published_at: Option<OffsetDateTime>,
    pub updated_at_source: Option<OffsetDateTime>,
    pub first_seen_at: OffsetDateTime,
    pub content_hash: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct LoadedState {
    pub state: BrowserState,
    pub warning: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PersistedAppStateSlice {
    pub last_opened_feed_id: Option<i64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PersistedEntryFlagsSlice {
    pub entries: Vec<PersistedEntryFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedEntryFlag {
    pub id: i64,
    pub is_read: bool,
    pub is_starred: bool,
    pub read_at: Option<OffsetDateTime>,
    pub starred_at: Option<OffsetDateTime>,
}

pub fn load_state() -> LoadedState {
    let Some(storage) = browser_storage() else {
        return LoadedState { state: BrowserState::default(), warning: None };
    };

    let raw = match storage.get_item(STORAGE_KEY) {
        Ok(Some(raw)) => raw,
        Ok(None) => {
            return LoadedState { state: BrowserState::default(), warning: None };
        }
        Err(_) => {
            return LoadedState {
                state: BrowserState::default(),
                warning: Some("读取浏览器本地存储失败，已使用空状态启动。".to_string()),
            };
        }
    };

    match serde_json::from_str(&raw).context("解析浏览器本地状态失败") {
        Ok(core) => LoadedState {
            state: BrowserState {
                core,
                app_state: load_app_state_slice(&storage).unwrap_or_default(),
                entry_flags: load_entry_flags_slice(&storage).unwrap_or_default(),
            },
            warning: None,
        },
        Err(error) => {
            backup_corrupt_state(&storage, &raw);
            let _ = storage.remove_item(STORAGE_KEY);
            LoadedState {
                state: BrowserState::default(),
                warning: Some(format!(
                    "浏览器本地状态已损坏，已保留损坏副本并使用空状态启动：{error}"
                )),
            }
        }
    }
}

pub fn save_state_snapshot(state: BrowserState) -> anyhow::Result<()> {
    save_serialized_state(serde_json::to_string(&state.core)?)?;
    save_app_state_slice_internal(&state.app_state)?;
    save_entry_flags_slice_internal(&state.entry_flags)?;
    Ok(())
}

pub fn save_app_state_slice(last_opened_feed_id: Option<i64>) -> anyhow::Result<()> {
    save_app_state_slice_internal(&PersistedAppStateSlice { last_opened_feed_id })
}

pub fn save_entry_flag_patch(flag: PersistedEntryFlag) -> anyhow::Result<()> {
    let Some(storage) = browser_storage() else {
        return Ok(());
    };

    let mut slice = load_entry_flags_slice(&storage).unwrap_or_default();

    if let Some(existing) = slice.entries.iter_mut().find(|current| current.id == flag.id) {
        *existing = flag;
    } else {
        slice.entries.push(flag);
    }

    save_storage_key(&storage, ENTRY_FLAGS_STORAGE_KEY, serde_json::to_string(&slice)?)
}

fn save_serialized_state(raw: String) -> anyhow::Result<()> {
    let Some(storage) = browser_storage() else {
        return Ok(());
    };
    save_storage_key(&storage, STORAGE_KEY, raw)?;
    Ok(())
}

fn backup_corrupt_state(storage: &Storage, raw: &str) {
    let backup_key = format!("{STORAGE_KEY}-corrupt-{}", Date::now() as i64);
    let _ = storage.set_item(&backup_key, raw);
}

fn browser_storage() -> Option<Storage> {
    window().and_then(|window| window.local_storage().ok()).flatten()
}

fn save_storage_key(storage: &Storage, key: &str, raw: String) -> anyhow::Result<()> {
    storage.set_item(key, &raw).map_err(|_| anyhow::anyhow!("写入浏览器本地存储失败"))
}

fn save_app_state_slice_internal(slice: &PersistedAppStateSlice) -> anyhow::Result<()> {
    let Some(storage) = browser_storage() else {
        return Ok(());
    };

    let raw = serde_json::to_string(slice)?;
    save_storage_key(&storage, APP_STATE_STORAGE_KEY, raw)
}

fn save_entry_flags_slice_internal(slice: &PersistedEntryFlagsSlice) -> anyhow::Result<()> {
    let Some(storage) = browser_storage() else {
        return Ok(());
    };

    save_storage_key(&storage, ENTRY_FLAGS_STORAGE_KEY, serde_json::to_string(&slice)?)
}

fn load_app_state_slice(storage: &Storage) -> Option<PersistedAppStateSlice> {
    let raw = storage.get_item(APP_STATE_STORAGE_KEY).ok().flatten()?;
    match serde_json::from_str(&raw) {
        Ok(slice) => Some(slice),
        Err(_) => {
            backup_corrupt_blob(storage, APP_STATE_STORAGE_KEY, &raw);
            let _ = storage.remove_item(APP_STATE_STORAGE_KEY);
            None
        }
    }
}

fn load_entry_flags_slice(storage: &Storage) -> Option<PersistedEntryFlagsSlice> {
    let raw = storage.get_item(ENTRY_FLAGS_STORAGE_KEY).ok().flatten()?;
    match serde_json::from_str(&raw) {
        Ok(slice) => Some(slice),
        Err(_) => {
            backup_corrupt_blob(storage, ENTRY_FLAGS_STORAGE_KEY, &raw);
            let _ = storage.remove_item(ENTRY_FLAGS_STORAGE_KEY);
            None
        }
    }
}

fn backup_corrupt_blob(storage: &Storage, key: &str, raw: &str) {
    let backup_key = format!("{key}-corrupt-{}", Date::now() as i64);
    let _ = storage.set_item(&backup_key, raw);
}

pub fn last_opened_feed_id(state: &BrowserState) -> Option<i64> {
    state.app_state.last_opened_feed_id
}

pub fn entry_flags<'a>(state: &'a BrowserState, entry_id: i64) -> Option<&'a PersistedEntryFlag> {
    state.entry_flags.entries.iter().find(|flag| flag.id == entry_id)
}

pub fn to_domain_entry(state: &BrowserState, entry: &PersistedEntry) -> anyhow::Result<Entry> {
    let flags = entry_flags(state, entry.id);
    Ok(Entry {
        id: entry.id,
        feed_id: entry.feed_id,
        external_id: entry.external_id.clone(),
        dedup_key: entry.dedup_key.clone(),
        url: entry.url.as_ref().map(|raw| Url::parse(raw)).transpose()?,
        title: entry.title.clone(),
        author: entry.author.clone(),
        summary: entry.summary.clone(),
        content_html: entry.content_html.clone(),
        content_text: entry.content_text.clone(),
        published_at: entry.published_at,
        updated_at_source: entry.updated_at_source,
        first_seen_at: entry.first_seen_at,
        content_hash: entry.content_hash.clone(),
        is_read: flags.map(|flag| flag.is_read).unwrap_or(false),
        is_starred: flags.map(|flag| flag.is_starred).unwrap_or(false),
        read_at: flags.and_then(|flag| flag.read_at),
        starred_at: flags.and_then(|flag| flag.starred_at),
        created_at: entry.created_at,
        updated_at: entry.updated_at,
    })
}

pub fn upsert_entries(
    state: &mut PersistedState,
    feed_id: i64,
    entries: Vec<ParsedEntry>,
) -> anyhow::Result<()> {
    for entry in entries {
        let content_hash = hash_content(
            entry.content_html.as_deref(),
            entry.content_text.as_deref(),
            Some(&entry.title),
        );
        let now = now_utc();
        if let Some(existing) = state
            .entries
            .iter_mut()
            .find(|current| current.feed_id == feed_id && current.dedup_key == entry.dedup_key)
        {
            existing.external_id = entry.external_id;
            if let Some(url) = entry.url.as_ref() {
                existing.url = Some(url.to_string());
            }
            existing.title = entry.title;
            existing.author = entry.author;
            existing.summary = entry.summary;
            if entry.content_html.is_some() {
                existing.content_html = entry.content_html;
            }
            if entry.content_text.is_some() {
                existing.content_text = entry.content_text;
            }
            existing.published_at = entry.published_at.or(existing.published_at);
            existing.updated_at_source = entry.updated_at_source.or(existing.updated_at_source);
            existing.content_hash = content_hash;
            existing.updated_at = now;
        } else {
            state.next_entry_id += 1;
            state.entries.push(PersistedEntry {
                id: state.next_entry_id,
                feed_id,
                external_id: entry.external_id,
                dedup_key: entry.dedup_key,
                url: entry.url.map(|url| url.to_string()),
                title: entry.title,
                author: entry.author,
                summary: entry.summary,
                content_html: entry.content_html,
                content_text: entry.content_text,
                published_at: entry.published_at,
                updated_at_source: entry.updated_at_source,
                first_seen_at: now,
                content_hash,
                created_at: now,
                updated_at: now,
            });
        }
    }
    Ok(())
}
