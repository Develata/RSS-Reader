use dioxus::prelude::*;
use rssr_domain::UserSettings;

use super::themes::detect_preset_key;
use crate::{bootstrap::AppServices, status::set_status_error, theme::ThemeController};

const REPOSITORY_URL: &str = "https://github.com/Develata/RSS-Reader";

#[derive(Clone, Copy)]
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

    pub(crate) fn theme(self) -> ThemeController {
        self.theme
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

    pub(crate) async fn load(mut self) {
        match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(settings) => {
                    self.preset_choice.set(detect_preset_key(&settings.custom_css).to_string());
                    self.draft.set(settings);
                }
                Err(err) => {
                    set_status_error(self.status, self.status_tone, format!("读取设置失败：{err}"))
                }
            },
            Err(err) => {
                set_status_error(self.status, self.status_tone, format!("初始化应用失败：{err}"))
            }
        }
    }

    pub(crate) fn open_repository(self) {
        if let Err(err) = open_repository_url() {
            set_status_error(self.status, self.status_tone, format!("打开 GitHub 仓库失败：{err}"));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn open_repository_url() -> Result<(), String> {
    webbrowser::open(REPOSITORY_URL).map(|_| ()).map_err(|err| err.to_string())
}

#[cfg(target_arch = "wasm32")]
fn open_repository_url() -> Result<(), String> {
    web_sys::window()
        .ok_or_else(|| "浏览器窗口不可用".to_string())?
        .open_with_url_and_target(REPOSITORY_URL, "_blank")
        .map(|_| ())
        .map_err(|err| format!("{err:?}"))
}
