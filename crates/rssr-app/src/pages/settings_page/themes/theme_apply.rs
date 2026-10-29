use crate::pages::settings_page::facade::SettingsPageFacade;

pub(super) fn apply_custom_css_from_raw(
    facade: &SettingsPageFacade,
    raw: String,
    success_message: impl Into<String>,
) {
    facade.preview_custom_css(raw, success_message);
}
