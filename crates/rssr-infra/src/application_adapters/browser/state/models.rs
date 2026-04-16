use rssr_domain::AppStateSnapshot;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const STORAGE_KEY: &str = "rssr-web-state-v1";
pub const APP_STATE_STORAGE_KEY: &str = "rssr-web-app-state-v2";
pub const ENTRY_FLAGS_STORAGE_KEY: &str = "rssr-web-entry-flags-v1";
pub const ENTRY_CONTENT_STORAGE_KEY: &str = "rssr-web-entry-content-v1";

#[derive(Debug, Default, Clone)]
pub struct BrowserState {
    pub core: PersistedState,
    pub app_state: PersistedAppStateSlice,
    pub entry_flags: PersistedEntryFlagsSlice,
    pub entry_content: PersistedEntryContentSlice,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PersistedState {
    pub next_feed_id: i64,
    pub next_entry_id: i64,
    pub feeds: Vec<PersistedFeed>,
    pub entries: Vec<PersistedEntryIndex>,
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
pub struct PersistedEntryIndex {
    pub id: i64,
    pub feed_id: i64,
    pub external_id: String,
    pub dedup_key: String,
    pub url: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub published_at: Option<OffsetDateTime>,
    pub updated_at_source: Option<OffsetDateTime>,
    pub first_seen_at: OffsetDateTime,
    pub has_content: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct LoadedState {
    pub state: BrowserState,
    pub warning: Option<String>,
}

pub type PersistedAppStateSlice = AppStateSnapshot;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PersistedEntryContentSlice {
    pub entries: Vec<PersistedEntryContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedEntryContent {
    pub entry_id: i64,
    pub feed_id: i64,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub content_hash: Option<String>,
    pub updated_at: OffsetDateTime,
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
