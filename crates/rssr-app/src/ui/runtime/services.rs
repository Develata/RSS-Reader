use anyhow::Result;
use rssr_application::{
    AppliedRemoteConfigOutcome, ConfigImportOutcome, EntriesBootstrapInput,
    EntriesBootstrapOutcome, EntriesListOutcome, FeedsSnapshotOutcome, OpmlImportOutcome,
    ReaderEntrySnapshot, RemoteConfigPushOutcome, StartupTarget, ToggleEntryReadInput,
    ToggleEntryReadOutcome, ToggleEntryStarredInput, ToggleEntryStarredOutcome, ToggleReadInput,
    ToggleReadOutcome, ToggleStarredInput, ToggleStarredOutcome,
};
use rssr_domain::{EntriesWorkspaceState, EntryQuery, UserSettings};
use rssr_host::HostServices;

use crate::bootstrap::{
    AddSubscriptionOutcome, AppServices, RefreshAllExecutionOutcome, RefreshFeedExecutionOutcome,
};

/// 界面运行时的能力入口。各端口只是按页面分组的视图，实际实现都在 [`HostServices`]。
pub(crate) struct UiServices {
    services: HostServices,
}

impl UiServices {
    pub(crate) async fn shared() -> Result<Self> {
        let host = AppServices::shared().await?;
        Ok(Self { services: HostServices::new(host.use_cases(), host.host_capabilities()) })
    }

    pub(crate) fn entries(&self) -> EntriesPort {
        EntriesPort { services: self.services.clone() }
    }

    pub(crate) fn shell(&self) -> ShellPort {
        ShellPort { services: self.services.clone() }
    }

    pub(crate) fn settings(&self) -> SettingsPort {
        SettingsPort { services: self.services.clone() }
    }

    pub(crate) fn reader(&self) -> ReaderPort {
        ReaderPort { services: self.services.clone() }
    }

    pub(crate) fn feeds(&self) -> FeedsPort {
        FeedsPort { services: self.services.clone() }
    }
}

#[derive(Clone)]
pub(crate) struct EntriesPort {
    services: HostServices,
}

impl EntriesPort {
    pub(crate) async fn preview_mark_read(
        &self,
        query: &EntryQuery,
    ) -> Result<rssr_domain::MarkReadPreview> {
        self.services.preview_mark_read(query).await
    }
    pub(crate) async fn mark_read_if_unchanged(
        &self,
        preview: &rssr_domain::MarkReadPreview,
    ) -> Result<rssr_domain::MarkReadOutcome> {
        self.services.mark_read_if_unchanged(preview).await
    }

    pub(crate) async fn bootstrap(
        &self,
        input: EntriesBootstrapInput,
    ) -> Result<EntriesBootstrapOutcome> {
        self.services.bootstrap_entries(input).await
    }

    pub(crate) async fn save_workspace_if_changed(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> Result<bool> {
        self.services.save_workspace_if_changed(workspace).await
    }

    pub(crate) async fn list_entries(&self, query: &EntryQuery) -> Result<EntriesListOutcome> {
        self.services.list_entries(query).await
    }

    pub(crate) async fn toggle_read(
        &self,
        input: ToggleEntryReadInput,
    ) -> Result<ToggleEntryReadOutcome> {
        self.services.toggle_entry_read(input).await
    }

    pub(crate) async fn toggle_starred(
        &self,
        input: ToggleEntryStarredInput,
    ) -> Result<ToggleEntryStarredOutcome> {
        self.services.toggle_entry_starred(input).await
    }
}

#[derive(Clone)]
pub(crate) struct ShellPort {
    services: HostServices,
}

impl ShellPort {
    pub(crate) async fn load_authenticated_shell(&self) -> Result<UserSettings> {
        self.services.load_settings().await
    }

    pub(crate) fn ensure_auto_refresh_started(&self) {
        self.services.ensure_auto_refresh_started();
    }

    pub(crate) async fn resolve_startup_target(&self) -> Result<StartupTarget> {
        self.services.resolve_startup_target().await
    }
}

#[derive(Clone)]
pub(crate) struct SettingsPort {
    services: HostServices,
}

impl SettingsPort {
    pub(crate) async fn load_settings(&self) -> Result<UserSettings> {
        self.services.load_settings().await
    }

    pub(crate) async fn save_settings(&self, settings: &UserSettings) -> Result<()> {
        self.services.save_settings(settings).await
    }

    pub(crate) async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPushOutcome> {
        self.services.push_remote_config(endpoint, remote_path).await
    }

    pub(crate) async fn pull_remote_config_and_load_settings(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<AppliedRemoteConfigOutcome> {
        self.services.pull_remote_config_and_load_settings(endpoint, remote_path).await
    }
}

#[derive(Clone)]
pub(crate) struct ReaderPort {
    services: HostServices,
}

impl ReaderPort {
    pub(crate) async fn load_entry(&self, entry_id: i64) -> Result<ReaderEntrySnapshot> {
        self.services.load_entry(entry_id).await
    }

    pub(crate) async fn toggle_read(&self, input: ToggleReadInput) -> Result<ToggleReadOutcome> {
        self.services.toggle_reader_read(input).await
    }

    pub(crate) async fn toggle_starred(
        &self,
        input: ToggleStarredInput,
    ) -> Result<ToggleStarredOutcome> {
        self.services.toggle_reader_starred(input).await
    }

    pub(crate) async fn localize_entry_assets(&self, entry_id: i64) -> Result<bool> {
        self.services.localize_entry_assets(entry_id).await
    }
}

#[derive(Clone)]
pub(crate) struct FeedsPort {
    services: HostServices,
}

impl FeedsPort {
    pub(crate) async fn load_snapshot(&self) -> Result<FeedsSnapshotOutcome> {
        self.services.load_feeds_snapshot().await
    }

    pub(crate) async fn add_subscription(
        &self,
        raw_url: &str,
        fallback_site_url: Option<url::Url>,
    ) -> Result<AddSubscriptionOutcome> {
        self.services.add_subscription(raw_url, fallback_site_url).await
    }

    pub(crate) async fn refresh_all(&self) -> Result<RefreshAllExecutionOutcome> {
        self.services.refresh_all().await
    }

    pub(crate) async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedExecutionOutcome> {
        self.services.refresh_feed(feed_id).await
    }

    pub(crate) async fn remove_feed(&self, feed_id: i64) -> Result<()> {
        self.services.remove_feed(feed_id).await
    }

    pub(crate) async fn export_config_json(&self) -> Result<String> {
        self.services.export_config_json().await
    }

    pub(crate) async fn import_config_json(&self, raw: &str) -> Result<ConfigImportOutcome> {
        self.services.import_config_json(raw).await
    }

    pub(crate) async fn export_opml(&self) -> Result<String> {
        self.services.export_opml().await
    }

    pub(crate) async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome> {
        self.services.import_opml(raw).await
    }
}
