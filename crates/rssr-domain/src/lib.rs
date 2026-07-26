pub mod app_state;
pub mod entry;
pub mod feed;
pub mod repository;
pub mod settings;
pub mod validation;

pub use app_state::{AppStateSnapshot, EntriesWorkspaceState, EntryGroupingPreference};
pub use entry::{
    ArchiveFilter, Entry, EntryContent, EntryNavigation, EntryQuery, EntryRecord, EntrySummary,
    ReadFilter, StarredFilter, archive_cutoff_at, is_entry_archived,
};
pub use feed::{Feed, FeedSummary, NewFeedSubscription, normalize_feed_url};
pub use repository::{
    AppStateRepository, EntryContentRepository, EntryIndexRepository, EntryRepository,
    FeedRepository, HealthRepository, SettingsRepository,
};
pub use settings::{
    ConfigFeed, ConfigPackage, DEFAULT_ENTRIES_PAGE_SIZE, ListDensity, MAX_ARCHIVE_AFTER_MONTHS,
    MAX_ENTRIES_PAGE_SIZE, MAX_REFRESH_INTERVAL_MINUTES, StartupView, ThemeMode, UserSettings,
};
pub use validation::{
    CONFIG_PACKAGE_VERSION, parse_and_normalize_feed_url, validate_config_package,
    validate_user_settings,
};

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
