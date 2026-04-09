use dioxus::prelude::*;
use rssr_domain::{ListDensity, UserSettings};

#[derive(Clone, Copy, PartialEq)]
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

pub fn density_state(density: ListDensity) -> &'static str {
    match density {
        ListDensity::Comfortable => "comfortable",
        ListDensity::Compact => "compact",
    }
}
