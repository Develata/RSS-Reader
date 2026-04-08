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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PersistedState {
    pub next_feed_id: i64,
    pub next_entry_id: i64,
    pub feeds: Vec<PersistedFeed>,
    pub entries: Vec<PersistedEntry>,
    pub settings: rssr_domain::UserSettings,
    pub last_opened_feed_id: Option<i64>,
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
    pub is_read: bool,
    pub is_starred: bool,
    pub read_at: Option<OffsetDateTime>,
    pub starred_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct LoadedState {
    pub state: PersistedState,
    pub warning: Option<String>,
}

pub fn load_state() -> LoadedState {
    let Some(storage) = window().and_then(|window| window.local_storage().ok()).flatten() else {
        return LoadedState { state: PersistedState::default(), warning: None };
    };

    let raw = match storage.get_item(STORAGE_KEY) {
        Ok(Some(raw)) => raw,
        Ok(None) => {
            return LoadedState { state: PersistedState::default(), warning: None };
        }
        Err(_) => {
            return LoadedState {
                state: PersistedState::default(),
                warning: Some("读取浏览器本地存储失败，已使用空状态启动。".to_string()),
            };
        }
    };

    match serde_json::from_str(&raw).context("解析浏览器本地状态失败") {
        Ok(state) => LoadedState { state, warning: None },
        Err(error) => {
            backup_corrupt_state(&storage, &raw);
            let _ = storage.remove_item(STORAGE_KEY);
            LoadedState {
                state: PersistedState::default(),
                warning: Some(format!(
                    "浏览器本地状态已损坏，已保留损坏副本并使用空状态启动：{error}"
                )),
            }
        }
    }
}

pub fn save_state_snapshot(state: PersistedState) -> anyhow::Result<()> {
    save_serialized_state(serde_json::to_string(&state)?)
}

fn save_serialized_state(raw: String) -> anyhow::Result<()> {
    let Some(storage) = window().and_then(|window| window.local_storage().ok()).flatten() else {
        return Ok(());
    };
    storage.set_item(STORAGE_KEY, &raw).map_err(|_| anyhow::anyhow!("写入浏览器本地存储失败"))?;
    Ok(())
}

fn backup_corrupt_state(storage: &Storage, raw: &str) {
    let backup_key = format!("{STORAGE_KEY}-corrupt-{}", Date::now() as i64);
    let _ = storage.set_item(&backup_key, raw);
}

pub fn to_domain_entry(entry: &PersistedEntry) -> anyhow::Result<Entry> {
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
        is_read: entry.is_read,
        is_starred: entry.is_starred,
        read_at: entry.read_at,
        starred_at: entry.starred_at,
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
                is_read: false,
                is_starred: false,
                read_at: None,
                starred_at: None,
                created_at: now,
                updated_at: now,
            });
        }
    }
    Ok(())
}
