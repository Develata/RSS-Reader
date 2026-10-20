//! A backend for the UI's tests (C4b): it answers like the library it wraps, except for the
//! operations told to fail, which fail with the error they were given. The in-process library
//! never produces the errors the page must recover from (`outcome_unknown`, `unauthenticated`,
//! `build_mismatch`); this one does, on demand. It also counts the operations that ran.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rssr_api::{
    AddSubscriptionOutcome, Backend, BackendError, RefreshAllExecutionOutcome,
    RefreshFeedExecutionOutcome, Result,
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

pub(crate) struct Scripted {
    inner: Arc<dyn Backend + Send + Sync>,
    failures: Mutex<HashMap<&'static str, BackendError>>,
    ran: Mutex<Vec<&'static str>>,
}

impl Scripted {
    pub(crate) fn over(inner: Arc<dyn Backend + Send + Sync>) -> Arc<Self> {
        Arc::new(Self { inner, failures: Mutex::default(), ran: Mutex::default() })
    }

    /// Over a new, empty library in memory (opened on a thread of its own, so a test's runtime
    /// is not blocked).
    pub(crate) fn over_new_library() -> Arc<Self> {
        let store = std::thread::spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime")
                .block_on(rssr_infra::store::LibraryStore::open_in_memory())
                .expect("library")
        })
        .join()
        .expect("open");
        Self::over(Arc::new(rssr_infra::local_backend::native_local_backend(store)))
    }

    /// From now on `operation` (the [`Backend`] method's name) fails with `error`.
    pub(crate) fn fail(&self, operation: &'static str, error: BackendError) {
        self.failures.lock().unwrap().insert(operation, error);
    }

    /// From now on `operation` answers like the library again.
    pub(crate) fn heal(&self, operation: &'static str) {
        self.failures.lock().unwrap().remove(operation);
    }

    /// How many times `operation` was called (failing or not).
    pub(crate) fn calls(&self, operation: &str) -> usize {
        self.ran.lock().unwrap().iter().filter(|ran| **ran == operation).count()
    }

    fn enter(&self, operation: &'static str) -> Result<()> {
        self.ran.lock().unwrap().push(operation);
        match self.failures.lock().unwrap().get(operation) {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

#[async_trait::async_trait]
impl Backend for Scripted {
    async fn load_settings(&self) -> Result<UserSettings> {
        self.enter("load_settings")?;
        self.inner.load_settings().await
    }

    async fn update_settings(&self, patch: SettingsPatch) -> Result<UserSettings> {
        self.enter("update_settings")?;
        self.inner.update_settings(patch).await
    }

    fn ensure_auto_refresh_started(&self) {
        self.ran.lock().unwrap().push("ensure_auto_refresh_started");
        self.inner.ensure_auto_refresh_started();
    }

    async fn resolve_startup_target(&self) -> Result<StartupTarget> {
        self.enter("resolve_startup_target")?;
        self.inner.resolve_startup_target().await
    }

    async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPushOutcome> {
        self.enter("push_remote_config")?;
        self.inner.push_remote_config(endpoint, remote_path).await
    }

    async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPullOutcome> {
        self.enter("pull_remote_config")?;
        self.inner.pull_remote_config(endpoint, remote_path).await
    }

    async fn bootstrap_entries(
        &self,
        input: EntriesBootstrapInput,
    ) -> Result<EntriesBootstrapOutcome> {
        self.enter("bootstrap_entries")?;
        self.inner.bootstrap_entries(input).await
    }

    async fn remember_opened_feed(&self, feed_id: i64) -> Result<()> {
        self.enter("remember_opened_feed")?;
        self.inner.remember_opened_feed(feed_id).await
    }

    async fn save_entries_workspace_if_changed(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> Result<bool> {
        self.enter("save_entries_workspace_if_changed")?;
        self.inner.save_entries_workspace_if_changed(workspace).await
    }

    async fn list_entries(&self, query: &EntryQuery) -> Result<EntriesListOutcome> {
        self.enter("list_entries")?;
        self.inner.list_entries(query).await
    }

    async fn set_entry_read(&self, entry_id: i64, read: bool) -> Result<EntryFlagOutcome> {
        self.enter("set_entry_read")?;
        self.inner.set_entry_read(entry_id, read).await
    }

    async fn set_entry_starred(&self, entry_id: i64, starred: bool) -> Result<EntryFlagOutcome> {
        self.enter("set_entry_starred")?;
        self.inner.set_entry_starred(entry_id, starred).await
    }

    async fn preview_mark_read(&self, query: &EntryQuery) -> Result<MarkReadPreview> {
        self.enter("preview_mark_read")?;
        self.inner.preview_mark_read(query).await
    }

    async fn mark_read_if_unchanged(&self, preview: &MarkReadPreview) -> Result<MarkReadOutcome> {
        self.enter("mark_read_if_unchanged")?;
        self.inner.mark_read_if_unchanged(preview).await
    }

    async fn load_reader_entry(&self, entry_id: i64) -> Result<ReaderEntrySnapshot> {
        self.enter("load_reader_entry")?;
        self.inner.load_reader_entry(entry_id).await
    }

    async fn localize_entry_assets(&self, entry_id: i64) -> Result<bool> {
        self.enter("localize_entry_assets")?;
        self.inner.localize_entry_assets(entry_id).await
    }

    async fn load_feeds_snapshot(&self) -> Result<FeedsSnapshotOutcome> {
        self.enter("load_feeds_snapshot")?;
        self.inner.load_feeds_snapshot().await
    }

    async fn add_subscription(
        &self,
        raw_url: &str,
        fallback_site_url: Option<Url>,
    ) -> Result<AddSubscriptionOutcome> {
        self.enter("add_subscription")?;
        self.inner.add_subscription(raw_url, fallback_site_url).await
    }

    async fn refresh_all(&self) -> Result<RefreshAllExecutionOutcome> {
        self.enter("refresh_all")?;
        self.inner.refresh_all().await
    }

    async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedExecutionOutcome> {
        self.enter("refresh_feed")?;
        self.inner.refresh_feed(feed_id).await
    }

    async fn remove_feed(&self, feed_id: i64) -> Result<()> {
        self.enter("remove_feed")?;
        self.inner.remove_feed(feed_id).await
    }

    async fn export_config_json(&self) -> Result<String> {
        self.enter("export_config_json")?;
        self.inner.export_config_json().await
    }

    async fn import_config_json(
        &self,
        raw: &str,
        expected: &ConfigFingerprint,
    ) -> Result<ConfigImportOutcome> {
        self.enter("import_config_json")?;
        self.inner.import_config_json(raw, expected).await
    }

    async fn export_opml(&self) -> Result<String> {
        self.enter("export_opml")?;
        self.inner.export_opml().await
    }

    async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome> {
        self.enter("import_opml")?;
        self.inner.import_opml(raw).await
    }
}
