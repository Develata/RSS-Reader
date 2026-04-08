use dioxus::prelude::*;
use rssr_domain::UserSettings;

use crate::{
    bootstrap::AppServices,
    status::{set_status_error, set_status_info},
    theme::ThemeController,
};

use super::theme_preset::{detect_preset_key, preset_css};
use super::theme_validation::validate_custom_css;

pub(super) fn apply_builtin_theme(
    theme: ThemeController,
    mut draft: Signal<UserSettings>,
    mut preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
    preset_key: &str,
    preset_name: &str,
) {
    let mut next = draft();
    next.custom_css = preset_css(preset_key).to_string();
    preset_choice.set(preset_key.to_string());
    let applied = next.clone();
    draft.set(next);
    apply_settings_immediately(
        theme,
        draft,
        preset_choice,
        status,
        status_tone,
        applied,
        format!("已应用示例主题：{preset_name}。"),
    );
}

pub(super) fn apply_settings_immediately(
    mut theme: ThemeController,
    mut draft: Signal<UserSettings>,
    mut preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
    next: UserSettings,
    success_message: String,
) {
    let previous = (theme.settings)();
    let previous_preset = detect_preset_key(&previous.custom_css).to_string();
    theme.settings.set(next.clone());
    spawn(async move {
        match AppServices::shared().await {
            Ok(services) => match services.save_settings(&next).await {
                Ok(()) => set_status_info(status, status_tone, success_message),
                Err(err) => {
                    theme.settings.set(previous.clone());
                    draft.set(previous);
                    preset_choice.set(previous_preset);
                    set_status_error(status, status_tone, format!("保存设置失败：{err}"));
                }
            },
            Err(err) => {
                theme.settings.set(previous.clone());
                draft.set(previous);
                preset_choice.set(previous_preset);
                set_status_error(status, status_tone, format!("初始化应用失败：{err}"));
            }
        }
    });
}

pub(super) fn apply_custom_css_from_raw(
    theme: ThemeController,
    mut draft: Signal<UserSettings>,
    mut preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
    raw: String,
    success_message: String,
) {
    if let Err(err) = validate_custom_css(&raw) {
        set_status_error(status, status_tone, format!("自定义 CSS 格式无效：{err}"));
        return;
    }

    let mut next = draft();
    next.custom_css = raw;
    preset_choice.set(detect_preset_key(&next.custom_css).to_string());
    let applied = next.clone();
    draft.set(next);
    apply_settings_immediately(
        theme,
        draft,
        preset_choice,
        status,
        status_tone,
        applied,
        success_message,
    );
}
