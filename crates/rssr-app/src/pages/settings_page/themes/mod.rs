mod lab;
mod presets;
mod theme_apply;
mod theme_io;
mod theme_preset;
mod theme_validation;

use dioxus::prelude::*;
use rssr_domain::UserSettings;

use crate::theme::ThemeController;

use self::{lab::ThemeLabSection, presets::ThemePresetSections};

pub(crate) use self::theme_preset::detect_preset_key;
pub(crate) use self::theme_validation::validate_custom_css;

#[component]
pub(crate) fn ThemeSettingsSections(
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
    rsx! {
        ThemeLabSection {
            theme,
            draft,
            preset_choice,
            status,
            status_tone,
        }
        ThemePresetSections {
            theme,
            draft,
            preset_choice,
            status,
            status_tone,
        }
    }
}
