use dioxus::prelude::*;

use super::{
    facade::SettingsPageFacade, preferences::ReadingPreferencesSection,
    themes::ThemeSettingsSections,
};

#[component]
pub(crate) fn AppearanceSettingsCard(facade: SettingsPageFacade) -> Element {
    rsx! {
        div { "data-layout": "settings-card", "data-section": "appearance",
            div { "data-slot": "settings-card-header",
                h3 { "data-slot": "card-title", "阅读外观" }
            }
            ReadingPreferencesSection { facade: facade.clone() }
            ThemeSettingsSections { facade: facade.clone() }
            div { "data-layout": "settings-card-footer",
                button {
                    class: "button",
                    "data-variant": "primary",
                    "data-state": "{facade.save_state()}",
                    disabled: facade.is_save_pending(),
                    "data-action": "save-settings",
                    onclick: move |_| facade.save(),
                    "{facade.save_button_label()}"
                }
            }
        }
    }
}
