use anyhow::Result;
use rssr_application::{
    AppUseCases, AppliedRemoteConfigOutcome, ConfigImportOutcome, EntriesBootstrapInput,
    EntriesBootstrapOutcome, EntriesListOutcome, FeedsSnapshotOutcome, OpmlImportOutcome,
    ReaderEntrySnapshot, RemoteConfigPushOutcome, RemoveSubscriptionInput, StartupTarget,
    ToggleEntryReadInput, ToggleEntryReadOutcome, ToggleEntryStarredInput,
    ToggleEntryStarredOutcome, ToggleReadInput, ToggleReadOutcome, ToggleStarredInput,
    ToggleStarredOutcome,
};
use rssr_domain::{EntriesWorkspaceState, EntryQuery, UserSettings};

use crate::bootstrap::{
    AddSubscriptionOutcome, AppServices, HostCapabilities, RefreshAllExecutionOutcome,
    RefreshFeedExecutionOutcome,
};

pub(crate) struct UiServices {
    use_cases: AppUseCases,
    host_capabilities: HostCapabilities,
}

impl UiServices {
    pub(crate) async fn shared() -> Result<Self> {
        let host = AppServices::shared().await?;
        let use_cases = host.use_cases();
        let host_capabilities = host.host_capabilities();
        Ok(Self { use_cases, host_capabilities })
    }

    pub(crate) fn entries(&self) -> EntriesPort {
        EntriesPort { use_cases: self.use_cases.clone() }
    }

    pub(crate) fn shell(&self) -> ShellPort {
        ShellPort {
            use_cases: self.use_cases.clone(),
            host_capabilities: self.host_capabilities.clone(),
        }
    }

    pub(crate) fn settings(&self) -> SettingsPort {
        SettingsPort {
            use_cases: self.use_cases.clone(),
            host_capabilities: self.host_capabilities.clone(),
        }
    }

    pub(crate) fn reader(&self) -> ReaderPort {
        ReaderPort { use_cases: self.use_cases.clone() }
    }

    pub(crate) fn feeds(&self) -> FeedsPort {
        FeedsPort {
            use_cases: self.use_cases.clone(),
            host_capabilities: self.host_capabilities.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct EntriesPort {
    use_cases: AppUseCases,
}

impl EntriesPort {
    pub(crate) async fn bootstrap(
        &self,
        input: EntriesBootstrapInput,
    ) -> Result<EntriesBootstrapOutcome> {
        self.use_cases.entries_workspace_service.bootstrap(input).await
    }

    pub(crate) async fn save_workspace_if_changed(
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

    pub(crate) async fn list_entries(&self, query: &EntryQuery) -> Result<EntriesListOutcome> {
        self.use_cases.entries_list_service.list_entries(query).await
    }

    pub(crate) async fn toggle_read(
        &self,
        input: ToggleEntryReadInput,
    ) -> Result<ToggleEntryReadOutcome> {
        self.use_cases.entries_list_service.toggle_read(input).await
    }

    pub(crate) async fn toggle_starred(
        &self,
        input: ToggleEntryStarredInput,
    ) -> Result<ToggleEntryStarredOutcome> {
        self.use_cases.entries_list_service.toggle_starred(input).await
    }
}

#[derive(Clone)]
pub(crate) struct ShellPort {
    use_cases: AppUseCases,
    host_capabilities: HostCapabilities,
}

impl ShellPort {
    pub(crate) async fn load_authenticated_shell(&self) -> Result<UserSettings> {
        Ok(self.use_cases.shell_service.load_authenticated_shell().await?.settings)
    }

    pub(crate) fn ensure_auto_refresh_started(&self) {
        self.host_capabilities.auto_refresh.ensure_started();
    }

    pub(crate) async fn resolve_startup_target(&self) -> Result<StartupTarget> {
        self.use_cases.startup_service.resolve_startup_target().await
    }
}

#[derive(Clone)]
pub(crate) struct SettingsPort {
    use_cases: AppUseCases,
    host_capabilities: HostCapabilities,
}

impl SettingsPort {
    pub(crate) async fn load_settings(&self) -> Result<UserSettings> {
        Ok(self.use_cases.settings_page_service.load().await?.settings)
    }

    pub(crate) async fn save_settings(&self, settings: &UserSettings) -> Result<()> {
        self.use_cases.settings_page_service.save_appearance(settings).await?;
        Ok(())
    }

    pub(crate) async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<RemoteConfigPushOutcome> {
        self.host_capabilities.remote_config.push(endpoint, remote_path).await
    }

    pub(crate) async fn pull_remote_config_and_load_settings(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<AppliedRemoteConfigOutcome> {
        let outcome = self.host_capabilities.remote_config.pull(endpoint, remote_path).await?;
        self.use_cases.settings_page_service.apply_remote_pull(outcome).await
    }
}

#[derive(Clone)]
pub(crate) struct ReaderPort {
    use_cases: AppUseCases,
}

impl ReaderPort {
    pub(crate) async fn load_entry(&self, entry_id: i64) -> Result<ReaderEntrySnapshot> {
        self.use_cases.reader_service.load_entry(entry_id).await
    }

    pub(crate) async fn toggle_read(&self, input: ToggleReadInput) -> Result<ToggleReadOutcome> {
        self.use_cases.reader_service.toggle_read(input).await
    }

    pub(crate) async fn toggle_starred(
        &self,
        input: ToggleStarredInput,
    ) -> Result<ToggleStarredOutcome> {
        self.use_cases.reader_service.toggle_starred(input).await
    }
}

#[derive(Clone)]
pub(crate) struct FeedsPort {
    use_cases: AppUseCases,
    host_capabilities: HostCapabilities,
}

impl FeedsPort {
    pub(crate) async fn load_snapshot(&self) -> Result<FeedsSnapshotOutcome> {
        self.use_cases.feeds_snapshot_service.load_snapshot().await
    }

    pub(crate) async fn add_subscription(&self, raw_url: &str) -> Result<AddSubscriptionOutcome> {
        self.host_capabilities.refresh.add_subscription(raw_url).await
    }

    pub(crate) async fn refresh_all(&self) -> Result<RefreshAllExecutionOutcome> {
        self.host_capabilities.refresh.refresh_all().await
    }

    pub(crate) async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedExecutionOutcome> {
        self.host_capabilities.refresh.refresh_feed(feed_id).await
    }

    pub(crate) async fn remove_feed(&self, feed_id: i64) -> Result<()> {
        self.use_cases
            .subscription_workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id, purge_entries: true })
            .await
    }

    pub(crate) async fn export_config_json(&self) -> Result<String> {
        self.use_cases.import_export_service.export_config_json().await
    }

    pub(crate) async fn import_config_json(&self, raw: &str) -> Result<ConfigImportOutcome> {
        self.use_cases.import_export_service.import_config_json(raw).await
    }

    pub(crate) async fn export_opml(&self) -> Result<String> {
        self.use_cases.import_export_service.export_opml().await
    }

    pub(crate) async fn import_opml(&self, raw: &str) -> Result<OpmlImportOutcome> {
        self.use_cases.import_export_service.import_opml(raw).await
    }
}
