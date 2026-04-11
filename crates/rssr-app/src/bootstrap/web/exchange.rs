use rssr_application::{ImportExportService, RemoteConfigStore};

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
