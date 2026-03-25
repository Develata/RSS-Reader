use dioxus::prelude::*;
use rssr_domain::UserSettings;

#[derive(Clone, Copy)]
pub struct ThemeController {
    pub settings: Signal<UserSettings>,
}

pub fn theme_class(mode: rssr_domain::ThemeMode) -> &'static str {
    match mode {
        rssr_domain::ThemeMode::Light => "theme-light",
        rssr_domain::ThemeMode::Dark => "theme-dark",
        rssr_domain::ThemeMode::System => "theme-system",
    }
}
