use std::sync::Arc;

use anyhow::Result;
use rssr_domain::{
    EntriesWorkspaceState, Entry, EntryNavigation, EntryQuery, EntrySummary, FeedSummary,
    UserSettings,
};

use crate::bootstrap::AppServices;

pub(crate) struct UiServices {
    inner: Arc<AppServices>,
}

impl UiServices {
    pub(crate) async fn shared() -> Result<Self> {
        Ok(Self { inner: AppServices::shared().await? })
    }

    pub(crate) fn entries(&self) -> EntriesPort {
        EntriesPort { inner: self.inner.clone() }
    }

    pub(crate) fn shell(&self) -> ShellPort {
        ShellPort { inner: self.inner.clone() }
    }

    pub(crate) fn settings(&self) -> SettingsPort {
        SettingsPort { inner: self.inner.clone() }
    }

    pub(crate) fn reader(&self) -> ReaderPort {
        ReaderPort { inner: self.inner.clone() }
    }

    pub(crate) fn feeds(&self) -> FeedsPort {
        FeedsPort { inner: self.inner.clone() }
    }
}

#[derive(Clone)]
pub(crate) struct EntriesPort {
    inner: Arc<AppServices>,
}

impl EntriesPort {
    pub(crate) async fn remember_last_opened_feed_id(&self, feed_id: i64) -> Result<()> {
        self.inner.remember_last_opened_feed_id(feed_id).await
    }

    pub(crate) async fn load_settings(&self) -> Result<UserSettings> {
        self.inner.load_settings().await
    }

    pub(crate) async fn load_workspace_state(&self) -> Result<EntriesWorkspaceState> {
        self.inner.load_entries_workspace_state().await
    }

    pub(crate) async fn save_workspace_state(
        &self,
        workspace: EntriesWorkspaceState,
    ) -> Result<()> {
        self.inner.save_entries_workspace_state(workspace).await
    }

    pub(crate) async fn list_feeds(&self) -> Result<Vec<FeedSummary>> {
        self.inner.list_feeds().await
    }

    pub(crate) async fn list_entries(&self, query: &EntryQuery) -> Result<Vec<EntrySummary>> {
        self.inner.list_entries(query).await
    }

    pub(crate) async fn set_read(&self, entry_id: i64, is_read: bool) -> Result<()> {
        self.inner.set_read(entry_id, is_read).await
    }

    pub(crate) async fn set_starred(&self, entry_id: i64, is_starred: bool) -> Result<()> {
        self.inner.set_starred(entry_id, is_starred).await
    }
}

#[derive(Clone)]
pub(crate) struct ShellPort {
    inner: Arc<AppServices>,
}

impl ShellPort {
    pub(crate) async fn load_settings(&self) -> Result<UserSettings> {
        self.inner.load_settings().await
    }

    pub(crate) fn ensure_auto_refresh_started(&self) {
        self.inner.ensure_auto_refresh_started();
    }

    pub(crate) async fn load_last_opened_feed_id(&self) -> Result<Option<i64>> {
        self.inner.load_last_opened_feed_id().await
    }

    pub(crate) async fn list_feeds(&self) -> Result<Vec<FeedSummary>> {
        self.inner.list_feeds().await
    }
}

#[derive(Clone)]
pub(crate) struct SettingsPort {
    inner: Arc<AppServices>,
}

impl SettingsPort {
    pub(crate) async fn load_settings(&self) -> Result<UserSettings> {
        self.inner.load_settings().await
    }

    pub(crate) async fn save_settings(&self, settings: &UserSettings) -> Result<()> {
        self.inner.save_settings(settings).await
    }

    pub(crate) async fn push_remote_config(&self, endpoint: &str, remote_path: &str) -> Result<()> {
        self.inner.push_remote_config(endpoint, remote_path).await
    }

    pub(crate) async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> Result<bool> {
        self.inner.pull_remote_config(endpoint, remote_path).await
    }
}

#[derive(Clone)]
pub(crate) struct ReaderPort {
    inner: Arc<AppServices>,
}

impl ReaderPort {
    pub(crate) async fn get_entry(&self, entry_id: i64) -> Result<Option<Entry>> {
        self.inner.get_entry(entry_id).await
    }

    pub(crate) async fn reader_navigation(&self, entry_id: i64) -> Result<EntryNavigation> {
        self.inner.reader_navigation(entry_id).await
    }

    pub(crate) async fn set_read(&self, entry_id: i64, is_read: bool) -> Result<()> {
        self.inner.set_read(entry_id, is_read).await
    }

    pub(crate) async fn set_starred(&self, entry_id: i64, is_starred: bool) -> Result<()> {
        self.inner.set_starred(entry_id, is_starred).await
    }
}

#[derive(Clone)]
pub(crate) struct FeedsPort {
    inner: Arc<AppServices>,
}

impl FeedsPort {
    pub(crate) async fn list_feeds(&self) -> Result<Vec<FeedSummary>> {
        self.inner.list_feeds().await
    }

    pub(crate) async fn list_entries(&self, query: &EntryQuery) -> Result<Vec<EntrySummary>> {
        self.inner.list_entries(query).await
    }

    pub(crate) async fn add_subscription(&self, raw_url: &str) -> Result<()> {
        self.inner.add_subscription(raw_url).await
    }

    pub(crate) async fn refresh_all(&self) -> Result<()> {
        self.inner.refresh_all().await
    }

    pub(crate) async fn refresh_feed(&self, feed_id: i64) -> Result<()> {
        self.inner.refresh_feed(feed_id).await
    }

    pub(crate) async fn remove_feed(&self, feed_id: i64) -> Result<()> {
        self.inner.remove_feed(feed_id).await
    }

    pub(crate) async fn export_config_json(&self) -> Result<String> {
        self.inner.export_config_json().await
    }

    pub(crate) async fn import_config_json(&self, raw: &str) -> Result<()> {
        self.inner.import_config_json(raw).await
    }

    pub(crate) async fn export_opml(&self) -> Result<String> {
        self.inner.export_opml().await
    }

    pub(crate) async fn import_opml(&self, raw: &str) -> Result<()> {
        self.inner.import_opml(raw).await
    }
}
