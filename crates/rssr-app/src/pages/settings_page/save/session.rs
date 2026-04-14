use dioxus::prelude::*;
use rssr_domain::DEFAULT_ENTRIES_PAGE_SIZE;

use super::state::SettingsPageSaveState;
use crate::{
    pages::settings_page::{
        intent::SettingsPageIntent,
        session::SettingsPageSession,
        themes::{detect_preset_key, validate_custom_css},
    },
    status::{set_status_error, set_status_info},
    ui::{SettingsCommand, UiCommand, UiIntent, apply_projected_ui_intents, spawn_ui_command},
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
        let mut next = (self.page.draft())();
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

        let status = self.page.status_signal();
        let status_tone = self.page.status_tone_signal();
        let mut success_message = success_message.into();
        if next.entries_page_size == 0 {
            next.entries_page_size = DEFAULT_ENTRIES_PAGE_SIZE;
            success_message.push_str(&format!(
                " 文章页每页数量输入为 0，已回退为默认值 {DEFAULT_ENTRIES_PAGE_SIZE}。"
            ));
        }

        spawn_ui_command(
            UiCommand::Settings(SettingsCommand::SaveAppearance {
                settings: next.clone(),
                success_message,
            }),
            move |intents| {
                state.with_mut(|state| state.pending_save = false);
                let mut saved_settings = None;
                let mut status_message = String::new();
                apply_projected_ui_intents(
                    intents.clone(),
                    UiIntent::into_settings_page_intent,
                    |intent| match intent {
                        SettingsPageIntent::SettingsLoaded(settings) => {
                            saved_settings = Some(settings);
                        }
                        SettingsPageIntent::SetStatus { message, .. } => {
                            status_message = message;
                        }
                    },
                );
                for intent in intents {
                    if let Some((message, _)) = intent.into_status() {
                        status_message = message;
                    }
                }
                if let Some(saved_settings) = saved_settings {
                    self.page.apply_loaded_settings(saved_settings);
                    set_status_info(status, status_tone, status_message);
                } else {
                    self.page.restore_settings(previous, previous_preset);
                    set_status_error(status, status_tone, status_message);
                }
            },
        );
    }
}
