use rssr_domain::UserSettings;

use crate::bootstrap::AppServices;

use super::effect::SettingsPageSyncEffect;

pub(crate) struct SettingsPageSyncRuntimeOutcome {
    pub(crate) status_message: String,
    pub(crate) status_tone: String,
    pub(crate) imported_settings: Option<UserSettings>,
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
                            status_message: "已从 WebDAV 下载并导入配置。".to_string(),
                            status_tone: "info".to_string(),
                            imported_settings: Some(settings),
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
        status_message: message.into(),
        status_tone: "info".to_string(),
        imported_settings: None,
    }
}

fn error(message: impl Into<String>) -> SettingsPageSyncRuntimeOutcome {
    SettingsPageSyncRuntimeOutcome {
        status_message: message.into(),
        status_tone: "error".to_string(),
        imported_settings: None,
    }
}
