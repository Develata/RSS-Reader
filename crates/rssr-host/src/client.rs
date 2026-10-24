//! 服务端模式的远程调用：按 [`crate::protocol`] 的端点把 [`HostServices`] 的同一组方法发到 `rssr-web`。
//!
//! 两个目标都编译：浏览器端用它接入服务端，原生端用它做客户端与服务端之间的契约测试。
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail, ensure};
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

    /// 确认服务端模式。只有 200 + `application/json` + 可解析且 `mode` 为服务端的响应才算成功；
    /// 其余一切（传输失败、超时、5xx、HTML、损坏的 JSON、其它模式）都是错误。调用方不得把错误
    /// 当成"本地部署"：页面由 rssr-web 托管时，这些情况只说明服务端暂时不可用或部署不一致。
    pub async fn capabilities(&self) -> Result<Capabilities> {
        let response = self
            .http
            .get(format!("{}{CAPABILITIES_PATH}", self.base))
            .timeout(CAPABILITIES_TIMEOUT)
            .send()
            .await
            .context("连接服务端失败")?;
        let status = response.status();
        let is_json = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.trim_start().starts_with("application/json"));
        if status != StatusCode::OK || !is_json {
            bail!("服务端没有返回服务端模式信息（状态 {status}）");
        }
        let bytes = read_limited(response, MAX_CAPABILITIES_BYTES)
            .await
            .context("读取服务端能力信息失败")?;
        let capabilities: Capabilities =
            serde_json::from_slice(&bytes).context("服务端能力信息格式不正确")?;
        ensure!(
            capabilities.mode == Capabilities::SERVER_MODE,
            "服务端报告的模式不是服务端模式：{}",
            capabilities.mode
        );
        Ok(capabilities)
    }

    /// 调用一个端点。超时只说明客户端不再等待，服务端任务可能仍在执行，所以不自动重试。
    pub async fn call<E: Endpoint>(&self, input: &E::Input) -> Result<E::Output> {
        let body = serde_json::to_vec(input).context("序列化请求失败")?;
        let response = self
            .http
            .post(format!("{}{API_PREFIX}/{}", self.base, E::PATH))
            .header(header::CONTENT_TYPE, "application/json")
            .header(PROTOCOL_HEADER, PROTOCOL_VERSION.to_string())
            .timeout(CALL_TIMEOUT)
            .body(body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    anyhow!("等待服务端响应超时；操作可能仍在服务端执行，请稍后刷新查看结果")
                } else {
                    anyhow::Error::new(error).context("连接服务端失败")
                }
            })?;
        let status = response.status();
        // 401 只看状态行就处理，也不再读正文：恢复登录不能依赖错误正文能读完，页面随即离开。
        if status == StatusCode::UNAUTHORIZED {
            if let Some(handler) = self.on_unauthorized {
                handler();
            }
            bail!("未登录或会话已过期，请重新登录");
        }
        if status.is_success() {
            let bytes =
                read_limited(response, MAX_RESPONSE_BYTES).await.context("读取服务端响应失败")?;
            return serde_json::from_slice(&bytes).context("服务端响应格式不正确");
        }
        let error = read_limited(response, MAX_ERROR_BYTES)
            .await
            .ok()
            .and_then(|bytes| serde_json::from_slice::<ApiError>(&bytes).ok());
        Err(match error {
            Some(error) => error.into_anyhow(),
            None => anyhow!("服务端返回了意外的状态：{status}"),
        })
    }
}

/// 能力信息只有一行 JSON。
const MAX_CAPABILITIES_BYTES: usize = 64 * 1024;
/// 错误正文是一条错误链。
const MAX_ERROR_BYTES: usize = 1024 * 1024;
/// 成功响应的上限。阅读页正文可能内嵌本地化后的图片（单篇最多约 18 MiB），留出余量。
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const CAPABILITIES_TIMEOUT: Duration = Duration::from_secs(15);
/// 刷新全部订阅等操作可能持续数分钟；超过这个时长就不再等待。
const CALL_TIMEOUT: Duration = Duration::from_secs(600);

/// 边读边累计，超过上限立即停止；`Content-Length` 只用于提前拒绝，不作为唯一依据。
async fn read_limited(response: reqwest::Response, max_bytes: usize) -> Result<Vec<u8>> {
    use futures_util::StreamExt;

    ensure!(
        response.content_length().is_none_or(|length| length <= max_bytes as u64),
        "服务端响应超过 {max_bytes} 字节上限"
    );
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        ensure!(chunk.len() <= max_bytes - bytes.len(), "服务端响应超过 {max_bytes} 字节上限");
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
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
    fn feed_edit_details(&self, feed_id: i64) -> rssr_domain::FeedEditDetails
        => local(feed_id), remote(c) c.call::<protocol::FeedEditDetails>(&feed_id);
    fn update_subscription(&self, edit: rssr_domain::FeedEditRequest) -> ()
        => local(edit), remote(c) c.call::<protocol::UpdateSubscription>(&edit);
    fn apply_entry_batch(&self, request: rssr_domain::EntryBatchRequest) -> u64
        => local(request), remote(c) c.call::<protocol::ApplyEntryBatch>(&request);
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
