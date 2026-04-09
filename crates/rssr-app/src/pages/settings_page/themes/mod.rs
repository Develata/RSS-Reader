mod lab;
mod presets;
mod theme_apply;
mod theme_io;
mod theme_preset;
mod theme_validation;

use crate::pages::settings_page::facade::SettingsPageFacade;
use dioxus::prelude::*;

use self::{lab::ThemeLabSection, presets::ThemePresetSections};

pub(crate) use self::theme_preset::detect_preset_key;
pub(crate) use self::theme_validation::validate_custom_css;

#[component]
pub(crate) fn ThemeSettingsSections(facade: SettingsPageFacade) -> Element {
    rsx! {
        ThemeLabSection { facade: facade.clone() }
        ThemePresetSections { facade }
    }
}
