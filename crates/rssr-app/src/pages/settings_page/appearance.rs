use dioxus::prelude::*;

use super::{
    facade::SettingsPageFacade, preferences::ReadingPreferencesSection,
    themes::ThemeSettingsSections,
};

#[component]
pub(crate) fn AppearanceSettingsCard(facade: SettingsPageFacade) -> Element {
    rsx! {
        div { class: "settings-card",
            div { class: "settings-card__header",
                h3 { "阅读外观" }
            }
            ReadingPreferencesSection { facade: facade.clone() }
            ThemeSettingsSections { facade: facade.clone() }
            div { class: "settings-card__footer",
                button {
                    class: "button",
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
