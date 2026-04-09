use crate::pages::settings_page::facade::SettingsPageFacade;

use super::theme_preset::detect_preset_key;
use super::theme_validation::validate_custom_css;

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
