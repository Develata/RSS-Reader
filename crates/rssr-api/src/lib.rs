//! The interface between the user interface and the reader's use cases.
//!
//! [`Backend`] lists every operation the UI performs. The desktop app, Android and the Pages demo
//! implement it in-process (a `LocalBackend` over the use cases); the server-side Web will
//! implement it over HTTP. Data types are the application's own (`rssr_application`,
//! `rssr_domain`); this crate adds no parallel copies.
//!
//! Phase B1 (`docs/design/v0.2-architecture.md`) made the interface mirror the UI's operations;
//! phase B2 (`docs/design/v0.2-phase-b2-contracts.md`) changes the contract item by item. Every
//! operation fails with a [`BackendError`]: a stable [`ErrorCode`], a message for the user and a
//! request id.
//!
//! [`protocol`] is the HTTP form of the same operations (phase C): paths, request bodies and
//! background jobs, shared by the server and the browser's backend.

pub use rssr_application::{
    AddSubscriptionOutcome, RefreshAllExecutionOutcome, RefreshFailureItem,
    RefreshFeedExecutionOutcome,
};
use rssr_application::{
    ConfigImportOutcome, EntriesBootstrapInput, EntriesBootstrapOutcome, EntriesListOutcome,
    EntryFlagOutcome, FeedsSnapshotOutcome, OpmlImportOutcome, ReaderEntrySnapshot,
    RemoteConfigPullOutcome, RemoteConfigPushOutcome, SettingsPatch, StartupTarget,
};
use rssr_domain::{
    ConfigFingerprint, EntriesWorkspaceState, EntryQuery, MarkReadOutcome, MarkReadPreview,
    UserSettings,
};
use url::Url;

mod error;
pub mod protocol;

pub use error::{BackendError, ErrorCode};

/// Every operation's result: a [`BackendError`] carries the stable code and the user's message.
pub type Result<T, E = BackendError> = std::result::Result<T, E>;

/// Every operation of the user interface.
// `async_trait` marks its boxed futures `#[must_use]` twice.
#[allow(clippy::double_must_use)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait Backend {
    // Shell and settings.
    async fn load_settings(&self) -> Result<UserSettings>;
    /// Changes the given settings fields (merged, checked and saved in one write) and returns
    /// the settings now stored; an empty patch only reads them.
    async fn update_settings(&self, patch: SettingsPatch) -> Result<UserSettings>;
    /// Starts periodic refreshing where this backend runs it (once; later calls do nothing).
    fn ensure_auto_refresh_started(&self);
    async fn resolve_startup_target(&self) -> Result<StartupTarget>;
    async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPushOutcome>;
    /// Downloads the configuration and replaces the local one with it; the outcome carries the
    /// settings now stored. A local change made during the download makes it a conflict.
    async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPullOutcome>;

    // Entry list.
    async fn bootstrap_entries(
        &self,
        input: EntriesBootstrapInput,
    ) -> Result<EntriesBootstrapOutcome>;
    /// Remembers the subscription the user opened, for the startup view.
    async fn remember_opened_feed(&self, feed_id: i64) -> Result<()>;
    /// Saves the entry-list preferences; `true` when they changed.
    async fn save_entries_workspace_if_changed(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> Result<bool>;
    async fn list_entries(&self, query: &EntryQuery) -> Result<EntriesListOutcome>;
    /// Sets the entry's read flag to `read` (the entry list and the reader alike).
    async fn set_entry_read(&self, entry_id: i64, read: bool) -> Result<EntryFlagOutcome>;
    /// Sets the entry's starred flag to `starred`.
    async fn set_entry_starred(&self, entry_id: i64, starred: bool) -> Result<EntryFlagOutcome>;
    async fn preview_mark_read(&self, query: &EntryQuery) -> Result<MarkReadPreview>;
    async fn mark_read_if_unchanged(&self, preview: &MarkReadPreview) -> Result<MarkReadOutcome>;

    // Reader.
    async fn load_reader_entry(&self, entry_id: i64) -> Result<ReaderEntrySnapshot>;
    /// Stores the entry's images locally where this backend can; `true` when it did.
    async fn localize_entry_assets(&self, entry_id: i64) -> Result<bool>;

    // Subscriptions and exchange.
    async fn load_feeds_snapshot(&self) -> Result<FeedsSnapshotOutcome>;
    async fn add_subscription(
        &self,
        raw_url: &str,
        fallback_site_url: Option<Url>,
    ) -> Result<AddSubscriptionOutcome>;
    async fn refresh_all(&self) -> Result<RefreshAllExecutionOutcome>;
    async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedExecutionOutcome>;
    /// Removes the subscription and its entries.
    async fn remove_feed(&self, feed_id: i64) -> Result<()>;
    async fn export_config_json(&self) -> Result<String>;
    /// Replaces the configuration with the package `raw`, unless it changed since the caller
    /// read `expected` (from [`Backend::load_feeds_snapshot`]): then a conflict
    /// ([`rssr_domain::DomainError::Conflict`] in the error chain) and nothing changed.
    async fn import_config_json(
        &self,
        raw: &str,
        expected: &ConfigFingerprint,
    ) -> Result<ConfigImportOutcome>;
    async fn export_opml(&self) -> Result<String>;
    async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome>;
}
