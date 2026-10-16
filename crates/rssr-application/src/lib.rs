// Rust 1.99 Clippy false-positive from async_trait macro expansion:
// https://github.com/rust-lang/rust-clippy/issues/17529
#![allow(clippy::double_must_use)]
pub mod app_state_service;
pub mod composition;
pub mod entries_list_service;
pub mod entries_workspace_service;
pub mod entry_flags_service;
pub mod error;
pub mod feed_catalog_service;
pub mod feed_service;
pub mod feeds_snapshot_service;
pub mod import_export_service;
pub mod library_queries;
pub mod persistence_mutation;
pub mod reader_service;
pub mod refresh_service;
mod result_bounds;
pub mod settings_service;
pub mod startup_service;
mod subscription_discovery;
pub mod subscription_workflow;
pub use subscription_discovery::*;

pub use app_state_service::AppStateService;
pub use composition::{AppCompositionInput, AppStateServicesPort, AppUseCases};
pub use entries_list_service::{EntriesListOutcome, EntriesListService};
pub use entries_workspace_service::{
    EntriesBootstrapInput, EntriesBootstrapOutcome, EntriesWorkspaceService,
    SaveEntriesWorkspaceOutcome,
};
pub use entry_flags_service::{EntryFlagOutcome, EntryFlagsService};
pub use error::{ErrorKind, PublicError, classify};
pub use feed_catalog_service::FeedCatalogService;
pub use feed_service::{AddSubscriptionInput, FeedService, RemoveSubscriptionInput};
pub use feeds_snapshot_service::{FeedsSnapshotOutcome, FeedsSnapshotService};
pub use import_export_service::{
    ClockPort, ConfigImportOutcome, ImportExportService, OpmlCodecPort, OpmlImportOutcome,
    RemoteConfigPullOutcome, RemoteConfigPushOutcome, RemoteConfigStore, SystemClock,
};
pub use library_queries::{
    ConfigSnapshot, EntriesPage, EntriesStart, FeedsOverview, LibraryQueryPort, ReaderSnapshot,
    StartupState,
};
pub use persistence_mutation::{
    ActivatedSubscription, ConfigReplacementFeed, ConfigReplacementOutcome, ConfigReplacementPlan,
    ConfigReplacementPort, SettingsUpdatePort, SubscriptionActivationPort, SubscriptionRemovalPort,
};
pub use reader_service::{ReaderEntry, ReaderEntrySnapshot, ReaderService};
pub use refresh_service::{
    DEFAULT_REFRESH_CONCURRENCY, FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate,
    ParsedEntryData, ParsedFeedUpdate, PreparedUpdateTicket, RefreshAllExecutionOutcome,
    RefreshAllInput, RefreshAllOutcome, RefreshAllSummary, RefreshCommit, RefreshCommitOutcome,
    RefreshFailure, RefreshFailureItem, RefreshFeedExecutionOutcome, RefreshFeedOutcome,
    RefreshFeedResult, RefreshHttpMetadata, RefreshLocalizedEntry, RefreshService,
    RefreshStorePort, RefreshTarget, RefreshTicket, SERIAL_REFRESH_CONCURRENCY,
};
pub use result_bounds::{
    MAX_DISCOVERY_CANDIDATES, MAX_LISTED_FAILURES, MAX_MESSAGE_BYTES, MAX_NAME_BYTES,
    MAX_RESULT_BYTES, MAX_URL_BYTES, TRUNCATION_MARK, truncate_marked,
};
pub use settings_service::{SettingsPatch, SettingsService};
pub use startup_service::{StartupService, StartupTarget};
pub use subscription_workflow::{
    AddSubscriptionAndRefreshOutcome, AddSubscriptionLifecycleInput,
    AddSubscriptionLifecycleOutcome, AddSubscriptionOutcome, AppStatePort, SubscriptionWorkflow,
};
