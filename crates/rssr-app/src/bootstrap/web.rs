use std::sync::{Arc, Mutex, atomic::AtomicBool};

#[path = "web/clipboard.rs"]
mod clipboard;
#[path = "web/exchange.rs"]
mod exchange;
#[path = "web/refresh.rs"]
mod refresh;

use anyhow::Context;
use rssr_application::{
    AddSubscriptionInput, AddSubscriptionLifecycleInput, AppUseCases, ClockPort, RefreshAllInput,
    RefreshAllOutcome, RefreshFeedOutcome, RefreshFeedResult, RemoteConfigPullOutcome,
    RemoteConfigPushOutcome,
};
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use rssr_domain::UserSettings;
use rssr_infra::application_adapters::browser::{
    adapters::BrowserRemoteConfigStore, now_utc, state::load_state,
};
use rssr_infra::composition::compose_browser_use_cases;
use time::OffsetDateTime;
use tokio::sync::OnceCell;

use self::{
    clipboard::read_browser_clipboard_text,
    exchange::{
        pull_remote_config as pull_exchange_remote, push_remote_config as push_exchange_remote,
    },
    refresh::ensure_auto_refresh_started as start_auto_refresh,
};
use super::{
    AddSubscriptionOutcome, AutoRefreshPort, ClipboardPort, HostCapabilities,
    ReaderAssetLocalizationOutcome, ReaderAssetPort, RefreshAllExecutionOutcome,
    RefreshFeedExecutionOutcome, RefreshPort, RemoteConfigPort,
};

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

pub struct AppServices {
    client: reqwest::Client,
    use_cases: AppUseCases,
    auto_refresh_started: AtomicBool,
}

#[derive(Clone)]
struct AutoRefreshCapability {
    host: Arc<AppServices>,
}

#[derive(Clone)]
struct RefreshCapability {
    host: Arc<AppServices>,
}

#[derive(Clone)]
struct ReaderAssetCapability;

#[derive(Clone)]
struct RemoteConfigCapability {
    host: Arc<AppServices>,
}

#[derive(Clone)]
struct ClipboardCapability;

#[derive(Clone)]
struct BrowserClock;

impl ClockPort for BrowserClock {
    fn now_utc(&self) -> OffsetDateTime {
        now_utc()
    }
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
                let use_cases =
                    compose_browser_use_cases(state, client.clone(), Arc::new(BrowserClock));
                Ok(Arc::new(Self {
                    client,
                    use_cases,
                    auto_refresh_started: AtomicBool::new(false),
                }))
            })
            .await
            .map(Arc::clone)
    }

    pub fn default_settings() -> UserSettings {
        UserSettings::default()
    }

    pub(crate) fn use_cases(&self) -> AppUseCases {
        self.use_cases.clone()
    }

    pub(crate) fn host_capabilities(self: &Arc<Self>) -> HostCapabilities {
        HostCapabilities {
            auto_refresh: Arc::new(AutoRefreshCapability { host: Arc::clone(self) }),
            refresh: Arc::new(RefreshCapability { host: Arc::clone(self) }),
            reader_assets: Arc::new(ReaderAssetCapability),
            remote_config: Arc::new(RemoteConfigCapability { host: Arc::clone(self) }),
            clipboard: Arc::new(ClipboardCapability),
        }
    }
}

impl AutoRefreshPort for AutoRefreshCapability {
    fn ensure_started(&self) {
        start_auto_refresh(&self.host);
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RefreshPort for RefreshCapability {
    async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<AddSubscriptionOutcome> {
        let outcome = self
            .host
            .use_cases
            .subscription_workflow
            .add_subscription_lifecycle(AddSubscriptionLifecycleInput {
                subscription: AddSubscriptionInput {
                    url: raw_url.to_string(),
                    title: None,
                    folder: None,
                },
                refresh_after_add: true,
            })
            .await
            .context("保存订阅失败")?;
        let refresh = outcome.first_refresh.expect("refresh_after_add produces refresh outcome");
        let outcome = self.handle_refresh_feed_outcome(refresh);
        match outcome.failure_message {
            Some(message) => Ok(AddSubscriptionOutcome::SavedRefreshFailed { message }),
            None => Ok(AddSubscriptionOutcome::SavedAndRefreshed),
        }
    }

    async fn refresh_all(&self) -> anyhow::Result<RefreshAllExecutionOutcome> {
        let outcome =
            self.host.use_cases.refresh_service.refresh_all(RefreshAllInput::default()).await?;
        self.handle_refresh_all_outcome(outcome)
    }

    async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<RefreshFeedExecutionOutcome> {
        let outcome = self.host.use_cases.refresh_service.refresh_feed(feed_id).await?;
        Ok(self.handle_refresh_feed_outcome(outcome))
    }
}

impl RefreshCapability {
    fn handle_refresh_all_outcome(
        &self,
        outcome: RefreshAllOutcome,
    ) -> anyhow::Result<RefreshAllExecutionOutcome> {
        let failure_lines = outcome.joined_failure_lines();

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
                }
            }
        }

        Ok(RefreshAllExecutionOutcome { failure_message: failure_lines })
    }

    fn handle_refresh_feed_outcome(
        &self,
        outcome: RefreshFeedOutcome,
    ) -> RefreshFeedExecutionOutcome {
        let failure_message = outcome.failure_line();
        match outcome.result {
            RefreshFeedResult::Updated { .. } | RefreshFeedResult::NotModified => {
                RefreshFeedExecutionOutcome { failure_message: None }
            }
            RefreshFeedResult::Failed { message } => {
                tracing::warn!(feed_id = outcome.feed_id, error = %message, "刷新订阅失败");
                RefreshFeedExecutionOutcome {
                    failure_message: Some(
                        failure_message.unwrap_or_else(|| "刷新订阅失败".to_string()),
                    ),
                }
            }
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl ReaderAssetPort for ReaderAssetCapability {
    async fn localize_entry_assets(
        &self,
        _entry_id: i64,
    ) -> anyhow::Result<ReaderAssetLocalizationOutcome> {
        Ok(ReaderAssetLocalizationOutcome::default())
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigPort for RemoteConfigCapability {
    async fn push(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<RemoteConfigPushOutcome> {
        push_exchange_remote(
            &self.host.use_cases.import_export_service,
            &BrowserRemoteConfigStore::new(self.host.client.clone(), endpoint, remote_path),
        )
        .await
    }

    async fn pull(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<RemoteConfigPullOutcome> {
        pull_exchange_remote(
            &self.host.use_cases.import_export_service,
            &BrowserRemoteConfigStore::new(self.host.client.clone(), endpoint, remote_path),
        )
        .await
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl ClipboardPort for ClipboardCapability {
    async fn read_text(&self) -> anyhow::Result<Option<String>> {
        read_browser_clipboard_text().await
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
