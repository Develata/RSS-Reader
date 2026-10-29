use dioxus::prelude::*;
use rssr_domain::UserSettings;
use std::future::Future;

use super::{
    browser::open_repository_url,
    intent::SettingsPageIntent,
    request_lifecycle::{EditingRevision, SettingsRequestLifecycle, SettingsWrite, WriteRequest},
    save_policy::saved_draft_update,
    themes::detect_preset_key,
};
use crate::{
    status::{set_status_error, set_status_info},
    theme::ThemeController,
    ui::{SettingsCommand, UiCommand, UiIntent, execute_ui_command},
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
    requests: Signal<SettingsRequestLifecycle>,
}

impl SettingsPageSession {
    pub(crate) fn new(theme: ThemeController) -> Self {
        let draft = use_signal(|| (theme.settings)());
        let preset_choice =
            use_signal(|| detect_preset_key(&(theme.settings)().custom_css).to_string());
        let status = use_signal(String::new);
        let status_tone = use_signal(|| "info".to_string());
        let theme_import_revision = use_signal(|| 0);
        let requests = use_signal(SettingsRequestLifecycle::default);

        Self { theme, draft, preset_choice, status, status_tone, theme_import_revision, requests }
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
        self.note_draft_edit();
        let mut next = (self.draft)();
        update(&mut next);
        self.draft.set(next);
    }

    pub(crate) fn set_preset_choice(mut self, value: impl Into<String>) {
        self.invalidate_theme_import();
        self.requests.with_mut(SettingsRequestLifecycle::edit_preset);
        self.preset_choice.set(value.into());
    }

    pub(crate) fn note_draft_edit(mut self) {
        self.invalidate_theme_import();
        self.requests.with_mut(SettingsRequestLifecycle::edit_draft);
    }

    pub(crate) fn draft(self) -> Signal<UserSettings> {
        self.draft
    }

    pub(crate) fn draft_replacement_revision(self) -> u64 {
        (self.requests)().draft_replacement_revision()
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
        self.load_with(|command| execute_ui_command(UiCommand::Settings(command)));
    }

