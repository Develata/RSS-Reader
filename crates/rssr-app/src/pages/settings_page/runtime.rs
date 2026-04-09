use crate::bootstrap::AppServices;

use super::{effect::SettingsPageEffect, intent::SettingsPageIntent};

pub(crate) struct SettingsPageRuntimeOutcome {
    pub(crate) intents: Vec<SettingsPageIntent>,
}

pub(crate) async fn execute_settings_page_effect(
    effect: SettingsPageEffect,
) -> SettingsPageRuntimeOutcome {
    match effect {
        SettingsPageEffect::LoadSettings => match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(settings) => SettingsPageRuntimeOutcome {
                    intents: vec![SettingsPageIntent::SettingsLoaded(settings)],
                },
                Err(err) => status_error(format!("读取设置失败：{err}")),
            },
            Err(err) => status_error(format!("初始化应用失败：{err}")),
        },
    }
}

fn status_error(message: impl Into<String>) -> SettingsPageRuntimeOutcome {
    SettingsPageRuntimeOutcome {
        intents: vec![SettingsPageIntent::SetStatus {
            message: message.into(),
            tone: "error".to_string(),
        }],
    }
}
