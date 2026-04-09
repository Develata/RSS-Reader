use dioxus::prelude::*;

use super::{
    facade::SettingsPageFacade, preferences::ReadingPreferencesSection,
    themes::ThemeSettingsSections,
};

#[component]
pub(crate) fn AppearanceSettingsCard(facade: SettingsPageFacade) -> Element {
    let draft = facade.draft_signal();

    rsx! {
        div { class: "settings-card",
            div { class: "settings-card__header",
                h3 { "阅读外观" }
            }
            ReadingPreferencesSection { draft }
            ThemeSettingsSections { facade: facade.clone() }
            div { class: "settings-card__footer",
                button {
                    class: "button",
                    disabled: facade.pending_save(),
                    "data-action": "save-settings",
                    onclick: move |_| facade.save(),
                    if facade.pending_save() { "正在保存…" } else { "保存设置" }
                }
            }
        }
    }
}
