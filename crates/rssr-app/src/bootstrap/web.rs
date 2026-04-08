use std::sync::{Arc, Mutex, atomic::AtomicBool};

#[path = "web/exchange.rs"]
mod exchange;
#[path = "web/mutations.rs"]
mod mutations;
#[path = "web/refresh.rs"]
mod refresh;

use anyhow::Context;
use rssr_application::{
    AddSubscriptionInput, FeedService, ImportExportService, RefreshAllInput, RefreshAllOutcome,
    RefreshFeedOutcome, RefreshFeedResult, RefreshService, RemoveSubscriptionInput,
    SubscriptionWorkflow,
};
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use rssr_domain::{Entry, EntryQuery, EntrySummary, FeedSummary, UserSettings};
use rssr_infra::application_adapters::browser::{
    adapters::{
        BrowserAppStateAdapter, BrowserEntryRepository, BrowserFeedRefreshSource,
        BrowserFeedRepository, BrowserOpmlCodec, BrowserRefreshStore, BrowserRemoteConfigStore,
        BrowserSettingsRepository,
    },
    query::{
        get_entry as query_get_entry, list_entries as query_list_entries,
        list_feeds as query_list_feeds, reader_navigation as query_reader_navigation,
    },
    state::{PersistedState, load_state},
};
use tokio::sync::OnceCell;

use self::{
    exchange::{
        export_config_json as export_exchange_json, export_opml as export_exchange_opml,
        import_config_json as import_exchange_json, import_opml as import_exchange_opml,
        pull_remote_config as pull_exchange_remote, push_remote_config as push_exchange_remote,
    },
    mutations::{
        remember_last_opened_feed_id as remember_feed_id, save_settings as save_settings_state,
        set_read as set_entry_read, set_starred as set_entry_starred,
    },
    refresh::ensure_auto_refresh_started as start_auto_refresh,
};

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

pub struct AppServices {
    state: Arc<Mutex<PersistedState>>,
    client: reqwest::Client,
    refresh_service: RefreshService,
    subscription_workflow: SubscriptionWorkflow,
    import_export_service: ImportExportService,
    auto_refresh_started: AtomicBool,
}

