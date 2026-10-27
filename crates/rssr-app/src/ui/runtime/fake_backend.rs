//! A backend for the UI's tests (C4b): it answers like the library it wraps, except for the
//! operations told to fail, which fail with the error they were given. The in-process library
//! never produces the errors the page must recover from (`outcome_unknown`, `unauthenticated`,
//! `build_mismatch`); this one does, on demand. It also counts the operations that ran, and holds
//! the jobs told to wait (a job that is still running, C4b item 3).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    holds: Mutex<HashMap<&'static str, Arc<Hold>>>,
    /// Failures for the next call only, answered after its hold.
    once: Mutex<HashMap<&'static str, BackendError>>,
}

/// A job held before it answers, until released. It knows whether a caller is waiting on it and
/// whether a caller stopped waiting (its call was dropped).
pub(crate) struct Hold {
    gate: tokio::sync::Semaphore,
    waiting: AtomicUsize,
    abandoned: AtomicUsize,
}

impl Hold {
    /// Lets one call through.
    pub(crate) fn release(&self) {
        self.gate.add_permits(1);
    }

    /// Calls waiting on it now.
    pub(crate) fn waiting(&self) -> usize {
        self.waiting.load(Ordering::SeqCst)
    }

    /// Calls dropped while they waited.
    pub(crate) fn abandoned(&self) -> usize {
        self.abandoned.load(Ordering::SeqCst)
    }
}

impl Scripted {
    pub(crate) fn over(inner: Arc<dyn Backend + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            failures: Mutex::default(),
            ran: Mutex::default(),
            holds: Mutex::default(),
            once: Mutex::default(),
        })
    }

    /// From now on `operation` waits before it answers, until released.
    pub(crate) fn hold(&self, operation: &'static str) -> Arc<Hold> {
        let hold = Arc::new(Hold {
            gate: tokio::sync::Semaphore::new(0),
            waiting: AtomicUsize::new(0),
            abandoned: AtomicUsize::new(0),
        });
        self.holds.lock().unwrap().insert(operation, Arc::clone(&hold));
        hold
    }

    /// The next call of `operation` (a job, or another held operation) fails with `error` after
    /// its hold: it was already on its way when the failure was decided.
    pub(crate) fn fail_once(&self, operation: &'static str, error: BackendError) {
        self.once.lock().unwrap().insert(operation, error);
    }

    /// Waits while `operation` is held, then answers a one-shot failure if one was set.
    async fn held(&self, operation: &'static str) -> Result<()> {
        self.wait_while_held(operation).await;
        match self.once.lock().unwrap().remove(operation) {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    async fn wait_while_held(&self, operation: &'static str) {
        let Some(hold) = self.holds.lock().unwrap().get(operation).cloned() else { return };
        struct Waiting<'a> {
            hold: &'a Hold,
            answered: bool,
        }
        impl Drop for Waiting<'_> {
            fn drop(&mut self) {
                self.hold.waiting.fetch_sub(1, Ordering::SeqCst);
                if !self.answered {
                    self.hold.abandoned.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
        hold.waiting.fetch_add(1, Ordering::SeqCst);
        let mut waiting = Waiting { hold: &hold, answered: false };
        hold.gate.acquire().await.expect("never closed").forget();
        waiting.answered = true;
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
        self.held("load_settings").await?;
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
        self.held("push_remote_config").await?;
        self.inner.push_remote_config(endpoint, remote_path).await
    }

    async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPullOutcome> {
        self.enter("pull_remote_config")?;
        self.held("pull_remote_config").await?;
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
        self.held("save_entries_workspace_if_changed").await?;
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
        self.held("add_subscription").await?;
        self.inner.add_subscription(raw_url, fallback_site_url).await
    }

    async fn refresh_all(&self) -> Result<RefreshAllExecutionOutcome> {
        self.enter("refresh_all")?;
        self.held("refresh_all").await?;
        self.inner.refresh_all().await
    }

    async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedExecutionOutcome> {
        self.enter("refresh_feed")?;
        self.held("refresh_feed").await?;
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