    fn load_with<F: Future<Output = Vec<UiIntent>> + 'static>(
        mut self,
        execute: impl FnOnce(SettingsCommand) -> F,
    ) {
        let Some(request) = self.requests.with_mut(SettingsRequestLifecycle::begin_load) else {
            return;
        };
        let loading = execute(SettingsCommand::Load);
        spawn(async move {
            let intents = loading.await;
            if self.requests.with_mut(|state| state.complete_load(request)) {
                // Initial loading never discards an edit made since this page was mounted,
                // including one made before its load effect got to run.
                self.apply_load_result(intents, EditingRevision::default());
            }
        });
    }

    pub(super) fn pending_write(self) -> Option<SettingsWrite> {
        (self.requests)().pending()
    }

    pub(super) fn reject_pending_write(self) -> bool {
        let Some(pending) = self.requests.peek().pending() else {
            return false;
        };
        self.set_status(
            match pending {
                SettingsWrite::Save => {
                    "上一份设置仍在保存；新修改已保留在草稿中，完成后请再次操作。"
                }
                SettingsWrite::Download => "配置仍在下载；新修改已保留在草稿中，完成后请再次操作。",
            },
            "info",
        );
        true
    }

    pub(super) fn begin_write(mut self, kind: SettingsWrite) -> Option<WriteRequest> {
        self.requests.with_mut(|state| state.begin_write(kind))
    }

    pub(super) fn complete_write(mut self, request: WriteRequest) -> bool {
        self.requests.with_mut(|state| state.complete_write(request))
    }

    pub(super) fn apply_load_result(self, intents: Vec<UiIntent>, editing: EditingRevision) {
        let mut settings = None;
        let mut status = None;
        for intent in intents.into_iter().filter_map(UiIntent::into_settings_page_intent) {
            match intent {
                SettingsPageIntent::SettingsLoaded(value) => settings = Some(value),
                SettingsPageIntent::SetStatus { message, tone } => status = Some((message, tone)),
            }
        }
        let unsaved = settings.is_some_and(|value| self.apply_loaded_settings(value, editing));
        if unsaved {
            let (mut message, tone) =
                status.unwrap_or_else(|| ("设置已载入。".into(), "info".into()));
            message.push_str(" 加载期间的修改仍保留在草稿中，尚未保存。");
            self.set_status(message, tone);
        } else if let Some((message, tone)) = status {
            self.set_status(message, tone);
        }
    }

    fn apply_loaded_settings(mut self, settings: UserSettings, editing: EditingRevision) -> bool {
        let current = self.requests.peek().editing();
        let replace_draft = current.draft == editing.draft;
        let preset = (current.preset == editing.preset).then(|| {
            if replace_draft {
                detect_preset_key(&settings.custom_css)
            } else {
                detect_preset_key(&self.draft.peek().custom_css)
            }
        });
        if (replace_draft && *self.draft.peek() != settings)
            || preset.is_some_and(|value| *self.preset_choice.peek() != value)
        {
            self.invalidate_theme_import();
        }
        if let Some(preset) = preset {
            self.preset_choice.set(preset.to_string());
        }
        if replace_draft {
            self.draft.set(settings.clone());
            self.requests.with_mut(SettingsRequestLifecycle::note_draft_replacement);
        }
        let unsaved = *self.draft.peek() != settings;
        self.theme.settings.set(settings);
        unsaved
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
            self.requests.with_mut(SettingsRequestLifecycle::note_draft_replacement);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

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

    fn start_load(session: SettingsPageSession) -> oneshot::Sender<Vec<UiIntent>> {
        let (sender, receiver) = oneshot::channel();
        session.load_with(|command| {
            assert!(matches!(command, SettingsCommand::Load));
            async move { receiver.await.unwrap() }
        });
        sender
    }

    fn loaded(css: &str) -> Vec<UiIntent> {
        vec![UiIntent::SettingsPage(SettingsPageIntent::SettingsLoaded(UserSettings {
            custom_css: css.into(),
            ..Default::default()
        }))]
    }

    #[test]
    fn accepted_backfills_retire_feedback_even_for_equal_values_and_round_trips() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            let original = UserSettings::default();
            let invalid_revision = session.draft_replacement_revision();
            let import = session.begin_theme_import();
            session.apply_loaded_settings(original.clone(), EditingRevision::default());
            assert_ne!(session.draft_replacement_revision(), invalid_revision);
            assert!(
                session.complete_theme_import(import),
                "equal backfill is not a new import intent"
            );

            let replaced = session.draft_replacement_revision();
            let changed = UserSettings {
                refresh_interval_minutes: 60,
                archive_after_months: 6,
                entries_page_size: 50,
                reader_font_scale: 1.2,
                ..original.clone()
            };
            session.apply_loaded_settings(changed, EditingRevision::default());
            session.apply_loaded_settings(original.clone(), EditingRevision::default());
            assert_ne!(session.draft_replacement_revision(), replaced);
            assert_ne!(session.draft_replacement_revision(), invalid_revision);
            assert_eq!(*session.draft.peek(), original);
            assert_eq!(session.requests.peek().editing(), EditingRevision::default());
        });
    }

    #[test]
    fn later_unparsed_edits_keep_feedback_when_initial_or_download_backfill_is_rejected() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            let invalid_revision = session.draft_replacement_revision();
            session.note_draft_edit();
            session.apply_loaded_settings(UserSettings::default(), EditingRevision::default());
            assert_eq!(session.draft_replacement_revision(), invalid_revision);

            let request = session.begin_write(SettingsWrite::Download).unwrap();
            session.note_draft_edit();
            let changed = UserSettings { entries_page_size: 50, ..Default::default() };
            assert!(session.complete_write(request));
            session.apply_loaded_settings(changed.clone(), request.editing);
            assert_eq!(session.draft_replacement_revision(), invalid_revision);
            assert_eq!(*session.draft.peek(), UserSettings::default());
            assert_eq!(*session.theme.settings.peek(), changed);
        });
    }

    #[test]
    fn save_feedback_replacement_keeps_value_equality_instead_of_edit_revision() {
        let (dom, session) = setup();
        dom.in_scope(ScopeId::APP, || {
            session.update_draft(|draft| draft.entries_page_size = 0);
            let submitted = session.draft.peek().clone();
            let saved = UserSettings::default();
            let first_revision = session.draft_replacement_revision();
            let editing = session.requests.peek().editing();
            session.apply_saved_settings(saved.clone(), &submitted, "none");
            let normalized_revision = session.draft_replacement_revision();
            assert_ne!(normalized_revision, first_revision);
            assert_eq!(session.requests.peek().editing(), editing);

            session.update_draft(|draft| draft.entries_page_size = 51);
            session.apply_saved_settings(saved.clone(), &submitted, "none");
            assert_eq!(session.draft_replacement_revision(), normalized_revision);
            session.update_draft(|draft| draft.entries_page_size = 0);
            session.note_draft_edit(); // A later parse failure still retains the same submitted value.
            let editing = session.requests.peek().editing();
            session.apply_saved_settings(saved.clone(), &submitted, "none");
            assert_ne!(session.draft_replacement_revision(), normalized_revision);
            assert_eq!(session.requests.peek().editing(), editing);
            assert_eq!(*session.draft.peek(), saved);
        });
    }

    #[test]
    fn initial_load_preserves_edits_before_or_after_start_including_same_value_events() {
        for before_start in [false, true] {
            for edit_back in [false, true] {
                let (mut dom, session) = setup();
                let edit = || {
                    session.update_draft(|draft| draft.custom_css = "/* later */".into());
                    if edit_back {
                        session.update_draft(|draft| draft.custom_css.clear());
                    }
                    session.set_preset_choice("newsprint");
                    session.set_preset_choice("none");
                };
                let sender = dom.in_scope(ScopeId::APP, || {
                    if before_start {
                        edit();
                    }
                    let sender = start_load(session);
                    if !before_start {
                        edit();
                    }
                    sender
                });
                sender.send(loaded("/* persisted */")).unwrap();
                dom.render_immediate_to_vec();
                dom.in_scope(ScopeId::APP, || {
                    assert_eq!(
                        session.draft.peek().custom_css,
                        if edit_back { "" } else { "/* later */" }
                    );
                    assert_eq!(*session.preset_choice.peek(), "none");
                    assert_eq!(session.theme.settings.peek().custom_css, "/* persisted */");
                    assert!(session.status().contains("尚未保存"));
                });
            }
        }
    }

    #[test]
    fn unparsed_edit_preserves_last_valid_draft_without_blocking_loaded_theme() {
        let (mut dom, session) = setup();
        let sender = dom.in_scope(ScopeId::APP, || {
            let sender = start_load(session);
            session.note_draft_edit();
            sender
        });
        sender.send(loaded("/* persisted */")).unwrap();
        dom.render_immediate_to_vec();
        assert_eq!(*session.draft.peek(), UserSettings::default());
        assert_eq!(session.theme.settings.peek().custom_css, "/* persisted */");
    }

    #[test]
    fn latest_load_accepts_persisted_values_and_rejects_old_success_and_error() {
        for fail in [false, true] {
            let (mut dom, session) = setup();
            let (old, new) =
                dom.in_scope(ScopeId::APP, || (start_load(session), start_load(session)));
            new.send(loaded("/* latest */")).unwrap();
            dom.render_immediate_to_vec();
            old.send(if fail {
                vec![UiIntent::SetStatus { message: "old error".into(), tone: "error".into() }]
            } else {
                loaded("/* obsolete */")
            })
            .unwrap();
            dom.render_immediate_to_vec();
            assert_eq!(session.draft.peek().custom_css, "/* latest */");
            assert_eq!(*session.preset_choice.peek(), "custom");
            assert_eq!(session.theme.settings.peek().custom_css, "/* latest */");
            assert!(session.status.peek().is_empty());
        }
    }

    #[test]
    fn admitted_write_supersedes_initial_load_even_after_failure_or_no_result() {
        for kind in [SettingsWrite::Save, SettingsWrite::Download] {
            let (mut dom, session) = setup();
            let sender = dom.in_scope(ScopeId::APP, || {
                let sender = start_load(session);
                let write = session.begin_write(kind).unwrap();
                session.load_with(|_| -> std::future::Ready<Vec<UiIntent>> {
                    panic!("initial read cannot start during a settings write")
                });
                assert!(session.complete_write(write));
                session.set_status("write failed; draft kept", "error");
                sender
            });
            sender.send(loaded("/* obsolete */")).unwrap();
            dom.render_immediate_to_vec();
            assert_eq!(*session.draft.peek(), UserSettings::default());
            assert_eq!(*session.theme.settings.peek(), UserSettings::default());
            assert_eq!(*session.status.peek(), "write failed; draft kept");
        }
    }

    #[test]
    fn current_load_error_keeps_edits_and_disposal_drops_the_pending_read() {
        let (mut dom, session) = setup();
        let sender = dom.in_scope(ScopeId::APP, || {
            let sender = start_load(session);
            session.update_draft(|draft| draft.custom_css = "/* draft */".into());
            sender
        });
        sender
            .send(vec![UiIntent::SetStatus { message: "load failed".into(), tone: "error".into() }])
            .unwrap();
        dom.render_immediate_to_vec();
        assert_eq!(session.draft.peek().custom_css, "/* draft */");
        assert_eq!(*session.status.peek(), "load failed");
        assert_eq!(*session.status_tone.peek(), "error");
        let sender = dom.in_scope(ScopeId::APP, || start_load(session));
        dom.render_immediate_to_vec();
        assert!(!sender.is_closed());
        drop(dom);
        assert!(sender.is_closed());
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
            session.apply_loaded_settings(UserSettings::default(), EditingRevision::default());
            assert!(session.is_theme_import_current(request));
            let changed = UserSettings { custom_css: "/* loaded */".into(), ..Default::default() };
            session.apply_loaded_settings(changed.clone(), EditingRevision::default());
            assert!(!session.complete_theme_import(request));

            // A loaded preset reset also changes the active editing context.
            session.set_preset_choice("newsprint");
            let request = session.begin_theme_import();
            let editing = session.requests.peek().editing();
            session.apply_loaded_settings(changed, editing);
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
