use rssr_application::{ImportExportService, RemoteConfigStore};

pub(super) async fn export_config_json(service: &ImportExportService) -> anyhow::Result<String> {
    service.export_config_json().await
}

pub(super) async fn import_config_json(
    service: &ImportExportService,
    raw: &str,
) -> anyhow::Result<()> {
    service.import_config_json(raw).await
}

pub(super) async fn export_opml(service: &ImportExportService) -> anyhow::Result<String> {
    service.export_opml().await
}

pub(super) async fn import_opml(service: &ImportExportService, raw: &str) -> anyhow::Result<()> {
    service.import_opml(raw).await
}

pub(super) async fn push_remote_config(
    service: &ImportExportService,
    remote: &dyn RemoteConfigStore,
) -> anyhow::Result<()> {
    service.push_remote_config(remote).await
}

pub(super) async fn pull_remote_config(
    service: &ImportExportService,
    remote: &dyn RemoteConfigStore,
) -> anyhow::Result<bool> {
    service.pull_remote_config(remote).await
}
