use crate::pages::settings_page::facade::SettingsPageFacade;

use super::theme_preset::{detect_preset_key, preset_css, preset_display_name};
use super::theme_validation::validate_custom_css;

pub(super) fn apply_builtin_theme(facade: &SettingsPageFacade, preset_key: &str) {
    facade.update_draft(|next| {
        next.custom_css = preset_css(preset_key).to_string();
    });
    facade.set_preset_choice(preset_key);
    facade.save_with_message(format!("已应用示例主题：{}。", preset_display_name(preset_key)));
}

pub(super) fn clear_custom_css(facade: &SettingsPageFacade, success_message: impl Into<String>) {
    facade.update_draft(|next| {
        next.custom_css.clear();
    });
    facade.set_preset_choice("none");
    facade.save_with_message(success_message.into());
}

pub(super) fn apply_custom_css_from_raw(
    facade: &SettingsPageFacade,
    raw: String,
    success_message: impl Into<String>,
) {
    if let Err(err) = validate_custom_css(&raw) {
        facade.set_status(format!("自定义 CSS 格式无效：{err}"), "error");
        return;
    }

    let preset = detect_preset_key(&raw).to_string();
    facade.update_draft(|next| {
        next.custom_css = raw;
    });
    facade.set_preset_choice(preset);
    facade.save_with_message(success_message.into());
}
