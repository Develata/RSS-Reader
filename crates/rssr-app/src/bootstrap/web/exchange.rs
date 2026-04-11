use rssr_application::{
    ImportExportService, RemoteConfigPullOutcome, RemoteConfigPushOutcome, RemoteConfigStore,
};

pub(super) async fn push_remote_config(
    service: &ImportExportService,
    remote: &dyn RemoteConfigStore,
) -> anyhow::Result<RemoteConfigPushOutcome> {
    service.push_remote_config(remote).await
}

pub(super) async fn pull_remote_config(
    service: &ImportExportService,
    remote: &dyn RemoteConfigStore,
) -> anyhow::Result<RemoteConfigPullOutcome> {
    service.pull_remote_config(remote).await
}
