//! The [`Backend`] over this device's own use cases, shared by every host that runs them in
//! process: the desktop app and Android ([`NativeHost`]), the browser build (its host lives in
//! `rssr-app`), and later the server.
//!
//! What all hosts do the same way lives here: adding a subscription (prepare, then save and
//! refresh), refreshing (one batch at a time, see [`refresh_flight`]), turning refresh results
//! into what the UI shows, and the periodic refresh loop. What differs is behind [`LocalHost`]:
//! running background work, sleeping, the clock, the remote configuration store, image
//! localization, refresh concurrency, and whether the network may be used at all.

mod auto_refresh;
#[cfg(not(target_arch = "wasm32"))]
mod native_host;
mod refresh_flight;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use rssr_api::{Backend, Result as BackendResult};
use rssr_application::{
    AddSubscriptionInput, AddSubscriptionLifecycleInput, AddSubscriptionOutcome, AppUseCases,
    ConfigImportOutcome, EntriesBootstrapInput, EntriesBootstrapOutcome, EntriesListOutcome,
    EntryFlagOutcome, FeedsSnapshotOutcome, OpmlImportOutcome, PrepareSubscriptionOutcome,
    ReaderEntrySnapshot, RefreshAllExecutionOutcome, RefreshAllInput, RefreshAllOutcome,
    RefreshFailureItem, RefreshFeedExecutionOutcome, RefreshFeedOutcome, RefreshFeedResult,
    RefreshLocalizedEntry, RemoteConfigPullOutcome, RemoteConfigPushOutcome, RemoteConfigStore,
    RemoveSubscriptionInput, SettingsPatch, StartupTarget,
};
use rssr_domain::{
    ConfigFingerprint, EntriesWorkspaceState, EntryQuery, MarkReadOutcome, MarkReadPreview,
    UserSettings,
};
use time::OffsetDateTime;

#[cfg(not(target_arch = "wasm32"))]
pub use native_host::{NativeHost, native_local_backend};
use refresh_flight::RefreshFlight;

/// Background work a host runs: `Send` on native targets, anything on wasm32.
#[cfg(not(target_arch = "wasm32"))]
pub type LocalTask = Pin<Box<dyn Future<Output = ()> + Send>>;
/// Background work a host runs: `Send` on native targets, anything on wasm32.
#[cfg(target_arch = "wasm32")]
pub type LocalTask = Pin<Box<dyn Future<Output = ()>>>;

/// What a host must be to sit behind a [`Backend`]: shareable across threads on native targets
/// (the backend's futures are `Send` there), anything on wasm32.
#[cfg(not(target_arch = "wasm32"))]
pub trait HostBound: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync> HostBound for T {}
/// What a host must be to sit behind a [`Backend`]: shareable across threads on native targets
/// (the backend's futures are `Send` there), anything on wasm32.
#[cfg(target_arch = "wasm32")]
pub trait HostBound {}
#[cfg(target_arch = "wasm32")]
impl<T> HostBound for T {}

/// What differs between the hosts that run the use cases in process.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait LocalHost: HostBound + 'static {
    /// Subscriptions refreshed at once when refreshing all of them.
    fn refresh_concurrency(&self) -> usize;
    /// Fails when this host may not use the network (the Pages demo).
    fn require_network(&self) -> Result<()> {
        Ok(())
    }
    /// Whether the periodic refresh runs on this host.
    fn runs_auto_refresh(&self) -> bool {
        true
    }
    fn now_utc(&self) -> OffsetDateTime;
    /// Runs `task` in the background.
    fn spawn(&self, task: LocalTask);
    async fn sleep(&self, duration: Duration);
    /// The store a configuration is pushed to or pulled from.
    fn remote_config_store(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<Box<dyn RemoteConfigStore>>;
    /// A refresh stored new or changed bodies of `feed_id`; they may hold images to keep locally.
    fn feed_refreshed(&self, _feed_id: i64, _entries: Vec<RefreshLocalizedEntry>) {}
    /// Stores the entry's images locally where this host can; `true` when it did.
    async fn localize_entry_assets(&self, _entry_id: i64) -> Result<bool> {
        Ok(false)
    }
}

/// The use cases of this device behind the [`Backend`] interface. Cheap to clone.
pub struct LocalBackend<H> {
    inner: Arc<Inner<H>>,
}

