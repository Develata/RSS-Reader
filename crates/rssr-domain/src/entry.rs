use time::OffsetDateTime;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub feed_id: i64,
    pub external_id: String,
    pub dedup_key: String,
    pub url: Option<Url>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySummary {
    pub id: i64,
    pub feed_id: i64,
    pub title: String,
    pub feed_title: String,
    pub published_at: Option<OffsetDateTime>,
    pub is_read: bool,
    pub is_starred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntryQuery {
    pub feed_id: Option<i64>,
    pub unread_only: bool,
    pub starred_only: bool,
    pub search_title: Option<String>,
    pub limit: Option<u32>,
}
