use crate::bootstrap::AppServices;

use crate::pages::settings_page::intent::SettingsPageIntent;

use super::effect::SettingsPageSyncEffect;

pub(crate) struct SettingsPageSyncRuntimeOutcome {
    pub(crate) page_intents: Vec<SettingsPageIntent>,
}

pub(crate) async fn execute_settings_page_sync_effect(
    effect: SettingsPageSyncEffect,
) -> SettingsPageSyncRuntimeOutcome {
    match effect {
        SettingsPageSyncEffect::PushConfig { endpoint, remote_path } => {
            match AppServices::shared().await {
                Ok(services) => match services.push_remote_config(&endpoint, &remote_path).await {
                    Ok(()) => info("配置已上传到 WebDAV。"),
                    Err(err) => error(format!("上传配置失败：{err}")),
                },
                Err(err) => error(format!("初始化应用失败：{err}")),
            }
        }
        SettingsPageSyncEffect::PullConfig { endpoint, remote_path } => {
            match AppServices::shared().await {
                Ok(services) => match services.pull_remote_config(&endpoint, &remote_path).await {
                    Ok(true) => match services.load_settings().await {
                        Ok(settings) => SettingsPageSyncRuntimeOutcome {
                            page_intents: vec![
                                SettingsPageIntent::SettingsLoaded(settings),
                                SettingsPageIntent::SetStatus {
                                    message: "已从 WebDAV 下载并导入配置。".to_string(),
                                    tone: "info".to_string(),
                                },
                            ],
                        },
                        Err(err) => error(format!("导入后读取设置失败：{err}")),
                    },
                    Ok(false) => info("远端配置不存在。"),
                    Err(err) => error(format!("下载配置失败：{err}")),
                },
                Err(err) => error(format!("初始化应用失败：{err}")),
            }
        }
    }
}

fn info(message: impl Into<String>) -> SettingsPageSyncRuntimeOutcome {
    SettingsPageSyncRuntimeOutcome {
        page_intents: vec![SettingsPageIntent::SetStatus {
            message: message.into(),
            tone: "info".to_string(),
        }],
    }
}

fn error(message: impl Into<String>) -> SettingsPageSyncRuntimeOutcome {
    SettingsPageSyncRuntimeOutcome {
        page_intents: vec![SettingsPageIntent::SetStatus {
            message: message.into(),
            tone: "error".to_string(),
        }],
    }
}
