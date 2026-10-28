use dioxus::prelude::*;
use rssr_domain::UserSettings;

use super::{
    browser::open_repository_url, intent::SettingsPageIntent, save_policy::saved_draft_update,
    themes::detect_preset_key,
};
use crate::{
    status::{set_status_error, set_status_info},
    theme::ThemeController,
    ui::{SettingsCommand, UiCommand, UiIntent, spawn_projected_ui_command},
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ThemeImportRequest(u64);

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SettingsPageSession {
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
    theme_import_revision: Signal<u64>,
}

impl SettingsPageSession {
    pub(crate) fn new(theme: ThemeController) -> Self {
        let draft = use_signal(|| (theme.settings)());
        let preset_choice =
            use_signal(|| detect_preset_key(&(theme.settings)().custom_css).to_string());
        let status = use_signal(String::new);
        let status_tone = use_signal(|| "info".to_string());
        let theme_import_revision = use_signal(|| 0);

        Self { theme, draft, preset_choice, status, status_tone, theme_import_revision }
    }

    /// Start at picker opening, before any await. Even a later cancellation supersedes old reads.
    pub(crate) fn begin_theme_import(mut self) -> ThemeImportRequest {
        self.theme_import_revision.with_mut(|revision| {
            *revision = revision.wrapping_add(1);
            ThemeImportRequest(*revision)
        })
    }

    pub(crate) fn invalidate_theme_import(self) {
        self.begin_theme_import();
    }

    pub(crate) fn is_theme_import_current(self, request: ThemeImportRequest) -> bool {
        *self.theme_import_revision.peek() == request.0
    }

    /// Consume admission before validation, status publication, draft changes or persistence.
    pub(crate) fn complete_theme_import(mut self, request: ThemeImportRequest) -> bool {
        self.theme_import_revision.with_mut(|revision| {
            if *revision != request.0 {
                return false;
            }
            *revision = revision.wrapping_add(1);
            true
        })
    }

    pub(crate) fn update_draft(mut self, update: impl FnOnce(&mut UserSettings)) {
        self.invalidate_theme_import();
        let mut next = (self.draft)();
        update(&mut next);
        self.draft.set(next);
    }

    pub(crate) fn set_preset_choice(mut self, value: impl Into<String>) {
        self.invalidate_theme_import();
        self.preset_choice.set(value.into());
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
        let preset = detect_preset_key(&settings.custom_css);
        if *self.draft.peek() != settings || *self.preset_choice.peek() != preset {
            self.invalidate_theme_import();
        }
        self.preset_choice.set(preset.to_string());
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
        let update = saved_draft_update(
            &settings,
            submitted_draft,
            submitted_preset,
            &self.draft.peek(),
            &self.preset_choice.peek(),
        );
        // A prior save returning identical values is not a newer editing intent.
        // Actual normalization/replacement must still invalidate outstanding file reads.
        if (update.replace_draft && *self.draft.peek() != settings)
            || update.preset_choice.is_some_and(|preset| *self.preset_choice.peek() != preset)
        {
            self.invalidate_theme_import();
        }
        if update.replace_draft {
            self.draft.set(settings.clone());
        }
        if let Some(preset) = update.preset_choice {
            self.preset_choice.set(preset.to_string());
        }
        self.theme.settings.set(settings);
        update.has_unsaved_edits
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

#[cfg(test)]
mod tests {
    use super::*;

    fn harness() -> Element {
        let theme = ThemeController { settings: use_signal(UserSettings::default) };
        let session = SettingsPageSession::new(theme);
        use_context_provider(|| session);
        rsx! {}
    }

    fn setup() -> (VirtualDom, SettingsPageSession) {
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let session = dom.in_scope(ScopeId::APP, consume_context);
        (dom, session)
    }

    #[test]
    fn only_the_latest_import_can_be_completed_once() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            let old = session.begin_theme_import();
            let new = session.begin_theme_import();
            assert!(!session.complete_theme_import(old));
            assert!(session.complete_theme_import(new));
            assert!(!session.complete_theme_import(old));
            assert!(!session.complete_theme_import(new));
        });
    }

    #[test]
    fn edits_and_preset_events_invalidate_even_after_values_return_to_the_original() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            let request = session.begin_theme_import();
            session.update_draft(|draft| draft.custom_css = "/* changed */".into());
            session.update_draft(|draft| draft.custom_css.clear());
            assert!(!session.complete_theme_import(request));
            let same_value = session.begin_theme_import();
            session.update_draft(|_| {});
            assert!(!session.complete_theme_import(same_value));
            let preset = session.begin_theme_import();
            session.set_preset_choice("newsprint");
            session.set_preset_choice("none");
            assert!(!session.complete_theme_import(preset));
        });
    }

    #[test]
    fn only_actual_loaded_settings_changes_supersede_a_pending_import() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            let request = session.begin_theme_import();
            session.set_status("feedback only", "info");
            assert!(session.is_theme_import_current(request));
            session.apply_loaded_settings(UserSettings::default());
            assert!(session.is_theme_import_current(request));
            let changed = UserSettings { custom_css: "/* loaded */".into(), ..Default::default() };
            session.apply_loaded_settings(changed.clone());
            assert!(!session.complete_theme_import(request));

            // A loaded preset reset also changes the active editing context.
            session.set_preset_choice("newsprint");
            let request = session.begin_theme_import();
            session.apply_loaded_settings(changed);
            assert!(!session.complete_theme_import(request));
        });
    }

    #[test]
    fn identical_prior_save_completion_preserves_a_newer_import() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            let submitted = session.draft.peek().clone();
            let request = session.begin_theme_import();
            session.apply_saved_settings(submitted.clone(), &submitted, "none");
            assert!(session.complete_theme_import(request));
        });
    }

    #[test]
    fn actual_save_normalization_invalidates_a_pending_import() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            session.update_draft(|draft| draft.entries_page_size = 0);
            let submitted = session.draft.peek().clone();
            let request = session.begin_theme_import();
            session.apply_saved_settings(UserSettings::default(), &submitted, "none");
            assert!(!session.complete_theme_import(request));
        });
    }

    #[test]
    fn save_completion_that_preserves_later_edits_also_preserves_their_import() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            let submitted = session.draft.peek().clone();
            session.update_draft(|draft| draft.custom_css = "/* later draft */".into());
            session.set_preset_choice("custom");
            let request = session.begin_theme_import();
            session.apply_saved_settings(submitted.clone(), &submitted, "none");
            assert!(session.complete_theme_import(request));
            assert_eq!(session.draft.peek().custom_css, "/* later draft */");
        });
    }
}
