use dioxus::prelude::*;
use rssr_domain::{ListDensity, UserSettings};

#[derive(Clone, Copy, PartialEq)]
pub struct ThemeController {
    pub settings: Signal<UserSettings>,
    /// How many times settings the library stored were applied (read or written). A read issued
    /// before a later application does not overwrite it (C4b item 2; design review C4D-R2 R2-03).
    stored: Signal<u64>,
}

/// A controller with the default settings, none from the library yet.
pub fn use_theme_controller() -> ThemeController {
    ThemeController { settings: use_signal(UserSettings::default), stored: use_signal(|| 0) }
}

impl ThemeController {
    /// Applies settings the library stored (read, saved, imported or pulled).
    pub fn store(mut self, settings: UserSettings) {
        *self.stored.write() += 1;
        self.settings.set(settings);
    }

    /// How many times settings from the library were applied so far.
    pub fn stored_count(self) -> u64 {
        *self.stored.peek()
    }
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
