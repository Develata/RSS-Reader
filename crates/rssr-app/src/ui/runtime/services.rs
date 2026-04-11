use std::sync::Arc;

use anyhow::Result;
use rssr_application::{AppUseCases, RemoveSubscriptionInput};
use rssr_domain::{
    EntriesWorkspaceState, Entry, EntryNavigation, EntryQuery, EntrySummary, FeedSummary,
    UserSettings,
};

use crate::bootstrap::AppServices;

pub(crate) struct UiServices {
    host: Arc<AppServices>,
    use_cases: AppUseCases,
}

impl UiServices {
    pub(crate) async fn shared() -> Result<Self> {
        let host = AppServices::shared().await?;
        let use_cases = host.use_cases();
        Ok(Self { host, use_cases })
    }

    pub(crate) fn entries(&self) -> EntriesPort {
        EntriesPort { use_cases: self.use_cases.clone() }
    }

    pub(crate) fn shell(&self) -> ShellPort {
        ShellPort { host: self.host.clone(), use_cases: self.use_cases.clone() }
    }

    pub(crate) fn settings(&self) -> SettingsPort {
        SettingsPort { host: self.host.clone(), use_cases: self.use_cases.clone() }
    }

    pub(crate) fn reader(&self) -> ReaderPort {
        ReaderPort { use_cases: self.use_cases.clone() }
    }

    pub(crate) fn feeds(&self) -> FeedsPort {
        FeedsPort { host: self.host.clone(), use_cases: self.use_cases.clone() }
    }
}

#[derive(Clone)]
pub(crate) struct EntriesPort {
    use_cases: AppUseCases,
}

impl EntriesPort {
    pub(crate) async fn remember_last_opened_feed_id(&self, feed_id: i64) -> Result<()> {
        self.use_cases.app_state_service.save_last_opened_feed_id(Some(feed_id)).await
    }

    pub(crate) async fn load_settings(&self) -> Result<UserSettings> {
        self.use_cases.settings_service.load().await
    }

    pub(crate) async fn load_workspace_state(&self) -> Result<EntriesWorkspaceState> {
        self.use_cases.app_state_service.load_entries_workspace().await
    }

    pub(crate) async fn save_workspace_state(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> Result<()> {
        self.use_cases.app_state_service.save_entries_workspace(workspace).await
    }

    pub(crate) async fn list_feeds(&self) -> Result<Vec<FeedSummary>> {
        self.use_cases.feed_service.list_feeds().await
    }

    pub(crate) async fn list_entries(&self, query: &EntryQuery) -> Result<Vec<EntrySummary>> {
        self.use_cases.entry_service.list_entries(query).await
    }

    pub(crate) async fn set_read(&self, entry_id: i64, is_read: bool) -> Result<()> {
        self.use_cases.entry_service.set_read(entry_id, is_read).await
    }

    pub(crate) async fn set_starred(&self, entry_id: i64, is_starred: bool) -> Result<()> {
        self.use_cases.entry_service.set_starred(entry_id, is_starred).await
    }
}

#[derive(Clone)]
pub(crate) struct ShellPort {
    host: Arc<AppServices>,
    use_cases: AppUseCases,
}

impl ShellPort {
    pub(crate) async fn load_settings(&self) -> Result<UserSettings> {
        self.use_cases.settings_service.load().await
    }

    pub(crate) fn ensure_auto_refresh_started(&self) {
        self.host.auto_refresh().ensure_started();
    }

    pub(crate) async fn load_last_opened_feed_id(&self) -> Result<Option<i64>> {
        self.use_cases.app_state_service.load_last_opened_feed_id().await
    }

    pub(crate) async fn list_feeds(&self) -> Result<Vec<FeedSummary>> {
        self.use_cases.feed_service.list_feeds().await
    }
}

#[derive(Clone)]
pub(crate) struct SettingsPort {
    host: Arc<AppServices>,
    use_cases: AppUseCases,
}

impl SettingsPort {
    pub(crate) async fn load_settings(&self) -> Result<UserSettings> {
        self.use_cases.settings_service.load().await
    }

    pub(crate) async fn save_settings(&self, settings: &UserSettings) -> Result<()> {
        self.use_cases.settings_service.save(settings).await
    }

    pub(crate) async fn push_remote_config(&self, endpoint: &str, remote_path: &str) -> Result<()> {
        self.host.remote_config().push(endpoint, remote_path).await
    }

    pub(crate) async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<bool> {
        self.host.remote_config().pull(endpoint, remote_path).await
    }
}

#[derive(Clone)]
pub(crate) struct ReaderPort {
    use_cases: AppUseCases,
}

impl ReaderPort {
    pub(crate) async fn get_entry(&self, entry_id: i64) -> Result<Option<Entry>> {
        self.use_cases.entry_service.get_entry(entry_id).await
    }

    pub(crate) async fn reader_navigation(&self, entry_id: i64) -> Result<EntryNavigation> {
        self.use_cases.entry_service.reader_navigation(entry_id).await
    }

    pub(crate) async fn set_read(&self, entry_id: i64, is_read: bool) -> Result<()> {
        self.use_cases.entry_service.set_read(entry_id, is_read).await
    }

    pub(crate) async fn set_starred(&self, entry_id: i64, is_starred: bool) -> Result<()> {
        self.use_cases.entry_service.set_starred(entry_id, is_starred).await
    }
}

#[derive(Clone)]
pub(crate) struct FeedsPort {
    host: Arc<AppServices>,
    use_cases: AppUseCases,
}

impl FeedsPort {
    pub(crate) async fn list_feeds(&self) -> Result<Vec<FeedSummary>> {
        self.use_cases.feed_service.list_feeds().await
    }

    pub(crate) async fn list_entries(&self, query: &EntryQuery) -> Result<Vec<EntrySummary>> {
        self.use_cases.entry_service.list_entries(query).await
    }

    pub(crate) async fn add_subscription(&self, raw_url: &str) -> Result<()> {
        self.host.refresh().add_subscription(raw_url).await
    }

    pub(crate) async fn refresh_all(&self) -> Result<()> {
        self.host.refresh().refresh_all().await
    }

    pub(crate) async fn refresh_feed(&self, feed_id: i64) -> Result<()> {
        self.host.refresh().refresh_feed(feed_id).await
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

    pub(crate) async fn import_config_json(&self, raw: &str) -> Result<()> {
        self.use_cases.import_export_service.import_config_json(raw).await
    }

    pub(crate) async fn export_opml(&self) -> Result<String> {
        self.use_cases.import_export_service.export_opml().await
    }

    pub(crate) async fn import_opml(&self, raw: &str) -> Result<()> {
        self.use_cases.import_export_service.import_opml(raw).await
    }
}
