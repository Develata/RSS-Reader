use dioxus::prelude::*;
use rssr_domain::UserSettings;

use super::{
    settings_page_preferences::ReadingPreferencesSection,
    settings_page_themes::ThemeSettingsSections,
};
use crate::theme::ThemeController;

#[component]
pub(crate) fn AppearanceSettingsCard(
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
    rsx! {
        div { class: "settings-card",
            div { class: "settings-card__header",
                h3 { "阅读外观" }
            }
            ReadingPreferencesSection { draft }
            ThemeSettingsSections {
                theme,
                draft,
                preset_choice,
                status,
                status_tone,
            }
        }
    }
}
