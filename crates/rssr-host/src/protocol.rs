//! 服务端模式的线协议：`POST {API_PREFIX}/{Endpoint::PATH}`，请求体是入参 JSON，
//! 成功时响应体是结果 JSON，失败时是 [`ApiError`]。
//!
//! 端点、路径与输入输出类型只在这里定义一次，服务端分派（`server`）与浏览器端的远程调用
//! 共用它，类型不一致在编译期就会暴露。application 类型即协议，所以另带协议版本：
//! 请求必须携带 `PROTOCOL_HEADER: PROTOCOL_VERSION`，服务端拒绝不匹配的版本。
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
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{AddSubscriptionOutcome, RefreshAllExecutionOutcome, RefreshFeedExecutionOutcome};

pub const PROTOCOL_VERSION: u32 = 1;
pub const PROTOCOL_HEADER: &str = "x-rssr-protocol";
pub const API_PREFIX: &str = "/api/v1";
/// 不需要登录的能力探测端点，浏览器端据此判断是否处于服务端模式。
pub const CAPABILITIES_PATH: &str = "/api/v1/capabilities";

pub trait Endpoint {
    /// 相对 [`API_PREFIX`] 的路径，形如 `entries/list`。
    const PATH: &'static str;
    type Input: Serialize + DeserializeOwned;
    type Output: Serialize + DeserializeOwned;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub protocol: u32,
    pub mode: String,
}

impl Capabilities {
    pub const SERVER_MODE: &'static str = "server";

    pub fn server() -> Self {
        Self { protocol: PROTOCOL_VERSION, mode: Self::SERVER_MODE.to_string() }
    }
}

/// 失败响应。`messages` 是错误链，最外层上下文在前，客户端据此还原出同样的错误文案。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub messages: Vec<String>,
}

impl ApiError {
    pub fn message(message: impl Into<String>) -> Self {
        Self { messages: vec![message.into()] }
    }

    pub fn from_anyhow(error: &anyhow::Error) -> Self {
        Self { messages: error.chain().map(ToString::to_string).collect() }
    }

    /// 还原成 `anyhow::Error`：`Display`（最外层）与 `{:#}`（整条链）都与服务端一致。
    pub fn into_anyhow(self) -> anyhow::Error {
        let mut messages = self.messages.into_iter().rev();
        let innermost = messages.next().unwrap_or_else(|| "服务端返回了空错误".to_string());
        messages.fold(anyhow::Error::msg(innermost), |error, context| error.context(context))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddSubscriptionRequest {
    pub raw_url: String,
    pub fallback_site_url: Option<url::Url>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteConfigRequest {
    pub endpoint: String,
    pub remote_path: String,
}

macro_rules! endpoints {
    ($($(#[$meta:meta])* $name:ident => $path:literal : $input:ty => $output:ty;)*) => {
        $(
            $(#[$meta])*
            pub struct $name;
            impl Endpoint for $name {
                const PATH: &'static str = $path;
                type Input = $input;
                type Output = $output;
            }
        )*

        /// 全部端点路径，供服务端路由与测试核对。
        pub const ENDPOINT_PATHS: &[&str] = &[$($path),*];
    };
}

endpoints! {
    PreviewMarkRead => "entries/preview_mark_read": EntryQuery => MarkReadPreview;
    MarkReadIfUnchanged => "entries/mark_read_if_unchanged": MarkReadPreview => MarkReadOutcome;
    BootstrapEntries => "entries/bootstrap": EntriesBootstrapInput => EntriesBootstrapOutcome;
    SaveWorkspaceIfChanged => "entries/save_workspace_if_changed": EntriesWorkspaceState => bool;
    ListEntries => "entries/list": EntryQuery => EntriesListOutcome;
    ToggleEntryRead => "entries/toggle_read": ToggleEntryReadInput => ToggleEntryReadOutcome;
    ToggleEntryStarred => "entries/toggle_starred": ToggleEntryStarredInput => ToggleEntryStarredOutcome;
    ResolveStartupTarget => "shell/resolve_startup_target": () => StartupTarget;
    LoadSettings => "settings/load": () => UserSettings;
    SaveSettings => "settings/save": UserSettings => ();
    PushRemoteConfig => "settings/push_remote_config": RemoteConfigRequest => RemoteConfigPushOutcome;
    PullRemoteConfig => "settings/pull_remote_config": RemoteConfigRequest => AppliedRemoteConfigOutcome;
    LoadEntry => "reader/load_entry": i64 => ReaderEntrySnapshot;
    ToggleReaderRead => "reader/toggle_read": ToggleReadInput => ToggleReadOutcome;
    ToggleReaderStarred => "reader/toggle_starred": ToggleStarredInput => ToggleStarredOutcome;
    LocalizeEntryAssets => "reader/localize_entry_assets": i64 => bool;
    LoadFeedsSnapshot => "feeds/snapshot": () => FeedsSnapshotOutcome;
    AddSubscription => "feeds/add_subscription": AddSubscriptionRequest => AddSubscriptionOutcome;
    RefreshAll => "feeds/refresh_all": () => RefreshAllExecutionOutcome;
    RefreshFeed => "feeds/refresh_feed": i64 => RefreshFeedExecutionOutcome;
    RemoveFeed => "feeds/remove": i64 => ();
    ExportConfigJson => "feeds/export_config_json": () => String;
    ImportConfigJson => "feeds/import_config_json": String => ConfigImportOutcome;
    ExportOpml => "feeds/export_opml": () => String;
    ImportOpml => "feeds/import_opml": String => OpmlImportOutcome;
}

#[cfg(test)]
mod tests {
    use super::{ApiError, ENDPOINT_PATHS};

    #[test]
    fn endpoint_paths_are_unique() {
        let mut paths = ENDPOINT_PATHS.to_vec();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), ENDPOINT_PATHS.len());
    }

    #[test]
    fn api_error_round_trips_the_error_chain() {
        let original = anyhow::anyhow!("operation timed out").context("读取 feed 响应正文失败");
        let restored = ApiError::from_anyhow(&original).into_anyhow();
        assert_eq!(restored.to_string(), original.to_string());
        assert_eq!(format!("{restored:#}"), format!("{original:#}"));
    }
}
