//! 界面运行时能调用的全部能力，不依赖界面框架。
//!
//! 方法与 `rssr-app` 的 `UiServices` 各端口一一对应：桌面与静态 Web 包在进程内直接调用，
//! 服务端模式经 [`crate::protocol`] 的端点远程调用同一组方法。
use anyhow::Result;
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

use crate::{
    AddSubscriptionOutcome, HostCapabilities, RefreshAllExecutionOutcome,
    RefreshFeedExecutionOutcome,
};

#[derive(Clone)]
pub struct HostServices {
    use_cases: AppUseCases,
    host_capabilities: HostCapabilities,
}

impl HostServices {
    pub fn new(use_cases: AppUseCases, host_capabilities: HostCapabilities) -> Self {
        Self { use_cases, host_capabilities }
    }

    // ---- entries ----

    pub async fn apply_entry_batch(&self, request: rssr_domain::EntryBatchRequest) -> Result<u64> {
        self.use_cases.entries_list_service.apply_entry_batch(request).await
    }

    pub async fn preview_mark_read(&self, query: &EntryQuery) -> Result<MarkReadPreview> {
        self.use_cases.entries_list_service.preview_mark_read(query).await
    }

    pub async fn mark_read_if_unchanged(
        &self,
        preview: &MarkReadPreview,
    ) -> Result<MarkReadOutcome> {
        self.use_cases.entries_list_service.mark_read_if_unchanged(preview).await
    }

    pub async fn bootstrap_entries(
        &self,
        input: EntriesBootstrapInput,
    ) -> Result<EntriesBootstrapOutcome> {
        self.use_cases.entries_workspace_service.bootstrap(input).await
    }

    pub async fn save_workspace_if_changed(
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

    pub async fn list_entries(&self, query: &EntryQuery) -> Result<EntriesListOutcome> {
        self.use_cases.entries_list_service.list_entries(query).await
    }

    pub async fn toggle_entry_read(
        &self,
        input: ToggleEntryReadInput,
    ) -> Result<ToggleEntryReadOutcome> {
        self.use_cases.entries_list_service.toggle_read(input).await
    }

    pub async fn toggle_entry_starred(
        &self,
        input: ToggleEntryStarredInput,
    ) -> Result<ToggleEntryStarredOutcome> {
        self.use_cases.entries_list_service.toggle_starred(input).await
    }

    // ---- shell ----

    pub fn ensure_auto_refresh_started(&self) {
        self.host_capabilities.auto_refresh.ensure_started();
    }

    pub async fn resolve_startup_target(&self) -> Result<StartupTarget> {
        self.use_cases.startup_service.resolve_startup_target().await
    }

    // ---- settings ----

    pub async fn load_settings(&self) -> Result<UserSettings> {
        self.use_cases.settings_service.load().await
    }

    pub async fn save_settings(&self, settings: &UserSettings) -> Result<()> {
        self.use_cases.settings_service.save(settings).await?;
        Ok(())
    }

    pub async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPushOutcome> {
        self.host_capabilities.remote_config.push(endpoint, remote_path).await
    }

    pub async fn pull_remote_config_and_load_settings(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<AppliedRemoteConfigOutcome> {
        let outcome = self.host_capabilities.remote_config.pull(endpoint, remote_path).await?;
        self.use_cases.settings_sync_service.apply_remote_pull(outcome).await
    }

    // ---- reader ----

    pub async fn load_entry(&self, entry_id: i64) -> Result<ReaderEntrySnapshot> {
        self.use_cases.reader_service.load_entry(entry_id).await
    }

    pub async fn toggle_reader_read(&self, input: ToggleReadInput) -> Result<ToggleReadOutcome> {
        self.use_cases.reader_service.toggle_read(input).await
    }

    pub async fn toggle_reader_starred(
        &self,
        input: ToggleStarredInput,
    ) -> Result<ToggleStarredOutcome> {
        self.use_cases.reader_service.toggle_starred(input).await
    }

    pub async fn localize_entry_assets(&self, entry_id: i64) -> Result<bool> {
        Ok(self.host_capabilities.reader_assets.localize_entry_assets(entry_id).await?.localized)
    }

    // ---- feeds ----

    pub async fn feed_edit_details(&self, feed_id: i64) -> Result<rssr_domain::FeedEditDetails> {
        self.use_cases.feed_service.edit_details(feed_id).await
    }
    pub async fn update_subscription(&self, edit: rssr_domain::FeedEditRequest) -> Result<()> {
        self.use_cases.feed_service.update_subscription(edit).await
    }

    pub async fn load_feeds_snapshot(&self) -> Result<FeedsSnapshotOutcome> {
        self.use_cases.feeds_snapshot_service.load_snapshot().await
    }

    pub async fn add_subscription(
        &self,
        raw_url: &str,
        fallback_site_url: Option<url::Url>,
    ) -> Result<AddSubscriptionOutcome> {
        self.host_capabilities.refresh.add_subscription(raw_url, fallback_site_url).await
    }

    pub async fn refresh_all(&self) -> Result<RefreshAllExecutionOutcome> {
        self.host_capabilities.refresh.refresh_all().await
    }

    pub async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedExecutionOutcome> {
        self.host_capabilities.refresh.refresh_feed(feed_id).await
    }

    pub async fn remove_feed(&self, feed_id: i64) -> Result<()> {
        self.use_cases
            .subscription_workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id, purge_entries: true })
            .await
    }

    pub async fn export_config_json(&self) -> Result<String> {
        self.use_cases.import_export_service.export_config_json().await
    }

    pub async fn import_config_json(&self, raw: &str) -> Result<ConfigImportOutcome> {
        self.use_cases.import_export_service.import_config_json(raw).await
    }

    pub async fn export_opml(&self) -> Result<String> {
        self.use_cases.import_export_service.export_opml().await
    }

    pub async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome> {
        self.use_cases.import_export_service.import_opml(raw).await
    }
}
