use anyhow::Context;

use super::{AppServices, exchange_adapter::WebRemoteConfigStore};

pub(super) async fn export_config_json(services: &AppServices) -> anyhow::Result<String> {
    services.import_export_service.export_config_json().await
}

pub(super) async fn import_config_json(services: &AppServices, raw: &str) -> anyhow::Result<()> {
    services.import_export_service.import_config_json(raw).await
}

pub(super) async fn export_opml(services: &AppServices) -> anyhow::Result<String> {
    services.import_export_service.export_opml().await
}

pub(super) async fn import_opml(services: &AppServices, raw: &str) -> anyhow::Result<()> {
    services.import_export_service.import_opml(raw).await
}

pub(super) async fn push_remote_config(
    services: &AppServices,
    endpoint: &str,
    remote_path: &str,
) -> anyhow::Result<()> {
    let remote = WebRemoteConfigStore::new(services.client.clone(), endpoint, remote_path)?;
    services
        .import_export_service
        .push_remote_config(&remote)
        .await
        .context("上传配置到 WebDAV 失败")
}

pub(super) async fn pull_remote_config(
    services: &AppServices,
    endpoint: &str,
    remote_path: &str,
) -> anyhow::Result<bool> {
    let remote = WebRemoteConfigStore::new(services.client.clone(), endpoint, remote_path)?;
    services.import_export_service.pull_remote_config(&remote).await.context("拉取远端配置失败")
}
