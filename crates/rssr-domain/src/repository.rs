use crate::{
    app_state::AppStateSnapshot,
    entry::{Entry, EntryContent, EntryNavigation, EntryQuery, EntryRecord, EntrySummary},
    feed::{Feed, FeedSummary, NewFeedSubscription},
    settings::UserSettings,
};

pub trait HealthRepository {
    fn is_ready(&self) -> bool;
}

#[async_trait::async_trait]
pub trait FeedRepository: Send + Sync {
    async fn upsert_subscription(&self, new_feed: &NewFeedSubscription) -> crate::Result<Feed>;
    async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> crate::Result<()>;
    async fn list_feeds(&self) -> crate::Result<Vec<Feed>>;
    async fn get_feed(&self, feed_id: i64) -> crate::Result<Option<Feed>>;
    async fn list_summaries(&self) -> crate::Result<Vec<FeedSummary>>;
}

#[async_trait::async_trait]
pub trait EntryIndexRepository: Send + Sync {
    async fn list_entries(&self, query: &EntryQuery) -> crate::Result<Vec<EntrySummary>>;
    async fn count_entries(&self, query: &EntryQuery) -> crate::Result<u64>;
    async fn get_entry_record(&self, entry_id: i64) -> crate::Result<Option<EntryRecord>>;
    async fn reader_navigation(&self, current_entry_id: i64) -> crate::Result<EntryNavigation>;
    async fn set_read(&self, entry_id: i64, is_read: bool) -> crate::Result<()>;
    async fn set_starred(&self, entry_id: i64, is_starred: bool) -> crate::Result<()>;
    async fn delete_for_feed(&self, feed_id: i64) -> crate::Result<()>;
}

#[async_trait::async_trait]
pub trait EntryContentRepository: Send + Sync {
    async fn get_content(&self, entry_id: i64) -> crate::Result<Option<EntryContent>>;
    async fn delete_for_feed(&self, feed_id: i64) -> crate::Result<()>;
    async fn delete_for_entry_ids(&self, entry_ids: &[i64]) -> crate::Result<()>;
}

#[async_trait::async_trait]
pub trait EntryRepository: EntryIndexRepository + EntryContentRepository + Send + Sync {
    async fn get_entry(&self, entry_id: i64) -> crate::Result<Option<Entry>> {
        let Some(record) = self.get_entry_record(entry_id).await? else {
            return Ok(None);
        };
        let content = self.get_content(entry_id).await?;
        Ok(Some(record.into_entry(content)))
    }
}

impl<T> EntryRepository for T where
    T: EntryIndexRepository + EntryContentRepository + Send + Sync + ?Sized
{
}

#[async_trait::async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn load(&self) -> crate::Result<UserSettings>;
    async fn save(&self, settings: &UserSettings) -> crate::Result<()>;
}

#[async_trait::async_trait]
pub trait AppStateRepository: Send + Sync {
    async fn load(&self) -> crate::Result<AppStateSnapshot>;
    async fn save(&self, state: &AppStateSnapshot) -> crate::Result<()>;
}
