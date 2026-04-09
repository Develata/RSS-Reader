use dioxus::prelude::WritableExt;

use crate::pages::settings_page::facade::SettingsPageFacade;

use super::theme_preset::{detect_preset_key, preset_css, preset_display_name};
use super::theme_validation::validate_custom_css;

pub(super) fn apply_builtin_theme(facade: &SettingsPageFacade, preset_key: &str) {
    let mut draft = facade.draft_signal();
    let mut preset_choice = facade.preset_choice_signal();
    let mut next = draft();
    next.custom_css = preset_css(preset_key).to_string();
    preset_choice.set(preset_key.to_string());
    draft.set(next);
    facade.save_with_message(format!("已应用示例主题：{}。", preset_display_name(preset_key)));
}

pub(super) fn clear_custom_css(facade: &SettingsPageFacade, success_message: impl Into<String>) {
    let mut draft = facade.draft_signal();
    let mut preset_choice = facade.preset_choice_signal();
    let mut next = draft();
    next.custom_css.clear();
    preset_choice.set("none".to_string());
    draft.set(next);
    facade.save_with_message(success_message.into());
}

pub(super) fn apply_custom_css_from_raw(
    facade: &SettingsPageFacade,
    raw: String,
    success_message: impl Into<String>,
) {
    if let Err(err) = validate_custom_css(&raw) {
        crate::status::set_status_error(
            facade.status_signal(),
            facade.status_tone_signal(),
            format!("自定义 CSS 格式无效：{err}"),
        );
        return;
    }

    let mut draft = facade.draft_signal();
    let mut preset_choice = facade.preset_choice_signal();
    let mut next = draft();
    next.custom_css = raw;
    preset_choice.set(detect_preset_key(&next.custom_css).to_string());
    draft.set(next);
    facade.save_with_message(success_message.into());
}
