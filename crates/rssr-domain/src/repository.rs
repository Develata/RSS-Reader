use url::Url;

use crate::{
    app_state::AppStateSnapshot,
    entry::{Entry, EntryContent, EntryNavigation, EntryQuery, EntryRecord, EntrySummary},
    feed::{Feed, FeedSummary, NewFeedSubscription, normalize_feed_url},
    settings::UserSettings,
};

pub trait HealthRepository {
    fn is_ready(&self) -> bool;
}

#[async_trait::async_trait]
pub trait FeedRepository: Send + Sync {
    async fn upsert_subscription(&self, new_feed: &NewFeedSubscription) -> crate::Result<Feed>;
    /// Adds or updates all of `new_feeds` in one transaction of the store.
    async fn upsert_subscriptions(
        &self,
        new_feeds: &[NewFeedSubscription],
    ) -> crate::Result<Vec<Feed>>;
    async fn has_active_subscription_url(&self, url: &Url) -> crate::Result<bool> {
        let normalized_url = normalize_feed_url(url);
        Ok(self
            .list_feeds()
            .await?
            .iter()
            .any(|feed| !feed.is_deleted && normalize_feed_url(&feed.url) == normalized_url))
    }
    async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> crate::Result<()>;
    async fn list_feeds(&self) -> crate::Result<Vec<Feed>>;
    async fn get_feed(&self, feed_id: i64) -> crate::Result<Option<Feed>>;
    async fn list_summaries(&self) -> crate::Result<Vec<FeedSummary>>;
}

#[async_trait::async_trait]
pub trait EntryIndexRepository: Send + Sync {
    async fn preview_mark_read(&self, query: &EntryQuery) -> crate::Result<crate::MarkReadPreview>;
    async fn mark_read_if_unchanged(
        &self,
        preview: &crate::MarkReadPreview,
    ) -> crate::Result<crate::MarkReadOutcome>;

    async fn list_entries(&self, query: &EntryQuery) -> crate::Result<Vec<EntrySummary>>;
    async fn count_entries(&self, query: &EntryQuery) -> crate::Result<u64>;
    async fn get_entry_record(&self, entry_id: i64) -> crate::Result<Option<EntryRecord>>;
    async fn reader_navigation(&self, current_entry_id: i64) -> crate::Result<EntryNavigation>;
    /// Sets the flag to `is_read`; `true` when it changed. `read_at` moves only on a change.
    /// A missing entry is [`crate::DomainError::NotFound`].
    async fn set_read(&self, entry_id: i64, is_read: bool) -> crate::Result<bool>;
    /// Sets the flag to `is_starred`; `true` when it changed. `starred_at` moves only on a
    /// change. A missing entry is [`crate::DomainError::NotFound`].
    async fn set_starred(&self, entry_id: i64, is_starred: bool) -> crate::Result<bool>;
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

/// 对整份 [`AppStateSnapshot`] 的一次修改，由 [`AppStateRepository::update`] 原子地应用。
pub type AppStateMutation = Box<dyn FnOnce(&mut AppStateSnapshot) + Send>;

#[async_trait::async_trait]
pub trait AppStateRepository: Send + Sync {
    async fn load(&self) -> crate::Result<AppStateSnapshot>;

    /// 在一次原子操作里读出整份状态、应用 `mutate` 并写回。
    ///
    /// 并发调用必须串行化：任何一方都不能用自己读到的旧快照覆盖另一方的改动。状态里的
    /// 各字段由不同页面分别更新（文章页偏好、最后打开的订阅），整份读改写若不原子就会
    /// 互相吞掉修改。故意不提供整份写入的 `save`，避免调用方重新拼出“先读后写”。
    async fn update(&self, mutate: AppStateMutation) -> crate::Result<()>;
}
