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
use rssr_api::{
    AddSubscriptionOutcome, Backend, RefreshAllExecutionOutcome, RefreshFeedExecutionOutcome,
};
use rssr_application::{
    AddSubscriptionInput, AddSubscriptionLifecycleInput, AppUseCases, AppliedRemoteConfigOutcome,
    ConfigImportOutcome, EntriesBootstrapInput, EntriesBootstrapOutcome, EntriesListOutcome,
    FeedsSnapshotOutcome, OpmlImportOutcome, PrepareSubscriptionOutcome, ReaderEntrySnapshot,
    RefreshAllInput, RefreshAllOutcome, RefreshFeedOutcome, RefreshFeedResult,
    RefreshLocalizedEntry, RemoteConfigPushOutcome, RemoteConfigStore, RemoveSubscriptionInput,
    StartupTarget, ToggleEntryReadInput, ToggleEntryReadOutcome, ToggleEntryStarredInput,
    ToggleEntryStarredOutcome, ToggleReadInput, ToggleReadOutcome, ToggleStarredInput,
    ToggleStarredOutcome,
};
use rssr_domain::{
    EntriesWorkspaceState, EntryQuery, MarkReadOutcome, MarkReadPreview, UserSettings,
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

    fn handle_refresh_all_outcome(&self, outcome: RefreshAllOutcome) -> RefreshAllExecutionOutcome {
        let summary = outcome.summary();
        let failure_lines = summary.joined_failure_lines();
        for feed in outcome.feeds {
            match feed.result {
                RefreshFeedResult::Updated { localization_entries, .. } => {
                    tracing::debug!(feed_id = feed.feed_id, "刷新订阅成功");
                    self.inner.host.feed_refreshed(feed.feed_id, localization_entries);
                }
                RefreshFeedResult::NotModified => {
                    tracing::debug!(feed_id = feed.feed_id, "订阅未变化");
                }
                RefreshFeedResult::Failed { message } => {
                    tracing::warn!(feed_id = feed.feed_id, error = %message, "刷新订阅失败");
                }
            }
        }
        RefreshAllExecutionOutcome {
            failure_message: failure_lines,
            inserted_count: summary.inserted_count,
            total_count: summary.total_count,
            failed_count: summary.failed_count,
        }
    }

    fn handle_refresh_feed_outcome(
        &self,
        outcome: RefreshFeedOutcome,
    ) -> RefreshFeedExecutionOutcome {
        let inserted_count = outcome.inserted_count();
        let failure_message = outcome.failure_line();
        match outcome.result {
            RefreshFeedResult::Updated { localization_entries, .. } => {
                self.inner.host.feed_refreshed(outcome.feed_id, localization_entries);
                RefreshFeedExecutionOutcome { inserted_count, failure_message: None }
            }
            RefreshFeedResult::NotModified => {
                RefreshFeedExecutionOutcome { inserted_count, failure_message: None }
            }
            RefreshFeedResult::Failed { message } => {
                tracing::warn!(feed_id = outcome.feed_id, error = %message, "刷新订阅失败");
                RefreshFeedExecutionOutcome {
                    inserted_count,
                    failure_message: Some(
                        failure_message.unwrap_or_else(|| "刷新订阅失败".to_string()),
                    ),
                }
            }
        }
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
    async fn load_settings(&self) -> Result<UserSettings> {
        self.inner.use_cases.settings_service.load().await
    }

    async fn save_settings(&self, settings: &UserSettings) -> Result<()> {
        self.inner.use_cases.settings_service.save(settings).await?;
        Ok(())
    }

    fn ensure_auto_refresh_started(&self) {
        if !self.inner.host.runs_auto_refresh()
            || self.inner.auto_refresh_started.swap(true, Ordering::SeqCst)
        {
            return;
        }
        self.inner.host.spawn(Box::pin(self.clone().auto_refresh_loop()));
    }

    async fn resolve_startup_target(&self) -> Result<StartupTarget> {
        self.inner.use_cases.startup_service.resolve_startup_target().await
    }

    async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPushOutcome> {
        self.inner.host.require_network()?;
        let remote = self.inner.host.remote_config_store(endpoint, remote_path)?;
        self.inner.use_cases.import_export_service.push_remote_config(remote.as_ref()).await
    }

    async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<AppliedRemoteConfigOutcome> {
        self.inner.host.require_network()?;
        let remote = self.inner.host.remote_config_store(endpoint, remote_path)?;
        let outcome =
            self.inner.use_cases.import_export_service.pull_remote_config(remote.as_ref()).await?;
        self.inner.use_cases.settings_sync_service.apply_remote_pull(outcome).await
    }

    async fn bootstrap_entries(
        &self,
        input: EntriesBootstrapInput,
    ) -> Result<EntriesBootstrapOutcome> {
        self.inner.use_cases.entries_workspace_service.bootstrap(input).await
    }

    async fn save_entries_workspace_if_changed(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> Result<bool> {
        Ok(self
            .inner
            .use_cases
            .entries_workspace_service
            .save_workspace_if_changed(workspace)
            .await?
            .changed)
    }

    async fn list_entries(&self, query: &EntryQuery) -> Result<EntriesListOutcome> {
        self.inner.use_cases.entries_list_service.list_entries(query).await
    }

    async fn toggle_entry_read(
        &self,
        input: ToggleEntryReadInput,
    ) -> Result<ToggleEntryReadOutcome> {
        self.inner.use_cases.entries_list_service.toggle_read(input).await
    }

    async fn toggle_entry_starred(
        &self,
        input: ToggleEntryStarredInput,
    ) -> Result<ToggleEntryStarredOutcome> {
        self.inner.use_cases.entries_list_service.toggle_starred(input).await
    }

    async fn preview_mark_read(&self, query: &EntryQuery) -> Result<MarkReadPreview> {
        self.inner.use_cases.entries_list_service.preview_mark_read(query).await
    }

    async fn mark_read_if_unchanged(&self, preview: &MarkReadPreview) -> Result<MarkReadOutcome> {
        self.inner.use_cases.entries_list_service.mark_read_if_unchanged(preview).await
    }

    async fn load_reader_entry(&self, entry_id: i64) -> Result<ReaderEntrySnapshot> {
        self.inner.use_cases.reader_service.load_entry(entry_id).await
    }

    async fn toggle_reader_read(&self, input: ToggleReadInput) -> Result<ToggleReadOutcome> {
        self.inner.use_cases.reader_service.toggle_read(input).await
    }

    async fn toggle_reader_starred(
        &self,
        input: ToggleStarredInput,
    ) -> Result<ToggleStarredOutcome> {
        self.inner.use_cases.reader_service.toggle_starred(input).await
    }

    async fn localize_entry_assets(&self, entry_id: i64) -> Result<bool> {
        self.inner.host.localize_entry_assets(entry_id).await
    }

    async fn load_feeds_snapshot(&self) -> Result<FeedsSnapshotOutcome> {
        self.inner.use_cases.feeds_snapshot_service.load_snapshot().await
    }

    async fn add_subscription(
        &self,
        raw_url: &str,
        fallback_site_url: Option<url::Url>,
    ) -> Result<AddSubscriptionOutcome> {
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
        let outcome = self.handle_refresh_feed_outcome(refresh);
        match outcome.failure_message {
            Some(message) => Ok(AddSubscriptionOutcome::SavedRefreshFailed { message }),
            None => Ok(AddSubscriptionOutcome::SavedAndRefreshed),
        }
    }

    async fn refresh_all(&self) -> Result<RefreshAllExecutionOutcome> {
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
                Ok(self.handle_refresh_all_outcome(outcome))
            })
            .await
    }

    async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedExecutionOutcome> {
        self.inner.host.require_network()?;
        let outcome = self.inner.use_cases.refresh_service.refresh_feed(feed_id).await?;
        Ok(self.handle_refresh_feed_outcome(outcome))
    }

    async fn remove_feed(&self, feed_id: i64) -> Result<()> {
        self.inner
            .use_cases
            .subscription_workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id, purge_entries: true })
            .await
    }

    async fn export_config_json(&self) -> Result<String> {
        self.inner.use_cases.import_export_service.export_config_json().await
    }

    async fn import_config_json(&self, raw: &str) -> Result<ConfigImportOutcome> {
        self.inner.use_cases.import_export_service.import_config_json(raw).await
    }

    async fn export_opml(&self) -> Result<String> {
        self.inner.use_cases.import_export_service.export_opml().await
    }

    async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome> {
        self.inner.use_cases.import_export_service.import_opml(raw).await
    }
}