impl<H> Clone for LocalBackend<H> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

struct Inner<H> {
    use_cases: AppUseCases,
    host: H,
    refresh_flight: RefreshFlight,
    auto_refresh_started: AtomicBool,
}

/// How long the periodic refresh waits after failing to read the settings.
const AUTO_REFRESH_RETRY_DELAY: Duration = Duration::from_secs(30);

impl<H: LocalHost> LocalBackend<H> {
    pub fn new(use_cases: AppUseCases, host: H) -> Self {
        Self {
            inner: Arc::new(Inner {
                use_cases,
                host,
                refresh_flight: RefreshFlight::default(),
                auto_refresh_started: AtomicBool::new(false),
            }),
        }
    }

    pub fn use_cases(&self) -> &AppUseCases {
        &self.inner.use_cases
    }

    pub fn host(&self) -> &H {
        &self.inner.host
    }

    /// The display names of failed subscriptions, read once after the refresh; a failure to
    /// read them only leaves the names out (the address is shown instead).
    async fn named(&self, mut failures: Vec<RefreshFailureItem>) -> Vec<RefreshFailureItem> {
        if failures.is_empty() {
            return failures;
        }
        match self.inner.use_cases.feeds_snapshot_service.load_snapshot().await {
            Ok(snapshot) => {
                for failure in &mut failures {
                    failure.name = snapshot
                        .feeds
                        .iter()
                        .find(|feed| feed.id == failure.feed_id)
                        .map(|feed| feed.title.clone());
                }
            }
            Err(error) => tracing::warn!(error = %format!("{error:#}"), "读取订阅名称失败"),
        }
        failures
    }

    async fn handle_refresh_all_outcome(
        &self,
        outcome: RefreshAllOutcome,
    ) -> RefreshAllExecutionOutcome {
        let summary = outcome.summary();
        for feed in outcome.feeds {
            match feed.result {
                RefreshFeedResult::Updated { localization_entries, .. } => {
                    tracing::debug!(feed_id = feed.feed_id, "刷新订阅成功");
                    self.inner.host.feed_refreshed(feed.feed_id, localization_entries);
                }
                RefreshFeedResult::NotModified => {
                    tracing::debug!(feed_id = feed.feed_id, "订阅未变化");
                }
                RefreshFeedResult::Failed { message, .. } => {
                    tracing::warn!(feed_id = feed.feed_id, error = %message, "刷新订阅失败");
                }
            }
        }
        RefreshAllExecutionOutcome {
            inserted_count: summary.inserted_count,
            total_count: summary.total_count as u64,
            failed_count: summary.failed_count as u64,
            failures: self.named(summary.failures).await,
        }
    }

    /// Hands new entries to the host's image localization before turning the outcome into what
    /// the user interface receives.
    async fn handle_refresh_feed_outcome(
        &self,
        outcome: RefreshFeedOutcome,
    ) -> RefreshFeedExecutionOutcome {
        let inserted_count = outcome.inserted_count();
        let failure = outcome.failure_summary();
        match outcome.result {
            RefreshFeedResult::Updated { localization_entries, .. } => {
                self.inner.host.feed_refreshed(outcome.feed_id, localization_entries);
            }
            RefreshFeedResult::NotModified => {}
            RefreshFeedResult::Failed { message, .. } => {
                tracing::warn!(feed_id = outcome.feed_id, error = %message, "刷新订阅失败");
            }
        }
        let failure = self.named(failure.into_iter().collect()).await.pop();
        RefreshFeedExecutionOutcome { inserted_count, failure }
    }

