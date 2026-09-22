use dioxus::prelude::*;
use rssr_domain::UserSettings;

use super::{browser::open_repository_url, intent::SettingsPageIntent, themes::detect_preset_key};
use crate::{
    status::{set_status_error, set_status_info},
    theme::ThemeController,
    ui::{SettingsCommand, UiCommand, UiIntent, spawn_projected_ui_command},
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SettingsPageSession {
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
}

impl SettingsPageSession {
    pub(crate) fn new(theme: ThemeController) -> Self {
        let draft = use_signal(|| (theme.settings)());
        let preset_choice =
            use_signal(|| detect_preset_key(&(theme.settings)().custom_css).to_string());
        let status = use_signal(String::new);
        let status_tone = use_signal(|| "info".to_string());

        Self { theme, draft, preset_choice, status, status_tone }
    }

    pub(crate) fn draft(self) -> Signal<UserSettings> {
        self.draft
    }

    pub(crate) fn preset_choice(self) -> Signal<String> {
        self.preset_choice
    }

    pub(crate) fn status_signal(self) -> Signal<String> {
        self.status
    }

    pub(crate) fn status_tone_signal(self) -> Signal<String> {
        self.status_tone
    }

    pub(crate) fn status(self) -> String {
        (self.status)()
    }

    pub(crate) fn status_tone(self) -> String {
        (self.status_tone)()
    }

    pub(crate) fn load(self) {
        self.spawn_ui_command(UiCommand::Settings(SettingsCommand::Load));
    }

    pub(crate) fn dispatch(self, intent: SettingsPageIntent) {
        match intent {
            SettingsPageIntent::SettingsLoaded(settings) => self.apply_loaded_settings(settings),
            SettingsPageIntent::SetStatus { message, tone } => self.set_status(message, tone),
        }
    }

    pub(crate) fn apply_loaded_settings(mut self, settings: UserSettings) {
        self.preset_choice.set(detect_preset_key(&settings.custom_css).to_string());
        self.draft.set(settings.clone());
        self.theme.settings.set(settings);
    }

    /// Persisted settings and editable input are different snapshots while a save is pending.
    /// Keep subsequent edits, including a newly selected preset, for the next explicit save.
    pub(crate) fn apply_saved_settings(
        mut self,
        settings: UserSettings,
        submitted_draft: &UserSettings,
        submitted_preset: &str,
    ) -> bool {
        if *self.draft.peek() == *submitted_draft {
            self.draft.set(settings.clone());
        }
        if self.preset_choice.peek().as_str() == submitted_preset {
            self.preset_choice.set(detect_preset_key(&self.draft.peek().custom_css).to_string());
        }
        let has_unsaved_edits = *self.draft.peek() != settings;
        self.theme.settings.set(settings);
        has_unsaved_edits
    }

    pub(crate) fn set_status(self, message: impl Into<String>, tone: impl Into<String>) {
        let message = message.into();
        let tone = tone.into();
        if tone == "error" {
            set_status_error(self.status, self.status_tone, message);
        } else {
            set_status_info(self.status, self.status_tone, message);
        }
    }

    pub(crate) fn open_repository(self) {
        if let Err(err) = open_repository_url() {
            self.set_status(format!("打开 GitHub 仓库失败：{err}"), "error");
        }
    }

    fn spawn_ui_command(self, command: UiCommand) {
        spawn_projected_ui_command(command, UiIntent::into_settings_page_intent, move |intent| {
            self.dispatch(intent);
        });
    }
}
