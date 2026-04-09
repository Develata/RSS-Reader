use dioxus::prelude::*;

use super::{
    preferences::ReadingPreferencesSection, save::SettingsPageSaveSession,
    save::SettingsPageSaveState, session::SettingsPageSession, themes::ThemeSettingsSections,
};

#[component]
pub(crate) fn AppearanceSettingsCard(session: SettingsPageSession) -> Element {
    let save_state = use_signal(SettingsPageSaveState::new);
    let save_session = SettingsPageSaveSession::new(
        save_state,
        session.theme(),
        session.draft(),
        session.preset_choice(),
        session.status_signal(),
        session.status_tone_signal(),
    );
    let save_snapshot = save_session.snapshot();

    rsx! {
        div { class: "settings-card",
            div { class: "settings-card__header",
                h3 { "阅读外观" }
            }
            ReadingPreferencesSection { draft: session.draft() }
            ThemeSettingsSections {
                theme: session.theme(),
                draft: session.draft(),
                preset_choice: session.preset_choice(),
                status: session.status_signal(),
                status_tone: session.status_tone_signal(),
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
