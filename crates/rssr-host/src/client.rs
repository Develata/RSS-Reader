//! 服务端模式的远程调用：按 [`crate::protocol`] 的端点把 [`HostServices`] 的同一组方法发到 `rssr-web`。
//!
//! 两个目标都编译：浏览器端用它接入服务端，原生端用它做客户端与服务端之间的契约测试。
use anyhow::{Context, Result, anyhow};
use reqwest::{StatusCode, header};
use rssr_application::{
    AppliedRemoteConfigOutcome, ConfigImportOutcome, EntriesBootstrapInput,
    EntriesBootstrapOutcome, EntriesListOutcome, FeedsSnapshotOutcome, OpmlImportOutcome,
    ReaderEntrySnapshot, RemoteConfigPushOutcome, StartupTarget, ToggleEntryReadInput,
    ToggleEntryReadOutcome, ToggleEntryStarredInput, ToggleEntryStarredOutcome, ToggleReadInput,
    ToggleReadOutcome, ToggleStarredInput, ToggleStarredOutcome,
};
use rssr_domain::{
    EntriesWorkspaceState, EntryQuery, MarkReadOutcome, MarkReadPreview, UserSettings,
};

use crate::{
    AddSubscriptionOutcome, HostServices, RefreshAllExecutionOutcome, RefreshFeedExecutionOutcome,
    protocol::{
        self, API_PREFIX, AddSubscriptionRequest, ApiError, CAPABILITIES_PATH, Capabilities,
        Endpoint, PROTOCOL_HEADER, PROTOCOL_VERSION, RemoteConfigRequest,
    },
};

#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    /// 服务地址（scheme + 主机 + 端口），不带结尾斜杠。
    base: String,
    /// 收到 401 时调用，例如整页跳回登录页。用函数指针而不是闭包：wasm 上的浏览器对象不是 `Send`。
    on_unauthorized: Option<fn()>,
}

impl ApiClient {
    pub fn new(http: reqwest::Client, base: impl Into<String>) -> Self {
        Self { http, base: base.into().trim_end_matches('/').to_string(), on_unauthorized: None }
    }

    pub fn on_unauthorized(mut self, handler: fn()) -> Self {
        self.on_unauthorized = Some(handler);
        self
    }

    /// 探测服务端模式。传输失败返回错误；服务端可达但不是服务端模式（404、SPA 回退的 HTML、
    /// 无法解析的 JSON、`mode` 不对）返回 `Ok(None)`。只看状态码会把静态托管的 SPA 回退误判为服务端。
    pub async fn capabilities(&self) -> Result<Option<Capabilities>> {
        let response = self
            .http
            .get(format!("{}{CAPABILITIES_PATH}", self.base))
            .send()
            .await
            .context("连接服务端失败")?;
        let is_json = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.trim_start().starts_with("application/json"));
        if response.status() != StatusCode::OK || !is_json {
            return Ok(None);
        }
        let bytes = response.bytes().await.context("读取服务端能力信息失败")?;
        Ok(serde_json::from_slice::<Capabilities>(&bytes)
            .ok()
            .filter(|capabilities| capabilities.mode == Capabilities::SERVER_MODE))
    }

    pub async fn call<E: Endpoint>(&self, input: &E::Input) -> Result<E::Output> {
        let body = serde_json::to_vec(input).context("序列化请求失败")?;
        let response = self
            .http
            .post(format!("{}{API_PREFIX}/{}", self.base, E::PATH))
            .header(header::CONTENT_TYPE, "application/json")
            .header(PROTOCOL_HEADER, PROTOCOL_VERSION.to_string())
            .body(body)
            .send()
            .await
            .context("连接服务端失败")?;
        let status = response.status();
        let bytes = response.bytes().await.context("读取服务端响应失败")?;
        if status.is_success() {
            return serde_json::from_slice(&bytes).context("服务端响应格式不正确");
        }
        if status == StatusCode::UNAUTHORIZED
            && let Some(handler) = self.on_unauthorized
        {
            handler();
        }
        Err(match serde_json::from_slice::<ApiError>(&bytes) {
            Ok(error) => error.into_anyhow(),
            Err(_) => anyhow!("服务端返回了意外的状态：{status}"),
        })
    }
}

/// [`HostServices`] 的远程版本，方法与之一一对应。
#[derive(Clone)]
pub struct RemoteServices {
    client: ApiClient,
}

/// 界面运行时的能力入口：进程内的 [`HostServices`]，或经 `/api/v1` 的 [`RemoteServices`]。
#[derive(Clone)]
pub enum Backend {
    /// 装箱：进程内的用例集合比远程客户端大得多。
    Local(Box<HostServices>),
    Remote(RemoteServices),
}

impl RemoteServices {
    pub fn new(client: ApiClient) -> Self {
        Self { client }
    }
}

