use dioxus::prelude::WritableExt;

use crate::pages::settings_page::{save::SettingsPageSaveSession, session::SettingsPageSession};

use super::theme_preset::{detect_preset_key, preset_css, preset_display_name};
use super::theme_validation::validate_custom_css;

pub(super) fn apply_builtin_theme(
    session: SettingsPageSession,
    save_session: SettingsPageSaveSession,
    preset_key: &str,
) {
    let mut next = session.draft()();
    next.custom_css = preset_css(preset_key).to_string();
    session.preset_choice().set(preset_key.to_string());
    session.draft().set(next);
    save_session
        .save_with_message(format!("已应用示例主题：{}。", preset_display_name(preset_key)));
}

pub(super) fn clear_custom_css(
    session: SettingsPageSession,
    save_session: SettingsPageSaveSession,
    success_message: impl Into<String>,
) {
    let mut next = session.draft()();
    next.custom_css.clear();
    session.preset_choice().set("none".to_string());
    session.draft().set(next);
    save_session.save_with_message(success_message.into());
}

pub(super) fn apply_custom_css_from_raw(
    session: SettingsPageSession,
    save_session: SettingsPageSaveSession,
    raw: String,
    success_message: impl Into<String>,
) {
    if let Err(err) = validate_custom_css(&raw) {
        crate::status::set_status_error(
            session.status_signal(),
            session.status_tone_signal(),
            format!("自定义 CSS 格式无效：{err}"),
        );
        return;
    }

    let mut next = session.draft()();
    next.custom_css = raw;
    session.preset_choice().set(detect_preset_key(&next.custom_css).to_string());
    session.draft().set(next);
    save_session.save_with_message(success_message.into());
}
