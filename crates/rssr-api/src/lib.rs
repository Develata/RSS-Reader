//! The interface between the user interface and the reader's use cases.
//!
//! [`Backend`] lists every operation the UI performs. The desktop app, Android and the Pages demo
//! implement it in-process (a `LocalBackend` over the use cases); the server-side Web will
//! implement it over HTTP. Data types are the application's own (`rssr_application`,
//! `rssr_domain`); this crate adds no parallel copies.
//!
//! Phase B1 (`docs/design/v0.2-architecture.md`): the interface mirrors the UI's existing
//! operations one to one with their existing types. Contract changes (serializable types,
//! fixed-width counts, error categories, device state) come separately.

use anyhow::Result;
use rssr_application::{
    AppliedRemoteConfigOutcome, ConfigImportOutcome, EntriesBootstrapInput,
    EntriesBootstrapOutcome, EntriesListOutcome, FeedDiscoveryCandidate, FeedsSnapshotOutcome,
    OpmlImportOutcome, ReaderEntrySnapshot, RemoteConfigPushOutcome, StartupTarget,
    ToggleEntryReadInput, ToggleEntryReadOutcome, ToggleEntryStarredInput,
    ToggleEntryStarredOutcome, ToggleReadInput, ToggleReadOutcome, ToggleStarredInput,
    ToggleStarredOutcome,
};
use rssr_domain::{
    EntriesWorkspaceState, EntryQuery, MarkReadOutcome, MarkReadPreview, UserSettings,
};
use url::Url;

/// What adding a subscription led to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddSubscriptionOutcome {
    /// The address is a page listing several feeds; the user picks one.
    NeedsSelection {
        page_url: Url,
        candidates: Vec<FeedDiscoveryCandidate>,
    },
    SavedAndRefreshed,
    /// Saved, but its first refresh failed.
    SavedRefreshFailed {
        message: String,
    },
}

/// What refreshing every subscription led to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshAllExecutionOutcome {
    pub inserted_count: u64,
    pub total_count: usize,
    pub failed_count: usize,
    pub failure_message: Option<String>,
}

/// What refreshing one subscription led to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshFeedExecutionOutcome {
    pub inserted_count: u64,
    pub failure_message: Option<String>,
}

/// Every operation of the user interface.
// `async_trait` marks its boxed futures `#[must_use]` twice.
#[allow(clippy::double_must_use)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait Backend {
    // Shell and settings.
    async fn load_settings(&self) -> Result<UserSettings>;
    async fn save_settings(&self, settings: &UserSettings) -> Result<()>;
    /// Starts periodic refreshing where this backend runs it (once; later calls do nothing).
    fn ensure_auto_refresh_started(&self);
    async fn resolve_startup_target(&self) -> Result<StartupTarget>;
    async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPushOutcome>;
    /// Downloads the configuration, applies it and returns the resulting settings.
    async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<AppliedRemoteConfigOutcome>;

    // Entry list.
    async fn bootstrap_entries(
        &self,
        input: EntriesBootstrapInput,
    ) -> Result<EntriesBootstrapOutcome>;
    /// Saves the entry-list preferences; `true` when they changed.
    async fn save_entries_workspace_if_changed(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> Result<bool>;
    async fn list_entries(&self, query: &EntryQuery) -> Result<EntriesListOutcome>;
    async fn toggle_entry_read(
        &self,
        input: ToggleEntryReadInput,
    ) -> Result<ToggleEntryReadOutcome>;
    async fn toggle_entry_starred(
        &self,
        input: ToggleEntryStarredInput,
    ) -> Result<ToggleEntryStarredOutcome>;
    async fn preview_mark_read(&self, query: &EntryQuery) -> Result<MarkReadPreview>;
    async fn mark_read_if_unchanged(&self, preview: &MarkReadPreview) -> Result<MarkReadOutcome>;

    // Reader.
    async fn load_reader_entry(&self, entry_id: i64) -> Result<ReaderEntrySnapshot>;
    async fn toggle_reader_read(&self, input: ToggleReadInput) -> Result<ToggleReadOutcome>;
    async fn toggle_reader_starred(
        &self,
        input: ToggleStarredInput,
    ) -> Result<ToggleStarredOutcome>;
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
    async fn import_config_json(&self, raw: &str) -> Result<ConfigImportOutcome>;
    async fn export_opml(&self) -> Result<String>;
    async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome>;
}
