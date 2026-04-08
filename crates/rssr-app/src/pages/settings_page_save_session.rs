use dioxus::prelude::*;
use rssr_domain::UserSettings;

use crate::{
    status::{set_status_error, set_status_info},
    theme::ThemeController,
};

use super::{
    settings_page_save_effect::SettingsPageSaveEffect,
    settings_page_save_runtime::execute_settings_page_save_effect,
    settings_page_save_state::SettingsPageSaveState, settings_page_themes::detect_preset_key,
    settings_page_themes::validate_custom_css,
};

#[derive(Clone, Copy)]
pub(crate) struct SettingsPageSaveSession {
    state: Signal<SettingsPageSaveState>,
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
}

impl SettingsPageSaveSession {
    pub(crate) fn new(
        state: Signal<SettingsPageSaveState>,
        theme: ThemeController,
        draft: Signal<UserSettings>,
        preset_choice: Signal<String>,
        status: Signal<String>,
        status_tone: Signal<String>,
    ) -> Self {
        Self { state, theme, draft, preset_choice, status, status_tone }
    }

    pub(crate) fn snapshot(self) -> SettingsPageSaveState {
        (self.state)()
    }

    pub(crate) fn save(self) {
        let next = (self.draft)();
        if let Err(err) = validate_custom_css(&next.custom_css) {
            set_status_error(self.status, self.status_tone, format!("自定义 CSS 格式无效：{err}"));
            return;
        }

        let previous = (self.theme.settings)();
        let previous_preset = detect_preset_key(&previous.custom_css).to_string();
        let mut state = self.state;
        state.with_mut(|state| state.pending_save = true);

        let mut theme = self.theme;
        let mut draft = self.draft;
        let mut preset_choice = self.preset_choice;
        let status = self.status;
        let status_tone = self.status_tone;

        spawn(async move {
            let outcome = execute_settings_page_save_effect(
                SettingsPageSaveEffect::SaveAppearance(next.clone()),
            )
            .await;

            state.with_mut(|state| state.pending_save = false);
            if let Some(saved_settings) = outcome.saved_settings {
                theme.settings.set(saved_settings);
                set_status_info(status, status_tone, outcome.status_message);
            } else {
                theme.settings.set(previous.clone());
                draft.set(previous);
                preset_choice.set(previous_preset);
                set_status_error(status, status_tone, outcome.status_message);
            }
        });
    }
}