macro_rules! backend_methods {
    ($(
        $(#[$meta:meta])*
        fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $output:ty
            => local($($local_arg:expr),*), remote($client:ident) $remote:expr;
    )*) => {
        impl Backend {
            $(
                $(#[$meta])*
                pub async fn $name(&self $(, $arg: $ty)*) -> Result<$output> {
                    match self {
                        Self::Local(services) => services.$name($($local_arg),*).await,
                        Self::Remote(remote) => {
                            let $client = &remote.client;
                            $remote.await
                        }
                    }
                }
            )*
        }
    };
}

backend_methods! {
    fn preview_mark_read(&self, query: &EntryQuery) -> MarkReadPreview
        => local(query), remote(c) c.call::<protocol::PreviewMarkRead>(query);
    fn mark_read_if_unchanged(&self, preview: &MarkReadPreview) -> MarkReadOutcome
        => local(preview), remote(c) c.call::<protocol::MarkReadIfUnchanged>(preview);
    fn bootstrap_entries(&self, input: EntriesBootstrapInput) -> EntriesBootstrapOutcome
        => local(input), remote(c) c.call::<protocol::BootstrapEntries>(&input);
    fn save_workspace_if_changed(&self, workspace: EntriesWorkspaceState) -> bool
        => local(workspace), remote(c) c.call::<protocol::SaveWorkspaceIfChanged>(&workspace);
    fn list_entries(&self, query: &EntryQuery) -> EntriesListOutcome
        => local(query), remote(c) c.call::<protocol::ListEntries>(query);
    fn toggle_entry_read(&self, input: ToggleEntryReadInput) -> ToggleEntryReadOutcome
        => local(input), remote(c) c.call::<protocol::ToggleEntryRead>(&input);
    fn toggle_entry_starred(&self, input: ToggleEntryStarredInput) -> ToggleEntryStarredOutcome
        => local(input), remote(c) c.call::<protocol::ToggleEntryStarred>(&input);
    fn resolve_startup_target(&self) -> StartupTarget
        => local(), remote(c) c.call::<protocol::ResolveStartupTarget>(&());
    fn load_settings(&self) -> UserSettings
        => local(), remote(c) c.call::<protocol::LoadSettings>(&());
    fn save_settings(&self, settings: &UserSettings) -> ()
        => local(settings), remote(c) c.call::<protocol::SaveSettings>(settings);
    fn push_remote_config(&self, endpoint: &str, remote_path: &str) -> RemoteConfigPushOutcome
        => local(endpoint, remote_path), remote(c) c.call::<protocol::PushRemoteConfig>(
            &RemoteConfigRequest { endpoint: endpoint.to_string(), remote_path: remote_path.to_string() });
    fn pull_remote_config_and_load_settings(&self, endpoint: &str, remote_path: &str) -> AppliedRemoteConfigOutcome
        => local(endpoint, remote_path), remote(c) c.call::<protocol::PullRemoteConfig>(
            &RemoteConfigRequest { endpoint: endpoint.to_string(), remote_path: remote_path.to_string() });
    fn load_entry(&self, entry_id: i64) -> ReaderEntrySnapshot
        => local(entry_id), remote(c) c.call::<protocol::LoadEntry>(&entry_id);
    fn toggle_reader_read(&self, input: ToggleReadInput) -> ToggleReadOutcome
        => local(input), remote(c) c.call::<protocol::ToggleReaderRead>(&input);
    fn toggle_reader_starred(&self, input: ToggleStarredInput) -> ToggleStarredOutcome
        => local(input), remote(c) c.call::<protocol::ToggleReaderStarred>(&input);
    fn localize_entry_assets(&self, entry_id: i64) -> bool
        => local(entry_id), remote(c) c.call::<protocol::LocalizeEntryAssets>(&entry_id);
    fn load_feeds_snapshot(&self) -> FeedsSnapshotOutcome
        => local(), remote(c) c.call::<protocol::LoadFeedsSnapshot>(&());
    fn add_subscription(&self, raw_url: &str, fallback_site_url: Option<url::Url>) -> AddSubscriptionOutcome
        => local(raw_url, fallback_site_url), remote(c) c.call::<protocol::AddSubscription>(
            &AddSubscriptionRequest { raw_url: raw_url.to_string(), fallback_site_url });
    fn refresh_all(&self) -> RefreshAllExecutionOutcome
        => local(), remote(c) c.call::<protocol::RefreshAll>(&());
    fn refresh_feed(&self, feed_id: i64) -> RefreshFeedExecutionOutcome
        => local(feed_id), remote(c) c.call::<protocol::RefreshFeed>(&feed_id);
    fn remove_feed(&self, feed_id: i64) -> ()
        => local(feed_id), remote(c) c.call::<protocol::RemoveFeed>(&feed_id);
    fn export_config_json(&self) -> String
        => local(), remote(c) c.call::<protocol::ExportConfigJson>(&());
    fn import_config_json(&self, raw: &str) -> ConfigImportOutcome
        => local(raw), remote(c) c.call::<protocol::ImportConfigJson>(&raw.to_string());
    fn export_opml(&self) -> String
        => local(), remote(c) c.call::<protocol::ExportOpml>(&());
    fn import_opml(&self, raw: &str) -> OpmlImportOutcome
        => local(raw), remote(c) c.call::<protocol::ImportOpml>(&raw.to_string());
}

impl Backend {
    /// 服务端模式下自动刷新由服务端常驻执行，这里不需要启动任何东西。
    pub fn ensure_auto_refresh_started(&self) {
        if let Self::Local(services) = self {
            services.ensure_auto_refresh_started();
        }
    }
}
