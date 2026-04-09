mod lab;
mod presets;
mod theme_apply;
mod theme_io;
mod theme_preset;
mod theme_validation;

use crate::pages::settings_page::save::SettingsPageSaveSession;
use crate::pages::settings_page::session::SettingsPageSession;
use dioxus::prelude::*;

use self::{lab::ThemeLabSection, presets::ThemePresetSections};

pub(crate) use self::theme_preset::detect_preset_key;
pub(crate) use self::theme_validation::validate_custom_css;

#[component]
pub(crate) fn ThemeSettingsSections(
    session: SettingsPageSession,
    save_session: SettingsPageSaveSession,
) -> Element {
    rsx! {
        ThemeLabSection { session, save_session }
        ThemePresetSections { session, save_session }
    }
}
