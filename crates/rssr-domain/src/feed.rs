use time::OffsetDateTime;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feed {
    pub id: i64,
    pub url: Url,
    pub title: Option<String>,
    pub site_url: Option<Url>,
    pub description: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_fetched_at: Option<OffsetDateTime>,
    pub last_success_at: Option<OffsetDateTime>,
    pub fetch_error: Option<String>,
    pub is_deleted: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedSummary {
    pub id: i64,
    pub title: String,
    pub unread_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFeedSubscription {
    pub url: Url,
    pub title: Option<String>,
}
