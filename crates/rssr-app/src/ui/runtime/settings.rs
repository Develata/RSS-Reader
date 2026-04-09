use crate::{
    bootstrap::AppServices,
    pages::settings_page::intent::SettingsPageIntent,
    ui::{commands::SettingsCommand, snapshot::UiIntent},
};

pub(super) async fn execute(command: SettingsCommand) -> Vec<UiIntent> {
    match command {
        SettingsCommand::Load => match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(settings) => {
                    settings_intents(vec![SettingsPageIntent::SettingsLoaded(settings)])
                }
                Err(err) => settings_status_error(format!("读取设置失败：{err}")),
            },
            Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
        },
        SettingsCommand::SaveAppearance { settings, success_message } => {
            match AppServices::shared().await {
                Ok(services) => match services.save_settings(&settings).await {
                    Ok(()) => settings_intents(vec![
                        SettingsPageIntent::SettingsLoaded(settings),
                        SettingsPageIntent::SetStatus {
                            message: success_message,
                            tone: "info".to_string(),
                        },
                    ]),
                    Err(err) => settings_status_error(format!("保存设置失败：{err}")),
                },
                Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
            }
        }
        SettingsCommand::PushConfig { endpoint, remote_path } => {
            match AppServices::shared().await {
                Ok(services) => match services.push_remote_config(&endpoint, &remote_path).await {
                    Ok(()) => settings_intents(vec![SettingsPageIntent::SetStatus {
                        message: "配置已上传到 WebDAV。".to_string(),
                        tone: "info".to_string(),
                    }]),
                    Err(err) => settings_status_error(format!("上传配置失败：{err}")),
                },
                Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
            }
        }
        SettingsCommand::PullConfig { endpoint, remote_path } => {
            match AppServices::shared().await {
                Ok(services) => match services.pull_remote_config(&endpoint, &remote_path).await {
                    Ok(true) => match services.load_settings().await {
                        Ok(settings) => settings_intents(vec![
                            SettingsPageIntent::SettingsLoaded(settings),
                            SettingsPageIntent::SetStatus {
                                message: "已从 WebDAV 下载并导入配置。".to_string(),
                                tone: "info".to_string(),
                            },
                        ]),
                        Err(err) => settings_status_error(format!("导入后读取设置失败：{err}")),
                    },
                    Ok(false) => settings_intents(vec![SettingsPageIntent::SetStatus {
                        message: "远端配置不存在。".to_string(),
                        tone: "info".to_string(),
                    }]),
                    Err(err) => settings_status_error(format!("下载配置失败：{err}")),
                },
                Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
            }
        }
    }
}

fn settings_intents(intents: Vec<SettingsPageIntent>) -> Vec<UiIntent> {
    intents.into_iter().map(UiIntent::SettingsPage).collect()
}

fn settings_status_error(message: impl Into<String>) -> Vec<UiIntent> {
    settings_intents(vec![SettingsPageIntent::SetStatus {
        message: message.into(),
        tone: "error".to_string(),
    }])
}
