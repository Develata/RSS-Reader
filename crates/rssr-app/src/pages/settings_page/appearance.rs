use dioxus::prelude::*;

use crate::components::status_banner::StatusBanner;

use super::{
    facade::SettingsPageFacade, preferences::ReadingPreferencesSection,
    themes::ThemeSettingsSections,
};

#[component]
pub(crate) fn AppearanceSettingsCard(facade: SettingsPageFacade) -> Element {
    rsx! {
        div { "data-layout": "settings-card", "data-section": "appearance",
            div { "data-slot": "settings-card-header",
                h2 { "data-slot": "card-title", "阅读外观" }
            }
            ReadingPreferencesSection { facade: facade.clone() }
            ThemeSettingsSections { facade }
        }
    }
}

#[component]
pub(crate) fn SettingsSaveRegion(facade: SettingsPageFacade) -> Element {
    let dirty = facade.has_unsaved_changes();
    let pending = facade.is_save_pending();
    let state = if pending {
        "pending"
    } else if dirty {
        "dirty"
    } else {
        "clean"
    };
    let label = if pending {
        "正在保存设置…"
    } else if dirty {
        "有未保存修改"
    } else {
        "没有未保存修改"
    };
    let save_facade = facade.clone();

    rsx! {
        div { "data-layout": "settings-save-region", "data-state": state,
            div { "data-slot": "settings-save-feedback",
                p { "data-slot": "settings-save-status", "{label}" }
                p { id: "settings-save-hint", "data-slot": "settings-save-hint", "预览仅在本页有效。离开此页会放弃未保存修改。" }
                StatusBanner { message: facade.status_message(), tone: facade.status_tone(), announce: true }
            }
            button {
                class: "button",
                "data-variant": "primary",
                "data-state": "{facade.save_state()}",
                disabled: pending,
                "data-action": "save-settings",
                aria_describedby: "settings-save-hint",
                onclick: move |_| save_facade.save(),
                "{facade.save_button_label()}"
            }
        }
    }
}
