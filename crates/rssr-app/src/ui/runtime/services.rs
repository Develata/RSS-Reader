use anyhow::Result;
use rssr_application::{
    AppliedRemoteConfigOutcome, ConfigImportOutcome, EntriesBootstrapInput,
    EntriesBootstrapOutcome, EntriesListOutcome, FeedsSnapshotOutcome, OpmlImportOutcome,
    ReaderEntrySnapshot, RemoteConfigPushOutcome, StartupTarget, ToggleEntryReadInput,
    ToggleEntryReadOutcome, ToggleEntryStarredInput, ToggleEntryStarredOutcome, ToggleReadInput,
    ToggleReadOutcome, ToggleStarredInput, ToggleStarredOutcome,
};
use rssr_domain::{EntriesWorkspaceState, EntryQuery, UserSettings};
use rssr_host::{HostServices, client::Backend};

use crate::bootstrap::{
    AddSubscriptionOutcome, AppServices, RefreshAllExecutionOutcome, RefreshFeedExecutionOutcome,
};

/// 界面运行时的能力入口。各端口只是按页面分组的视图，实际实现是进程内的 [`HostServices`]，
/// 或服务端模式下经 `/api/v1` 的远程调用（见 [`Backend`]）。
pub(crate) struct UiServices {
    services: Backend,
}

impl UiServices {
    pub(crate) async fn shared() -> Result<Self> {
        #[cfg(target_arch = "wasm32")]
        if let Some(client) = server_mode::client().await? {
            return Ok(Self {
                services: Backend::Remote(rssr_host::client::RemoteServices::new(client)),
            });
        }
        let host = AppServices::shared().await?;
        let local = HostServices::new(host.use_cases(), host.host_capabilities());
        Ok(Self { services: Backend::Local(Box::new(local)) })
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
    services: Backend,
}

impl EntriesPort {
    pub(crate) async fn apply_entry_batch(
        &self,
        request: rssr_domain::EntryBatchRequest,
    ) -> Result<u64> {
        self.services.apply_entry_batch(request).await
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
    services: Backend,
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
    services: Backend,
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
    services: Backend,
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
    services: Backend,
}

impl FeedsPort {
    pub(crate) async fn edit_details(&self, feed_id: i64) -> Result<rssr_domain::FeedEditDetails> {
        self.services.feed_edit_details(feed_id).await
    }
    pub(crate) async fn update_subscription(
        &self,
        edit: rssr_domain::FeedEditRequest,
    ) -> Result<()> {
        self.services.update_subscription(edit).await
    }
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

/// 服务端模式的判定，对整个页面固定。
///
/// - 演示包、或页面不是由 `rssr-web` 托管（没有门禁 cookie）：本地模式。
/// - 由 `rssr-web` 托管：必须确认 `/api/v1/capabilities` 是服务端模式。确认失败（网络、超时、5xx、
///   HTML、格式不对）是可重试的错误，不缓存、更不退回本地存储，否则会把数据写进与服务端无关的
///   localStorage。rssr-web 总是携带同版本的前端包发布，不存在"服务端没有接口"的合法组合。
///
/// 判定一旦成功就固定：之后门禁 cookie 到期或在别的标签页登出，都只会让接口返回 401 并跳回
/// 登录页，不会让这个页面改用另一套存储。
#[cfg(target_arch = "wasm32")]
mod server_mode {
    use anyhow::{Context, Result};
    use rssr_host::client::ApiClient;
    use tokio::sync::OnceCell;

    static SERVER_MODE: OnceCell<bool> = OnceCell::const_new();

    fn api_client() -> Result<ApiClient> {
        let origin = crate::web_auth::browser_origin().context("无法确定页面地址")?;
        Ok(ApiClient::new(reqwest::Client::new(), origin)
            .on_unauthorized(crate::web_auth::recover_from_expired_server_session))
    }

    pub(super) async fn client() -> Result<Option<ApiClient>> {
        let server_mode = *SERVER_MODE
            .get_or_try_init(|| async {
                if crate::demo::ENABLED || !crate::web_auth::server_gate_present() {
                    return Ok::<_, anyhow::Error>(false);
                }
                api_client()?.capabilities().await.context("确认服务端模式失败，请稍后重试")?;
                Ok(true)
            })
            .await?;
        if server_mode { api_client().map(Some) } else { Ok(None) }
    }
}
