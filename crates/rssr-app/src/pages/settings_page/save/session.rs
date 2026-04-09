use dioxus::prelude::*;

use crate::{
    pages::settings_page::{
        session::SettingsPageSession,
        themes::{detect_preset_key, validate_custom_css},
    },
    status::{set_status_error, set_status_info},
};

use super::{
    effect::SettingsPageSaveEffect, runtime::execute_settings_page_save_effect,
    state::SettingsPageSaveState,
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SettingsPageSaveSession {
    state: Signal<SettingsPageSaveState>,
    page: SettingsPageSession,
}

impl SettingsPageSaveSession {
    pub(crate) fn new(state: Signal<SettingsPageSaveState>, page: SettingsPageSession) -> Self {
        Self { state, page }
    }

    pub(crate) fn snapshot(self) -> SettingsPageSaveState {
        (self.state)()
    }

    pub(crate) fn save(self) {
        self.save_with_message("设置已保存。");
    }

    pub(crate) fn save_with_message(self, success_message: impl Into<String>) {
        let next = (self.page.draft())();
        if let Err(err) = validate_custom_css(&next.custom_css) {
            set_status_error(
                self.page.status_signal(),
                self.page.status_tone_signal(),
                format!("自定义 CSS 格式无效：{err}"),
            );
            return;
        }

        let previous = (self.page.theme().settings)();
        let previous_preset = detect_preset_key(&previous.custom_css).to_string();
        let mut state = self.state;
        state.with_mut(|state| state.pending_save = true);

        let mut theme = self.page.theme();
        let mut draft = self.page.draft();
        let mut preset_choice = self.page.preset_choice();
        let status = self.page.status_signal();
        let status_tone = self.page.status_tone_signal();
        let success_message = success_message.into();

        spawn(async move {
            let outcome =
                execute_settings_page_save_effect(SettingsPageSaveEffect::SaveAppearance {
                    settings: next.clone(),
                    success_message,
                })
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
