use dioxus::prelude::*;
use rssr_domain::{ListDensity, ThemeMode, UserSettings};

/// Presentation only. A settings-page preview never becomes the library's settings.
#[derive(Clone, PartialEq)]
pub(crate) struct ThemeAppearance {
    pub theme: ThemeMode,
    pub list_density: ListDensity,
    pub reader_font_scale: f32,
    pub custom_css: String,
}

impl From<&UserSettings> for ThemeAppearance {
    fn from(settings: &UserSettings) -> Self {
        Self {
            theme: settings.theme,
            list_density: settings.list_density,
            reader_font_scale: settings.reader_font_scale,
            custom_css: settings.custom_css.clone(),
        }
    }
}

#[derive(Clone, Default, PartialEq)]
struct Preview {
    owner: u64,
    appearance: Option<ThemeAppearance>,
}

#[derive(Clone, Copy, PartialEq)]
pub struct ThemeController {
    pub settings: Signal<UserSettings>,
    /// How many times settings the library stored were applied (read or written). A read issued
    /// before a later application does not overwrite it (C4b item 2; design review C4D-R2 R2-03).
    stored: Signal<u64>,
    preview: Signal<Preview>,
}

/// A controller with the default settings, none from the library yet.
pub fn use_theme_controller() -> ThemeController {
    ThemeController {
        settings: use_signal(UserSettings::default),
        stored: use_signal(|| 0),
        preview: use_signal(Preview::default),
    }
}

impl ThemeController {
    pub(crate) fn appearance(self) -> ThemeAppearance {
        self.preview
            .read()
            .appearance
            .clone()
            .unwrap_or_else(|| ThemeAppearance::from(&*self.settings.read()))
    }

    /// A page owns its preview until it leaves. A late cleanup from an older page must not
    /// clear the preview of a newly mounted settings page.
    pub(crate) fn begin_preview(mut self) -> u64 {
        let mut preview = self.preview.write();
        preview.owner = preview.owner.wrapping_add(1);
        preview.appearance = None;
        preview.owner
    }

    pub(crate) fn preview(mut self, owner: u64, appearance: ThemeAppearance) {
        if self.preview.peek().owner == owner
            && self.preview.peek().appearance.as_ref() != Some(&appearance)
        {
            self.preview.write().appearance = Some(appearance);
        }
    }

    pub(crate) fn end_preview(mut self, owner: u64) {
        if self.preview.peek().owner == owner {
            let mut preview = self.preview.write();
            preview.owner = preview.owner.wrapping_add(1);
            preview.appearance = None;
        }
    }

    /// Applies settings the library stored (read, saved, imported or pulled).
    pub fn store(mut self, settings: UserSettings) {
        *self.stored.write() += 1;
        self.settings.set(settings);
    }

    /// How many times settings from the library were applied so far.
    pub fn stored_count(self) -> u64 {
        *self.stored.peek()
    }

    /// How many times settings from the library were applied (reactive: a component reading it
    /// follows).
    pub fn stored_revision(self) -> u64 {
        *self.stored.read()
    }

    /// Whether settings from the library were applied (reactive: a component reading it
    /// follows).
    pub fn has_stored(self) -> bool {
        *self.stored.read() > 0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_page_cannot_replace_or_clear_a_newer_preview() {
        fn harness() -> Element {
            let theme = use_theme_controller();
            use_context_provider(|| theme);
            rsx! {}
        }

        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        dom.in_scope(ScopeId::APP, || {
            let theme = consume_context::<ThemeController>();
            let old = theme.begin_preview();
            let current = theme.begin_preview();
            let dark = UserSettings { theme: ThemeMode::Dark, ..UserSettings::default() };
            theme.preview(current, ThemeAppearance::from(&dark));
            theme.preview(old, ThemeAppearance::from(&UserSettings::default()));
            theme.end_preview(old);
            assert_eq!(theme.appearance().theme, ThemeMode::Dark);
            assert_eq!(theme.stored_count(), 0);
            theme.end_preview(current);
            theme.preview(current, ThemeAppearance::from(&dark));
            assert_eq!(theme.appearance().theme, UserSettings::default().theme);
        });
    }
}
