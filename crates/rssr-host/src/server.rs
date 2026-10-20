//! 服务端分派：按 [`crate::protocol`] 的端点路径反序列化入参，调用 [`HostServices`]，
//! 序列化结果。HTTP 细节（认证、头部校验、状态码）留给 `rssr-web`。
use std::future::Future;

use crate::{
    protocol::{self, Endpoint},
    services::HostServices,
};

#[derive(Debug)]
pub enum DispatchError {
    /// 路径不是任何端点。
    UnknownEndpoint,
    /// 入参无法按端点的输入类型解析。
    BadRequest(String),
    /// 用例或宿主能力返回的错误，原样交给界面展示。
    Failed(anyhow::Error),
}

async fn call<E, F, Fut>(body: &[u8], handler: F) -> Result<Vec<u8>, DispatchError>
where
    E: Endpoint,
    F: FnOnce(E::Input) -> Fut,
    Fut: Future<Output = anyhow::Result<E::Output>>,
{
    let input: E::Input = serde_json::from_slice(body)
        .map_err(|error| DispatchError::BadRequest(error.to_string()))?;
    let output = handler(input).await.map_err(DispatchError::Failed)?;
    serde_json::to_vec(&output).map_err(|error| DispatchError::Failed(error.into()))
}

/// 执行 `path`（相对 [`protocol::API_PREFIX`]）对应的端点。
pub async fn dispatch(
    services: &HostServices,
    path: &str,
    body: &[u8],
) -> Result<Vec<u8>, DispatchError> {
    let s = services;
    macro_rules! route {
        ($endpoint:ty, |$input:pat_param| $call:expr) => {
            if path == <$endpoint as Endpoint>::PATH {
                return call::<$endpoint, _, _>(body, |$input| async move { $call.await }).await;
            }
        };
    }

    route!(protocol::PreviewMarkRead, |query| s.preview_mark_read(&query));
    route!(protocol::MarkReadIfUnchanged, |preview| s.mark_read_if_unchanged(&preview));
    route!(protocol::BootstrapEntries, |input| s.bootstrap_entries(input));
    route!(protocol::SaveWorkspaceIfChanged, |workspace| s.save_workspace_if_changed(workspace));
    route!(protocol::ListEntries, |query| s.list_entries(&query));
    route!(protocol::ToggleEntryRead, |input| s.toggle_entry_read(input));
    route!(protocol::ToggleEntryStarred, |input| s.toggle_entry_starred(input));
    route!(protocol::ResolveStartupTarget, |()| s.resolve_startup_target());
    route!(protocol::LoadSettings, |()| s.load_settings());
    route!(protocol::SaveSettings, |settings| s.save_settings(&settings));
    route!(protocol::PushRemoteConfig, |request| s
        .push_remote_config(&request.endpoint, &request.remote_path));
    route!(protocol::PullRemoteConfig, |request| s
        .pull_remote_config_and_load_settings(&request.endpoint, &request.remote_path));
    route!(protocol::LoadEntry, |entry_id| s.load_entry(entry_id));
    route!(protocol::ToggleReaderRead, |input| s.toggle_reader_read(input));
    route!(protocol::ToggleReaderStarred, |input| s.toggle_reader_starred(input));
    route!(protocol::LocalizeEntryAssets, |entry_id| s.localize_entry_assets(entry_id));
    route!(protocol::LoadFeedsSnapshot, |()| s.load_feeds_snapshot());
    route!(protocol::AddSubscription, |request| s
        .add_subscription(&request.raw_url, request.fallback_site_url));
    route!(protocol::RefreshAll, |()| s.refresh_all());
    route!(protocol::RefreshFeed, |feed_id| s.refresh_feed(feed_id));
    route!(protocol::RemoveFeed, |feed_id| s.remove_feed(feed_id));
    route!(protocol::ExportConfigJson, |()| s.export_config_json());
    route!(protocol::ImportConfigJson, |raw| s.import_config_json(&raw));
    route!(protocol::ExportOpml, |()| s.export_opml());
    route!(protocol::ImportOpml, |raw| s.import_opml(&raw));

    Err(DispatchError::UnknownEndpoint)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rssr_domain::{EntryQuery, UserSettings};
    use rssr_infra::db::sqlite_native::NativeSqliteBackend;

    use super::{DispatchError, dispatch};
    use crate::{HostServices, native::NativeHost, protocol};

    /// 服务端会把分派放进 `tokio::spawn`，future 必须是 `Send`。
    #[allow(dead_code)]
    fn dispatch_future_is_send(services: &'static HostServices) {
        fn assert_send<T: Send>(_: &T) {}
        assert_send(&dispatch(services, "", &[]));
    }

    async fn open_temp_host() -> (Arc<NativeHost>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "rssr-host-dispatch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp data dir");
        let host = NativeHost::open(NativeSqliteBackend::with_path(dir.join("rss-reader.db")))
            .await
            .expect("open native host");
        (host, dir)
    }

    #[tokio::test]
    async fn dispatch_round_trips_typed_inputs_and_outputs() {
        let (host, dir) = open_temp_host().await;
        let services = HostServices::new(host.use_cases(), host.host_capabilities());

        let settings: UserSettings = serde_json::from_slice(
            &dispatch(&services, <protocol::LoadSettings as protocol::Endpoint>::PATH, b"null")
                .await
                .expect("load settings"),
        )
        .expect("settings JSON");
        assert_eq!(settings, UserSettings::default());

        let query = serde_json::to_vec(&EntryQuery::default()).unwrap();
        let listed: rssr_application::EntriesListOutcome = serde_json::from_slice(
            &dispatch(&services, <protocol::ListEntries as protocol::Endpoint>::PATH, &query)
                .await
                .expect("list entries"),
        )
        .expect("entries JSON");
        assert_eq!(listed, services.list_entries(&EntryQuery::default()).await.unwrap());

        let opml: String = serde_json::from_slice(
            &dispatch(&services, <protocol::ExportOpml as protocol::Endpoint>::PATH, b"null")
                .await
                .expect("export opml"),
        )
        .expect("opml JSON");
        assert!(opml.contains("<opml"), "unexpected OPML: {opml}");

        drop((services, host));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn dispatch_separates_unknown_paths_bad_inputs_and_use_case_failures() {
        let (host, dir) = open_temp_host().await;
        let services = HostServices::new(host.use_cases(), host.host_capabilities());

        assert!(matches!(
            dispatch(&services, "feeds/drop_database", b"null").await,
            Err(DispatchError::UnknownEndpoint)
        ));
        assert!(matches!(
            dispatch(&services, <protocol::LoadEntry as protocol::Endpoint>::PATH, b"\"x\"").await,
            Err(DispatchError::BadRequest(_))
        ));
        let invalid_url = serde_json::to_vec(&protocol::AddSubscriptionRequest {
            raw_url: "not a url".to_string(),
            fallback_site_url: None,
        })
        .unwrap();
        assert!(matches!(
            dispatch(
                &services,
                <protocol::AddSubscription as protocol::Endpoint>::PATH,
                &invalid_url
            )
            .await,
            Err(DispatchError::Failed(_))
        ));

        drop((services, host));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
