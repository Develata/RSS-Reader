use crate::{
    entry::{Entry, EntryQuery, EntrySummary},
    feed::{Feed, FeedSummary, NewFeedSubscription},
    settings::UserSettings,
};

pub trait HealthRepository {
    fn is_ready(&self) -> bool;
}

#[async_trait::async_trait]
pub trait FeedRepository: Send + Sync {
    async fn upsert_subscription(&self, new_feed: &NewFeedSubscription) -> crate::Result<Feed>;
    async fn list_feeds(&self) -> crate::Result<Vec<Feed>>;
    async fn list_summaries(&self) -> crate::Result<Vec<FeedSummary>>;
}

#[async_trait::async_trait]
pub trait EntryRepository: Send + Sync {
    async fn list_entries(&self, query: &EntryQuery) -> crate::Result<Vec<EntrySummary>>;
    async fn get_entry(&self, entry_id: i64) -> crate::Result<Option<Entry>>;
}

#[async_trait::async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn load(&self) -> crate::Result<UserSettings>;
    async fn save(&self, settings: &UserSettings) -> crate::Result<()>;
}