    /// The periodic refresh: refreshes everything when due, then sleeps until next time.
    async fn auto_refresh_loop(self) {
        let mut last_refresh_started_at = None;
        loop {
            let settings = match self.inner.use_cases.settings_service.load().await {
                Ok(settings) => settings,
                Err(error) => {
                    tracing::warn!(error = %error, "读取自动刷新设置失败，稍后重试");
                    self.inner.host.sleep(AUTO_REFRESH_RETRY_DELAY).await;
                    continue;
                }
            };
            let now = self.inner.host.now_utc();
            if auto_refresh::should_trigger(
                last_refresh_started_at,
                settings.refresh_interval_minutes,
                now,
            ) {
                tracing::info!(
                    refresh_interval_minutes = settings.refresh_interval_minutes,
                    "触发后台自动刷新全部订阅"
                );
                if let Err(error) = self.refresh_all().await {
                    tracing::warn!(error = %error, "后台自动刷新失败");
                }
                last_refresh_started_at = Some(now);
            }
            let wait_for = auto_refresh::wait_duration(
                last_refresh_started_at,
                settings.refresh_interval_minutes,
                self.inner.host.now_utc(),
            );
            self.inner.host.sleep(wait_for).await;
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl<H: LocalHost> Backend for LocalBackend<H> {
    async fn load_settings(&self) -> BackendResult<UserSettings> {
        Ok(self.inner.use_cases.settings_service.load().await?)
    }

    async fn update_settings(&self, patch: SettingsPatch) -> BackendResult<UserSettings> {
        Ok(self.inner.use_cases.settings_service.update(patch).await?)
    }

    fn ensure_auto_refresh_started(&self) {
        if !self.inner.host.runs_auto_refresh()
            || self.inner.auto_refresh_started.swap(true, Ordering::SeqCst)
        {
            return;
        }
        self.inner.host.spawn(Box::pin(self.clone().auto_refresh_loop()));
    }

    async fn resolve_startup_target(&self) -> BackendResult<StartupTarget> {
        Ok(self.inner.use_cases.startup_service.resolve_startup_target().await?)
    }

    async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> BackendResult<RemoteConfigPushOutcome> {
        self.inner.host.require_network()?;
        let remote = self.inner.host.remote_config_store(endpoint, remote_path)?;
        Ok(self.inner.use_cases.import_export_service.push_remote_config(remote.as_ref()).await?)
    }

    async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> BackendResult<RemoteConfigPullOutcome> {
        self.inner.host.require_network()?;
        let remote = self.inner.host.remote_config_store(endpoint, remote_path)?;
        Ok(self.inner.use_cases.import_export_service.pull_remote_config(remote.as_ref()).await?)
    }

    async fn bootstrap_entries(
        &self,
        input: EntriesBootstrapInput,
    ) -> BackendResult<EntriesBootstrapOutcome> {
        Ok(self.inner.use_cases.entries_workspace_service.bootstrap(input).await?)
    }

    async fn remember_opened_feed(&self, feed_id: i64) -> BackendResult<()> {
        Ok(self.inner.use_cases.entries_workspace_service.remember_opened_feed(feed_id).await?)
    }

    async fn save_entries_workspace_if_changed(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> BackendResult<bool> {
        Ok(self
            .inner
            .use_cases
            .entries_workspace_service
            .save_workspace_if_changed(workspace)
            .await?
            .changed)
    }

    async fn list_entries(&self, query: &EntryQuery) -> BackendResult<EntriesListOutcome> {
        Ok(self.inner.use_cases.entries_list_service.list_entries(query).await?)
    }

    async fn set_entry_read(&self, entry_id: i64, read: bool) -> BackendResult<EntryFlagOutcome> {
        Ok(self.inner.use_cases.entry_flags_service.set_read(entry_id, read).await?)
    }

    async fn set_entry_starred(
        &self,
        entry_id: i64,
        starred: bool,
    ) -> BackendResult<EntryFlagOutcome> {
        Ok(self.inner.use_cases.entry_flags_service.set_starred(entry_id, starred).await?)
    }

    async fn preview_mark_read(&self, query: &EntryQuery) -> BackendResult<MarkReadPreview> {
        Ok(self.inner.use_cases.entries_list_service.preview_mark_read(query).await?)
    }

    async fn mark_read_if_unchanged(
        &self,
        preview: &MarkReadPreview,
    ) -> BackendResult<MarkReadOutcome> {
        Ok(self.inner.use_cases.entries_list_service.mark_read_if_unchanged(preview).await?)
    }

    async fn load_reader_entry(&self, entry_id: i64) -> BackendResult<ReaderEntrySnapshot> {
        Ok(self.inner.use_cases.reader_service.load_entry(entry_id).await?)
    }

    async fn localize_entry_assets(&self, entry_id: i64) -> BackendResult<bool> {
        Ok(self.inner.host.localize_entry_assets(entry_id).await?)
    }

    async fn load_feeds_snapshot(&self) -> BackendResult<FeedsSnapshotOutcome> {
        Ok(self.inner.use_cases.feeds_snapshot_service.load_snapshot().await?)
    }

    async fn add_subscription(
        &self,
        raw_url: &str,
        fallback_site_url: Option<url::Url>,
    ) -> BackendResult<AddSubscriptionOutcome> {
        self.inner.host.require_network()?;
        let workflow = &self.inner.use_cases.subscription_workflow;
        let prepared = match workflow.prepare_subscription(raw_url).await? {
            PrepareSubscriptionOutcome::Ready(prepared) => {
                prepared.with_fallback_site_url(fallback_site_url)
            }
            PrepareSubscriptionOutcome::NeedsSelection { page_url, candidates } => {
                return Ok(AddSubscriptionOutcome::NeedsSelection { page_url, candidates });
            }
        };
        let outcome = workflow
            .add_prepared_subscription(
                AddSubscriptionLifecycleInput {
                    subscription: AddSubscriptionInput {
                        url: raw_url.to_string(),
                        title: None,
                        folder: None,
                    },
                    refresh_after_add: true,
                },
                prepared,
            )
            .await
            .context("保存订阅失败")?;
        let refresh = outcome.first_refresh.expect("refresh_after_add produces refresh outcome");
        match self.handle_refresh_feed_outcome(refresh).await.failure {
            Some(failure) => Ok(AddSubscriptionOutcome::SavedRefreshFailed { failure }),
            None => Ok(AddSubscriptionOutcome::SavedAndRefreshed),
        }
    }

    async fn refresh_all(&self) -> BackendResult<RefreshAllExecutionOutcome> {
        self.inner.host.require_network()?;
        self.inner
            .refresh_flight
            .run(async {
                let outcome = self
                    .inner
                    .use_cases
                    .refresh_service
                    .refresh_all(RefreshAllInput {
                        max_concurrency: self.inner.host.refresh_concurrency(),
                    })
                    .await?;
                Ok(self.handle_refresh_all_outcome(outcome).await)
            })
            .await
    }

    async fn refresh_feed(&self, feed_id: i64) -> BackendResult<RefreshFeedExecutionOutcome> {
        self.inner.host.require_network()?;
        let outcome = self.inner.use_cases.refresh_service.refresh_feed(feed_id).await?;
        Ok(self.handle_refresh_feed_outcome(outcome).await)
    }

    async fn remove_feed(&self, feed_id: i64) -> BackendResult<()> {
        Ok(self
            .inner
            .use_cases
            .subscription_workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id, purge_entries: true })
            .await?)
    }

    async fn export_config_json(&self) -> BackendResult<String> {
        Ok(self.inner.use_cases.import_export_service.export_config_json().await?)
    }

    async fn import_config_json(
        &self,
        raw: &str,
        expected: &ConfigFingerprint,
    ) -> BackendResult<ConfigImportOutcome> {
        Ok(self.inner.use_cases.import_export_service.import_config_json(raw, expected).await?)
    }

    async fn export_opml(&self) -> BackendResult<String> {
        Ok(self.inner.use_cases.import_export_service.export_opml().await?)
    }

    async fn import_opml(&self, raw: &str) -> BackendResult<OpmlImportOutcome> {
        Ok(self.inner.use_cases.import_export_service.import_opml(raw).await?)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::composition::compose_library_use_cases;
    use crate::store::LibraryStore;

    /// Records what the backend asks of its host; runs nothing.
    struct RecordingHost {
        runs_auto_refresh: bool,
        spawned: Mutex<usize>,
    }

    #[async_trait::async_trait]
    impl LocalHost for RecordingHost {
        fn refresh_concurrency(&self) -> usize {
            1
        }
        fn runs_auto_refresh(&self) -> bool {
            self.runs_auto_refresh
        }
        fn now_utc(&self) -> OffsetDateTime {
            OffsetDateTime::UNIX_EPOCH
        }
        fn spawn(&self, _task: LocalTask) {
            *self.spawned.lock().unwrap() += 1;
        }
        async fn sleep(&self, _duration: Duration) {}
        fn remote_config_store(&self, _: &str, _: &str) -> Result<Box<dyn RemoteConfigStore>> {
            anyhow::bail!("unused")
        }
    }

    async fn recording_backend(runs_auto_refresh: bool) -> LocalBackend<RecordingHost> {
        let store = LibraryStore::open_in_memory().await.unwrap();
        LocalBackend::new(
            compose_library_use_cases(store).use_cases,
            RecordingHost { runs_auto_refresh, spawned: Mutex::new(0) },
        )
    }

    #[tokio::test]
    async fn the_periodic_refresh_starts_once_and_not_where_the_host_does_not_run_it() {
        let backend = recording_backend(true).await;
        backend.ensure_auto_refresh_started();
        backend.clone().ensure_auto_refresh_started();
        assert_eq!(*backend.host().spawned.lock().unwrap(), 1, "once per backend, clones included");

        let disabled = recording_backend(false).await;
        disabled.ensure_auto_refresh_started();
        assert_eq!(*disabled.host().spawned.lock().unwrap(), 0);
        // Not marked as started: a host that later allows it is not locked out.
        assert!(!disabled.inner.auto_refresh_started.load(Ordering::SeqCst));
    }

    /// Each failure reaches the UI with its code; only the kinds meant for the user keep their
    /// own text. (`unavailable` needs a busy library and `network` a remote; both are covered
    /// where they arise: the store's error mapping and the remote adapters.)
    #[tokio::test]
    async fn failures_reach_the_ui_with_their_codes() {
        use rssr_api::ErrorCode;

        let backend = recording_backend(false).await;
        let invalid =
            rssr_application::SettingsPatch { entries_page_size: Some(0), ..Default::default() };
        let error = backend.update_settings(invalid).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert!(error.message.starts_with("文章页每页数量"), "{}", error.message);

        let error = backend.remove_feed(999).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::NotFound);
        let error = backend.refresh_feed(999).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::NotFound);

        let error = backend.add_subscription("not a url", None).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        let error = backend.import_opml("<opml").await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);

        let stale = backend.load_feeds_snapshot().await.unwrap().fingerprint;
        backend
            .update_settings(rssr_application::SettingsPatch {
                refresh_interval_minutes: Some(45),
                ..Default::default()
            })
            .await
            .unwrap();
        let package = backend.export_config_json().await.unwrap();
        let error = backend.import_config_json(&package, &stale).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::Conflict);
        let error = backend.import_config_json("{", &stale).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert!(error.message.contains("JSON"), "{}", error.message);
    }

    /// A failed refresh reaches the UI as a structured item with the subscription's name, so
    /// the GUI and the CLI word it themselves.
    #[tokio::test]
    async fn refresh_failures_carry_the_subscription_name() {
        let backend = recording_backend(false).await;
        // Nothing listens on the discard port: the fetch fails at once, without a network.
        let feed = backend
            .use_cases()
            .feed_service
            .add_subscription(&AddSubscriptionInput {
                url: "http://127.0.0.1:9/feed.xml".into(),
                title: Some("Named".into()),
                folder: None,
            })
            .await
            .unwrap();

        let failure = backend.refresh_feed(feed.id).await.unwrap().failure.expect("failed");
        assert_eq!((failure.feed_id, failure.name.as_deref()), (feed.id, Some("Named")));
        assert_eq!(failure.kind, rssr_application::ErrorKind::Network);

        let all = backend.refresh_all().await.unwrap();
        assert_eq!((all.total_count, all.failed_count), (1, 1));
        assert_eq!(all.failures[0].name.as_deref(), Some("Named"));
    }

    /// An archive cutoff must be a whole second: the library compares RFC3339 text, so a
    /// fractional one would not match the browser's comparison. It is refused, not compared.
    #[tokio::test]
    async fn a_fractional_archive_cutoff_is_refused() {
        use rssr_api::ErrorCode;
        use rssr_domain::ArchiveFilter;

        let backend = recording_backend(false).await;
        let whole = OffsetDateTime::UNIX_EPOCH + Duration::from_secs(86_400);
        let query = |cutoff| EntryQuery {
            archive_filter: ArchiveFilter::ExcludeArchived { cutoff },
            ..EntryQuery::default()
        };
        backend.list_entries(&query(whole)).await.expect("a whole second is accepted");

        let fractional = whole + Duration::from_millis(250);
        let error = backend.list_entries(&query(fractional)).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        let error = backend.preview_mark_read(&query(fractional)).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
    }
}
