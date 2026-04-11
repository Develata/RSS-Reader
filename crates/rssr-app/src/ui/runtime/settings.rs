use crate::{
    pages::settings_page::intent::SettingsPageIntent,
    ui::{commands::SettingsCommand, runtime::services::UiServices, snapshot::UiIntent},
};
use rssr_application::ConfigImportOutcome;

pub(super) async fn execute(command: SettingsCommand) -> Vec<UiIntent> {
    match command {
        SettingsCommand::Load => match UiServices::shared().await {
            Ok(services) => match services.settings().load_settings().await {
                Ok(settings) => {
                    settings_intents(vec![SettingsPageIntent::SettingsLoaded(settings)])
                }
                Err(err) => settings_status_error(format!("读取设置失败：{err}")),
            },
            Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
        },
        SettingsCommand::SaveAppearance { settings, success_message } => {
            match UiServices::shared().await {
                Ok(services) => match services.settings().save_settings(&settings).await {
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
        SettingsCommand::PushConfig { endpoint, remote_path } => match UiServices::shared().await {
            Ok(services) => {
                match services.settings().push_remote_config(&endpoint, &remote_path).await {
                    Ok(outcome) => settings_intents(vec![SettingsPageIntent::SetStatus {
                        message: format!(
                            "配置已上传到 WebDAV：{} 个订阅。",
                            outcome.exported_feed_count
                        ),
                        tone: "info".to_string(),
                    }]),
                    Err(err) => settings_status_error(format!("上传配置失败：{err}")),
                }
            }
            Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
        },
        SettingsCommand::PullConfig { endpoint, remote_path } => match UiServices::shared().await {
            Ok(services) => {
                match services.settings().pull_remote_config(&endpoint, &remote_path).await {
                    Ok(outcome) if outcome.found() => {
                        let import = outcome.import().expect("found outcome has import");
                        match services.settings().load_settings().await {
                            Ok(settings) => settings_intents(vec![
                                SettingsPageIntent::SettingsLoaded(settings),
                                SettingsPageIntent::SetStatus {
                                    message: format!(
                                        "已从 WebDAV 下载并导入配置：{}。",
                                        config_import_summary(import)
                                    ),
                                    tone: "info".to_string(),
                                },
                            ]),
                            Err(err) => settings_status_error(format!("导入后读取设置失败：{err}")),
                        }
                    }
                    Ok(_) => settings_intents(vec![SettingsPageIntent::SetStatus {
                        message: "远端配置不存在。".to_string(),
                        tone: "info".to_string(),
                    }]),
                    Err(err) => settings_status_error(format!("下载配置失败：{err}")),
                }
            }
            Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
        },
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

fn config_import_summary(outcome: &ConfigImportOutcome) -> String {
    let settings = if outcome.settings_updated { "设置已更新" } else { "设置未变化" };
    format!(
        "导入 {} 个订阅，清理 {} 个缺失订阅，{settings}",
        outcome.imported_feed_count, outcome.removed_feed_count
    )
}
