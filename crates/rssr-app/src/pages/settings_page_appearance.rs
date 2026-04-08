use dioxus::prelude::*;
use rssr_domain::UserSettings;

use super::{
    settings_page_preferences::ReadingPreferencesSection,
    settings_page_save_session::SettingsPageSaveSession,
    settings_page_save_state::SettingsPageSaveState, settings_page_themes::ThemeSettingsSections,
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
    let save_state = use_signal(SettingsPageSaveState::new);
    let save_session =
        SettingsPageSaveSession::new(save_state, theme, draft, preset_choice, status, status_tone);
    let save_snapshot = save_session.snapshot();

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
            div { class: "settings-card__footer",
                button {
                    class: "button",
                    disabled: save_snapshot.pending_save,
                    "data-action": "save-settings",
                    onclick: move |_| save_session.save(),
                    if save_snapshot.pending_save { "正在保存…" } else { "保存设置" }
                }
            }
        }
    }
}
