//! The backend the UI runs against: the use cases of this device, in-process.

use std::sync::Arc;

use anyhow::Result;
use rssr_api::{
    AddSubscriptionOutcome, Backend, RefreshAllExecutionOutcome, RefreshFeedExecutionOutcome,
};
use rssr_application::{
    AppUseCases, AppliedRemoteConfigOutcome, ConfigImportOutcome, EntriesBootstrapInput,
    EntriesBootstrapOutcome, EntriesListOutcome, FeedsSnapshotOutcome, OpmlImportOutcome,
    ReaderEntrySnapshot, RemoteConfigPushOutcome, RemoveSubscriptionInput, StartupTarget,
    ToggleEntryReadInput, ToggleEntryReadOutcome, ToggleEntryStarredInput,
    ToggleEntryStarredOutcome, ToggleReadInput, ToggleReadOutcome, ToggleStarredInput,
    ToggleStarredOutcome,
};
use rssr_domain::{
    EntriesWorkspaceState, EntryQuery, MarkReadOutcome, MarkReadPreview, UserSettings,
};

use crate::bootstrap::{AppServices, HostCapabilities};

/// The backend of this process.
// wasm32 runs on one thread: an `Arc` of a non-`Send` backend is fine there.
#[cfg_attr(target_arch = "wasm32", allow(clippy::arc_with_non_send_sync))]
pub(crate) async fn backend() -> Result<Arc<dyn Backend>> {
    let host = host().await?;
    Ok(Arc::new(LocalBackend {
        use_cases: host.use_cases(),
        host_capabilities: host.host_capabilities(),
    }))
}

async fn host() -> Result<Arc<AppServices>> {
    #[cfg(test)]
    if let Some(host) = test_host::current() {
        return Ok(host);
    }
    AppServices::shared().await
}

/// The use cases of this device and the host's own capabilities (refresh orchestration, image
/// localization, WebDAV, the auto-refresh loop).
struct LocalBackend {
    use_cases: AppUseCases,
    host_capabilities: HostCapabilities,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl Backend for LocalBackend {
    async fn load_settings(&self) -> Result<UserSettings> {
        self.use_cases.settings_service.load().await
    }

    async fn save_settings(&self, settings: &UserSettings) -> Result<()> {
        self.use_cases.settings_service.save(settings).await?;
        Ok(())
    }

    fn ensure_auto_refresh_started(&self) {
        self.host_capabilities.auto_refresh.ensure_started();
    }

    async fn resolve_startup_target(&self) -> Result<StartupTarget> {
        self.use_cases.startup_service.resolve_startup_target().await
    }

    async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPushOutcome> {
        self.host_capabilities.remote_config.push(endpoint, remote_path).await
    }

    async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<AppliedRemoteConfigOutcome> {
        let outcome = self.host_capabilities.remote_config.pull(endpoint, remote_path).await?;
        self.use_cases.settings_sync_service.apply_remote_pull(outcome).await
    }

    async fn bootstrap_entries(
        &self,
        input: EntriesBootstrapInput,
    ) -> Result<EntriesBootstrapOutcome> {
        self.use_cases.entries_workspace_service.bootstrap(input).await
    }

    async fn save_entries_workspace_if_changed(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> Result<bool> {
        Ok(self
            .use_cases
            .entries_workspace_service
            .save_workspace_if_changed(workspace)
            .await?
            .changed)
    }

    async fn list_entries(&self, query: &EntryQuery) -> Result<EntriesListOutcome> {
        self.use_cases.entries_list_service.list_entries(query).await
    }

    async fn toggle_entry_read(
        &self,
        input: ToggleEntryReadInput,
    ) -> Result<ToggleEntryReadOutcome> {
        self.use_cases.entries_list_service.toggle_read(input).await
    }

    async fn toggle_entry_starred(
        &self,
        input: ToggleEntryStarredInput,
    ) -> Result<ToggleEntryStarredOutcome> {
        self.use_cases.entries_list_service.toggle_starred(input).await
    }

    async fn preview_mark_read(&self, query: &EntryQuery) -> Result<MarkReadPreview> {
        self.use_cases.entries_list_service.preview_mark_read(query).await
    }

    async fn mark_read_if_unchanged(&self, preview: &MarkReadPreview) -> Result<MarkReadOutcome> {
        self.use_cases.entries_list_service.mark_read_if_unchanged(preview).await
    }

    async fn load_reader_entry(&self, entry_id: i64) -> Result<ReaderEntrySnapshot> {
        self.use_cases.reader_service.load_entry(entry_id).await
    }

    async fn toggle_reader_read(&self, input: ToggleReadInput) -> Result<ToggleReadOutcome> {
        self.use_cases.reader_service.toggle_read(input).await
    }

    async fn toggle_reader_starred(
        &self,
        input: ToggleStarredInput,
    ) -> Result<ToggleStarredOutcome> {
        self.use_cases.reader_service.toggle_starred(input).await
    }

    async fn localize_entry_assets(&self, entry_id: i64) -> Result<bool> {
        Ok(self.host_capabilities.reader_assets.localize_entry_assets(entry_id).await?.localized)
    }

    async fn load_feeds_snapshot(&self) -> Result<FeedsSnapshotOutcome> {
        self.use_cases.feeds_snapshot_service.load_snapshot().await
    }

    async fn add_subscription(
        &self,
        raw_url: &str,
        fallback_site_url: Option<url::Url>,
    ) -> Result<AddSubscriptionOutcome> {
        self.host_capabilities.refresh.add_subscription(raw_url, fallback_site_url).await
    }

    async fn refresh_all(&self) -> Result<RefreshAllExecutionOutcome> {
        self.host_capabilities.refresh.refresh_all().await
    }

    async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedExecutionOutcome> {
        self.host_capabilities.refresh.refresh_feed(feed_id).await
    }

    async fn remove_feed(&self, feed_id: i64) -> Result<()> {
        self.use_cases
            .subscription_workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id, purge_entries: true })
            .await
    }

    async fn export_config_json(&self) -> Result<String> {
        self.use_cases.import_export_service.export_config_json().await
    }

    async fn import_config_json(&self, raw: &str) -> Result<ConfigImportOutcome> {
        self.use_cases.import_export_service.import_config_json(raw).await
    }

    async fn export_opml(&self) -> Result<String> {
        self.use_cases.import_export_service.export_opml().await
    }

    async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome> {
        self.use_cases.import_export_service.import_opml(raw).await
    }
}

/// Runs UI commands of the current test thread against chosen services instead of the
/// process-wide ones (tests run on a current-thread runtime).
#[cfg(test)]
pub(crate) mod test_host {
    use std::cell::RefCell;
    use std::sync::Arc;

    use crate::bootstrap::AppServices;

    thread_local! {
        static HOST: RefCell<Option<Arc<AppServices>>> = const { RefCell::new(None) };
    }

    pub(crate) fn current() -> Option<Arc<AppServices>> {
        HOST.with(|host| host.borrow().clone())
    }

    /// Installs `host` until the returned guard is dropped.
    pub(crate) fn install(host: Arc<AppServices>) -> impl Drop {
        struct Restore(Option<Arc<AppServices>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                let previous = self.0.take();
                HOST.with(|host| *host.borrow_mut() = previous);
            }
        }
        Restore(HOST.with(|current| current.borrow_mut().replace(host)))
    }
}