impl AppServices {
    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async {
                let loaded = load_state();
                if let Some(warning) = loaded.warning.as_deref() {
                    tracing::warn!(warning = warning, "Web 本地状态恢复时发现异常");
                }
                let state = Arc::new(Mutex::new(loaded.state));
                let client = reqwest::Client::new();
                let feed_repository = Arc::new(BrowserFeedRepository::new(state.clone()));
                let entry_repository = Arc::new(BrowserEntryRepository::new(state.clone()));
                let settings_repository = Arc::new(BrowserSettingsRepository::new(state.clone()));
                let app_state_adapter = Arc::new(BrowserAppStateAdapter::new(state.clone()));
                let feed_service =
                    FeedService::new(feed_repository.clone(), entry_repository.clone());
                let refresh_service = RefreshService::new(
                    Arc::new(BrowserFeedRefreshSource::new(client.clone())),
                    Arc::new(BrowserRefreshStore::new(state.clone())),
                );
                Ok(Arc::new(Self {
                    state: state.clone(),
                    client,
                    refresh_service: refresh_service.clone(),
                    subscription_workflow: SubscriptionWorkflow::new(
                        feed_service,
                        refresh_service,
                        app_state_adapter.clone(),
                    ),
                    import_export_service: ImportExportService::new_with_feed_removal_cleanup(
                        feed_repository,
                        entry_repository,
                        settings_repository,
                        Arc::new(BrowserOpmlCodec),
                        app_state_adapter,
                    ),
                    auto_refresh_started: AtomicBool::new(false),
                }))
            })
            .await
            .map(Arc::clone)
    }

    pub fn default_settings() -> UserSettings {
        UserSettings::default()
    }

    pub async fn list_feeds(&self) -> anyhow::Result<Vec<FeedSummary>> {
        let state = self.state.lock().expect("lock state");
        Ok(query_list_feeds(&state))
    }

    pub async fn list_entries(&self, query: &EntryQuery) -> anyhow::Result<Vec<EntrySummary>> {
        let state = self.state.lock().expect("lock state");
        Ok(query_list_entries(&state, query))
    }

    pub async fn get_entry(&self, entry_id: i64) -> anyhow::Result<Option<Entry>> {
        let state = self.state.lock().expect("lock state");
        query_get_entry(&state, entry_id)
    }

    pub async fn reader_navigation(
        &self,
        current_entry_id: i64,
    ) -> anyhow::Result<ReaderNavigation> {
        let state = self.state.lock().expect("lock state");
        Ok(query_reader_navigation(&state, current_entry_id))
    }

    pub async fn set_read(&self, entry_id: i64, is_read: bool) -> anyhow::Result<()> {
        set_entry_read(self, entry_id, is_read)
    }

    pub async fn set_starred(&self, entry_id: i64, is_starred: bool) -> anyhow::Result<()> {
        set_entry_starred(self, entry_id, is_starred)
    }

    pub async fn load_settings(&self) -> anyhow::Result<UserSettings> {
        Ok(self.state.lock().expect("lock state").settings.clone())
    }

    pub async fn save_settings(&self, settings: &UserSettings) -> anyhow::Result<()> {
        save_settings_state(self, settings)
    }

    pub async fn load_last_opened_feed_id(&self) -> anyhow::Result<Option<i64>> {
        Ok(self.state.lock().expect("lock state").last_opened_feed_id)
    }

    pub async fn remember_last_opened_feed_id(&self, feed_id: i64) -> anyhow::Result<()> {
        remember_feed_id(self, feed_id)
    }

    pub fn ensure_auto_refresh_started(self: &Arc<Self>) {
        start_auto_refresh(self);
    }

    pub async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<()> {
        let outcome = self
            .subscription_workflow
            .add_subscription_and_refresh(&AddSubscriptionInput {
                url: raw_url.to_string(),
                title: None,
                folder: None,
            })
            .await
            .context("保存订阅失败")?;
        self.handle_refresh_outcome(outcome.refresh).context("首次刷新订阅失败")
    }

    pub async fn remove_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        self.subscription_workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id, purge_entries: true })
            .await
            .context("删除订阅失败")
    }

    pub async fn refresh_all(&self) -> anyhow::Result<()> {
        let outcome = self.refresh_service.refresh_all(RefreshAllInput::default()).await?;
        self.handle_refresh_all_outcome(outcome)
    }

    pub async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        let outcome = self.refresh_service.refresh_feed(feed_id).await?;
        self.handle_refresh_outcome(outcome)
    }

    pub async fn export_config_json(&self) -> anyhow::Result<String> {
        export_exchange_json(&self.import_export_service).await
    }

    pub async fn import_config_json(&self, raw: &str) -> anyhow::Result<()> {
        import_exchange_json(&self.import_export_service, raw).await
    }

    pub async fn export_opml(&self) -> anyhow::Result<String> {
        export_exchange_opml(&self.import_export_service).await
    }

    pub async fn import_opml(&self, raw: &str) -> anyhow::Result<()> {
        import_exchange_opml(&self.import_export_service, raw).await
    }

    pub async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<()> {
        push_exchange_remote(
            &self.import_export_service,
            &BrowserRemoteConfigStore::new(self.client.clone(), endpoint, remote_path),
        )
        .await
    }

    pub async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<bool> {
        pull_exchange_remote(
            &self.import_export_service,
            &BrowserRemoteConfigStore::new(self.client.clone(), endpoint, remote_path),
        )
        .await
    }

    fn handle_refresh_all_outcome(&self, outcome: RefreshAllOutcome) -> anyhow::Result<()> {
        let mut errors = Vec::new();

        for feed in outcome.feeds {
            match feed.result {
                RefreshFeedResult::Updated { .. } => {
                    tracing::debug!(feed_id = feed.feed_id, "刷新订阅成功");
                }
                RefreshFeedResult::NotModified => {
                    tracing::debug!(feed_id = feed.feed_id, "订阅未变化");
                }
                RefreshFeedResult::Failed { message } => {
                    tracing::warn!(feed_id = feed.feed_id, error = %message, "刷新订阅失败");
                    errors.push(format!("{}: {message}", feed.url));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!("部分订阅刷新失败: {}", errors.join(" | "))
        }
    }

    fn handle_refresh_outcome(&self, outcome: RefreshFeedOutcome) -> anyhow::Result<()> {
        match outcome.result {
            RefreshFeedResult::Updated { .. } | RefreshFeedResult::NotModified => Ok(()),
            RefreshFeedResult::Failed { message } => anyhow::bail!("{}: {message}", outcome.url),
        }
    }
}
#[cfg(test)]
mod tests {
    use rssr_infra::application_adapters::browser::query::title_matches_search;

    #[test]
    fn web_title_search_is_case_insensitive() {
        assert!(title_matches_search("Roche Scales NVIDIA AI Factories", "sca"));
        assert!(title_matches_search("Roche Scales NVIDIA AI Factories", "SCA"));
        assert!(!title_matches_search("Roche Scales NVIDIA AI Factories", "xyz"));
    }
}
