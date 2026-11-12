use rssr_application::RemoteConfigPullOutcome;

use crate::{
    pages::settings_page::intent::SettingsPageIntent,
    ui::{
        commands::SettingsCommand, failure::UiFailure, runtime::backend::backend,
        snapshot::UiIntent,
    },
};

pub(super) async fn execute(command: SettingsCommand) -> Vec<UiIntent> {
    match command {
        SettingsCommand::Load => match async { backend().await?.load_settings().await }.await {
            Ok(settings) => settings_intents(vec![SettingsPageIntent::SettingsLoaded(settings)]),
            Err(error) => settings_failed("读取设置", error),
        },
        SettingsCommand::UpdateSettings { patch, success_message } => {
            match async { backend().await?.update_settings(patch).await }.await {
                Ok(settings) => settings_intents(vec![
                    SettingsPageIntent::SettingsLoaded(settings),
                    SettingsPageIntent::SetStatus {
                        message: success_message,
                        tone: "info".to_string(),
                    },
                ]),
                Err(error) => settings_failed("保存设置", error),
            }
        }
        SettingsCommand::PushConfig { target } => {
            let pushed = async { backend().await?.push_remote_config(&target).await }.await;
            match pushed {
                Ok(outcome) => settings_intents(vec![SettingsPageIntent::SetStatus {
                    message: format!(
                        "配置已上传到 WebDAV：{} 个订阅。",
                        outcome.exported_feed_count
                    ),
                    tone: "info".to_string(),
                }]),
                Err(error) => settings_failed("上传配置", error),
            }
        }
        SettingsCommand::PullConfig { target } => {
            let pulled = async { backend().await?.pull_remote_config(&target).await }.await;
            match pulled {
                Ok(RemoteConfigPullOutcome { import: Some(import) }) => settings_intents(vec![
                    SettingsPageIntent::SettingsLoaded(import.settings.clone()),
                    SettingsPageIntent::SetStatus {
                        message: format!(
                            "已从 WebDAV 下载并导入配置：{}。",
                            super::config_import_summary(&import)
                        ),
                        tone: "info".to_string(),
                    },
                ]),
                Ok(RemoteConfigPullOutcome { import: None }) => {
                    settings_intents(vec![SettingsPageIntent::SetStatus {
                        message: "远端配置不存在。".to_string(),
                        tone: "info".to_string(),
                    }])
                }
                Err(error) => settings_failed("下载配置", error),
            }
        }
        SettingsCommand::LoadRemoteSync => {
            match async { backend().await?.load_remote_sync().await }.await {
                Ok(view) => vec![UiIntent::RemoteSyncLoaded(view)],
                Err(error) => settings_failed("读取 WebDAV 自动同步", error),
            }
        }
        SettingsCommand::SaveRemoteSync { endpoint, remote_path } => {
            match async { backend().await?.save_remote_sync(&endpoint, &remote_path).await }.await {
                Ok(view) => vec![
                    UiIntent::RemoteSyncLoaded(view),
                    UiIntent::SettingsPage(SettingsPageIntent::SetStatus {
                        message: "已记住 WebDAV 端点：之后启动和修改配置后会自动同步。".to_string(),
                        tone: "info".to_string(),
                    }),
                ],
                Err(error) => settings_failed("记住 WebDAV 端点", error),
            }
        }
        SettingsCommand::ClearRemoteSync => {
            let cleared = async {
                let backend = backend().await?;
                backend.clear_remote_sync().await?;
                backend.load_remote_sync().await
            }
            .await;
            match cleared {
                Ok(view) => vec![
                    UiIntent::RemoteSyncLoaded(view),
                    UiIntent::SettingsPage(SettingsPageIntent::SetStatus {
                        message: "已停用 WebDAV 自动同步，并删除保存的凭据。".to_string(),
                        tone: "info".to_string(),
                    }),
                ],
                Err(error) => settings_failed("停用 WebDAV 自动同步", error),
            }
        }
    }
}

fn settings_intents(intents: Vec<SettingsPageIntent>) -> Vec<UiIntent> {
    intents.into_iter().map(UiIntent::SettingsPage).collect()
}

fn settings_failed(action: &'static str, error: rssr_api::BackendError) -> Vec<UiIntent> {
    vec![UiIntent::Failed(UiFailure::new(action, error))]
}
