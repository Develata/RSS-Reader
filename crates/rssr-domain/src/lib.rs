// Rust 1.99 Clippy false-positive from async_trait macro expansion:
// https://github.com/rust-lang/rust-clippy/issues/17529
#![allow(clippy::double_must_use)]
pub mod app_state;
pub mod config_fingerprint;
pub mod entry;
pub mod feed;
pub mod listed;
pub mod repository;
pub mod search;
pub mod settings;
pub mod url_guard;
pub mod validation;
pub use url_guard::{
    MAX_UNTRUSTED_URL_BYTES, MAX_URL_PATH_DOTS, join_untrusted_url, parse_untrusted_url,
    untrusted_url_is_parseable, url_path_dots,
};

pub use app_state::{AppStateSnapshot, EntriesWorkspaceState, EntryGroupingPreference};
pub use config_fingerprint::{ConfigFingerprint, FingerprintFeed};
pub use entry::{
    ArchiveFilter, Entry, EntryBatchAction, EntryBatchRequest, EntryContent, EntryNavigation,
    EntryQuery, EntryRecord, EntrySummary, MAX_MARK_READ_SELECTION, MarkReadOutcome,
    MarkReadPreview, ReadFilter, StarredFilter, archive_cutoff_at, is_entry_archived,
    mark_read_selection_too_large, stored_published_at,
};
pub use feed::{
    Feed, FeedEditDetails, FeedEditRequest, FeedSummary, FetchErrorKind, NewFeedSubscription,
    normalize_feed_url, public_fetch_error,
};
pub use listed::{LISTED_TITLE_MARK, MAX_LISTED_TITLE_CHARS, listed_title};
pub use repository::{
    AppStateMutation, AppStateRepository, EntryContentRepository, EntryIndexRepository,
    EntryRepository, FeedRepository, HealthRepository, SettingsRepository,
};
pub use search::{fold_for_search, title_contains};
pub use settings::{
    ConfigFeed, ConfigPackage, DEFAULT_ENTRIES_PAGE_SIZE, ListDensity, MAX_ARCHIVE_AFTER_MONTHS,
    MAX_CUSTOM_CSS_BYTES, MAX_ENTRIES_PAGE_SIZE, MAX_REFRESH_INTERVAL_MINUTES, OpmlFeed,
    StartupView, ThemeMode, UserSettings,
};
pub use validation::{
    CONFIG_PACKAGE_VERSION, parse_and_normalize_feed_url, parse_feed_url, validate_config_package,
    validate_entry_query, validate_mark_read_preview, validate_user_settings,
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
    /// The data changed since the caller read it; it must read again before retrying.
    #[error("操作冲突：{0}")]
    Conflict(String),
    /// A remote site could not be reached or answered with a failure.
    #[error("网络访问失败：{0}")]
    Network(String),
    /// The store is busy, timed out or was interrupted; retrying later may work.
    #[error("暂时不可用：{0}")]
    Unavailable(String),
}
