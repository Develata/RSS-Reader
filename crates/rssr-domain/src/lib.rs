pub mod entry;
pub mod feed;
pub mod repository;
pub mod settings;

pub use entry::{
    Entry, EntryNavigation, EntryQuery, EntrySummary, ReadFilter, StarredFilter, is_entry_archived,
};
pub use feed::{Feed, FeedSummary, NewFeedSubscription, normalize_feed_url};
pub use repository::{EntryRepository, FeedRepository, HealthRepository, SettingsRepository};
pub use settings::{ConfigFeed, ConfigPackage, ListDensity, StartupView, ThemeMode, UserSettings};

pub type Result<T> = std::result::Result<T, DomainError>;

#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("未找到请求的资源")]
    NotFound,
    #[error("输入无效：{0}")]
    InvalidInput(String),
    #[error("持久化失败：{0}")]
    Persistence(String),
}
