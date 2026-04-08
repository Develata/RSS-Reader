use rssr_domain::UserSettings;

use crate::bootstrap::AppServices;

use super::settings_page_save_effect::SettingsPageSaveEffect;

pub(crate) struct SettingsPageSaveRuntimeOutcome {
    pub(crate) status_message: String,
    pub(crate) saved_settings: Option<UserSettings>,
}

pub(crate) async fn execute_settings_page_save_effect(
    effect: SettingsPageSaveEffect,
) -> SettingsPageSaveRuntimeOutcome {
    match effect {
        SettingsPageSaveEffect::SaveAppearance(settings) => match AppServices::shared().await {
            Ok(services) => match services.save_settings(&settings).await {
                Ok(()) => SettingsPageSaveRuntimeOutcome {
                    status_message: "设置已保存。".to_string(),
                    saved_settings: Some(settings),
                },
                Err(err) => error(format!("保存设置失败：{err}")),
            },
            Err(err) => error(format!("初始化应用失败：{err}")),
        },
    }
}

fn error(message: impl Into<String>) -> SettingsPageSaveRuntimeOutcome {
    SettingsPageSaveRuntimeOutcome {
        status_message: message.into(),
        saved_settings: None,
    }
}
