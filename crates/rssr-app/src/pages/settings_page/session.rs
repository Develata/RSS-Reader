use super::{
    draft_edits::Edits,
    draft_projection::{DraftProjection, ProjectionSource, project_stored},
    request_lifecycle::{
        LoadProjection, LoadedSettings, SettingsLoadRequest, SettingsRequestLifecycle,
    },
};
use std::future::Future;

use dioxus::prelude::*;
use rssr_application::SettingsPatch;
use rssr_domain::UserSettings;

use super::{browser::open_repository_url, intent::SettingsPageIntent, themes::detect_preset_key};
use crate::{
    theme::{ThemeAppearance, ThemeController},
    ui::{
        AppShellState, SettingsCommand, UiCommand, UiIntent, apply_projected_ui_intents, drafts,
        run_ui_command,
    },
};

/// The settings page shows the stored settings with the user's edits on top (B2-D4).
///
/// Edits are tracked, not inferred: every field the form sets through [`Self::edit`] is an edit
/// with its latest value until a save stores it, even when it is set back to the value it had.
/// A save sends exactly these fields; a reply carrying stored settings replaces the rest. The
/// stored settings are the page's own: the app-wide [`ThemeController`] signal, which startup
/// and other pages also set whenever their replies arrive, only renders the settings.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SettingsPageSession {
    theme: ThemeController,
    theme_import_revision: Signal<u64>,
    numeric_revisions: Signal<[u64; 4]>,
    preview_owner: u64,
    /// Explicit CSS preview, or the current stored CSS when none was requested on this page.
    /// Typing and restoring an editor draft never implicitly apply that draft.
    preview_css: Signal<Option<String>>,
    stored: Signal<UserSettings>,
    edits: Signal<Edits>,
    /// `stored` with the edits on top; what the form shows.
    draft: Signal<UserSettings>,
    requests: Signal<SettingsRequestLifecycle>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
    /// The app's shell, which keeps the edits not stored yet across a reload or a sign-in
    /// (C4b item 4); none in tests of the page alone.
    shell: Option<AppShellState>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ThemeImportRequest(u64);

impl SettingsPageSession {
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

    pub(crate) fn set_preset_choice(mut self, value: impl Into<String>) {
        self.invalidate_theme_import();
        self.preset_choice.set(value.into());
    }

    pub(crate) fn numeric_revisions(self) -> [u64; 4] {
        (self.numeric_revisions)()
    }

    /// A draft kept before a reload or a sign-in becomes the pending edits, over the settings the
    /// page then reads; it is not saved by itself (C4b item 4). It leaves the storage only once
    /// the shell has it (review C4B-B-09).
    pub(crate) fn new(theme: ThemeController) -> Self {
        let stored = use_signal(|| theme.settings.peek().clone());
        let edits = use_signal(|| Edits::restored(drafts::peek().unwrap_or_default()));
        let restored = !edits.peek().pending().is_empty();
        let draft = use_signal(|| edits.peek().over(&stored.peek()));
        let requests = use_signal(SettingsRequestLifecycle::default);
        let theme_import_revision = use_signal(|| 0);
        let numeric_revisions = use_signal(|| [0; 4]);
        let preview_owner = use_hook(move || theme.begin_preview());
        let preview_css = use_signal(|| None);
        use_drop(move || theme.end_preview(preview_owner));
        let preset_choice = use_signal(|| detect_preset_key(&draft.peek().custom_css).to_string());
        let status = use_signal(|| {
            if restored {
                let mut message = "已恢复重新加载前未保存的设置修改，尚未保存。".to_string();
                if edits.peek().pending().custom_css.is_some() {
                    message.push_str(" CSS 草稿已恢复，需重新预览。");
                }
                message
            } else {
                String::new()
            }
        });
        let status_tone = use_signal(|| "info".to_string());
        let shell = try_consume_context::<AppShellState>();

        let session = Self {
            theme,
            preview_owner,
            preview_css,
            stored,
            edits,
            draft,
            requests,
            preset_choice,
            status,
            status_tone,
            shell,
            theme_import_revision,
            numeric_revisions,
        };
        // The shell now owns any restored edits. Also consume a legacy draft whose only
        // field was retired; it must not stay in storage just because no edits remain.
        use_effect(move || {
            session.publish_draft();
            session.refresh_preview();
            drafts::forget();
        });
        session
    }

    /// Tells the shell the edits not stored yet, after every change of them (C4b item 4).
    fn publish_draft(self) {
        if let Some(shell) = self.shell {
            shell.set_settings_draft(self.edits.peek().unsaved());
        }
    }

    /// What the form shows (a reactive read).
    pub(crate) fn draft(self) -> UserSettings {
        (self.draft)()
    }

    /// The one way the form changes the draft: the fields `change` sets become edits.
    pub(crate) fn edit(mut self, change: impl FnOnce(&mut UserSettings)) {
        self.invalidate_theme_import();
        let mut next = self.draft.peek().clone();
        change(&mut next);
        let changed = SettingsPatch::between(&self.draft.peek(), &next);
        if changed.is_empty() {
            return;
        }
        self.edits.write().record(changed);
        self.draft.set(next);
        // A previous save result describes the previous draft, not these newer edits.
        self.set_status(String::new(), "info");
        self.publish_draft();
        self.refresh_preview();
    }

    fn refresh_preview(self) {
        let mut appearance = ThemeAppearance::from(&*self.draft.peek());
        appearance.custom_css = self
            .preview_css
            .peek()
            .clone()
            .unwrap_or_else(|| self.stored.peek().custom_css.clone());
        // Numeric inputs may hold an out-of-range draft until save reports validation. Do not
        // turn that intermediate value into unusable typography during a visual preview.
        if !(rssr_domain::validation::MIN_READER_FONT_SCALE
            ..=rssr_domain::validation::MAX_READER_FONT_SCALE)
            .contains(&appearance.reader_font_scale)
        {
            appearance.reader_font_scale = self.stored.peek().reader_font_scale;
        }
        self.theme.preview(self.preview_owner, appearance);
    }

    pub(crate) fn preview_custom_css(mut self, css: String) {
        self.preview_css.set(Some(css));
        self.refresh_preview();
    }

    pub(crate) fn has_unsaved_changes(self) -> bool {
        self.edits.read().has_unsaved_changes()
    }

    /// The edits a save would send.
    pub(crate) fn pending_edits(self) -> SettingsPatch {
        self.edits.peek().pending().clone()
    }

    /// Hands the pending edits to a save; call after [`Self::begin_write`] succeeded.
    pub(crate) fn submit_edits(mut self) -> SettingsPatch {
        let submitted = self.edits.write().submit();
        self.publish_draft();
        submitted
    }

    /// The save failed: what it sent is an edit again, under the ones made since.
    pub(crate) fn restore_submitted_edits(mut self) {
        self.edits.write().failed();
        self.publish_draft();
    }

    /// Starts a write whose reply may carry the stored settings (a save, a pull). Returns false,
    /// changing nothing, while another write is in flight.
    pub(crate) fn begin_write(mut self) -> bool {
        self.requests.write().begin_write()
    }

    /// Ends the write started by [`Self::begin_write`], after its reply has been applied. A load
    /// answered meanwhile applies now unless the write stored settings.
    pub(crate) fn end_write(mut self) {
        let held_load = self.requests.write().end_write();
        if let Some(loaded) = held_load {
            self.apply_load(loaded);
        }
    }

    /// Records that a write stored settings: loads issued before it are stale.
    fn note_stored(mut self) {
        self.requests.write().settings_stored();
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
        self.load_with(|command| run_ui_command(UiCommand::Settings(command)));
    }

    /// Reads the stored settings. Only the latest load's reply applies (an earlier one may have
    /// read before it). A write of the page that stores settings after the load was issued
    /// supersedes it: the read may predate the write, so the whole reply is discarded. A reply
    /// that arrives while a write is in flight waits for the write to end. Settings the app
    /// applied from the library after the load was issued (an import or a download that ended
    /// after its page was left, the shell's start) are not older than the read: the page takes
    /// them instead (C4b item 2, R2-03; reviews C4B-B-01, C4B-B-03).
    pub(super) fn load_with<F: Future<Output = Vec<UiIntent>> + 'static>(
        mut self,
        execute: impl FnOnce(SettingsCommand) -> F,
    ) {
        let request = self.requests.write().begin_load(self.theme.stored_count());
        let loading = execute(SettingsCommand::Load);
        spawn(async move {
            let intents = loading.await;
            if !self.requests.peek().accepts(request) {
                return;
            }
            apply_projected_ui_intents(intents, UiIntent::into_settings_page_intent, |intent| {
                match intent {
                    SettingsPageIntent::SettingsLoaded(settings) => {
                        self.receive_load(request, settings)
                    }
                    status => self.dispatch(status),
                }
            });
        });
    }

    fn receive_load(mut self, request: SettingsLoadRequest, settings: UserSettings) {
        let loaded = self.requests.write().receive_load(request, settings);
        if let Some(loaded) = loaded {
            self.apply_load(loaded);
        }
    }

    /// A load's settings, unless the app applied settings from the library since the load was
    /// issued (`applied` was its count then): the page then shows those, without applying them
    /// again. They are a write's (an import, a download), which the load may predate, or the
    /// shell's start read; either way nothing written is undone, and the next re-read brings
    /// anything newer.
    fn apply_load(self, loaded: LoadedSettings) {
        match loaded.resolve(self.theme.stored_count()) {
            LoadProjection::Apply(settings) => self.apply_stored_settings(settings),
            LoadProjection::ShowCurrent => {
                self.show_stored_settings(self.theme.settings.peek().clone());
            }
        }
    }

    /// Applies an intent from a reply other than a load's: statuses, and the settings a pull
    /// stored.
    pub(crate) fn dispatch(self, intent: SettingsPageIntent) {
        match intent {
            SettingsPageIntent::SettingsLoaded(settings) => {
                self.note_stored();
                self.apply_stored_settings(settings);
            }
            SettingsPageIntent::SetStatus { message, tone } => self.set_status(message, tone),
            SettingsPageIntent::Failed(failure) => {
                self.set_status(failure.message(), failure.tone())
            }
        }
    }

    /// Settings that were read or pulled are the stored ones; the edits stay on top. The preset
    /// follows the stored CSS unless the CSS is edited.
    fn apply_stored_settings(self, settings: UserSettings) {
        self.show_stored_settings(settings.clone());
        self.theme.store(settings);
    }

    /// The page's view of the stored settings, with the edits on top.
    fn show_stored_settings(mut self, settings: UserSettings) {
        let projection = self.project_stored(&settings, ProjectionSource::Stored);
        self.publish_projection(projection);
        self.stored.set(settings);
        self.refresh_preview();
    }

    fn project_stored(
        self,
        settings: &UserSettings,
        source: ProjectionSource<'_>,
    ) -> DraftProjection {
        project_stored(
            &self.edits.peek(),
            settings,
            &self.draft.peek().custom_css,
            *self.numeric_revisions.peek(),
            source,
        )
    }

    fn publish_projection(mut self, projection: DraftProjection) {
        self.numeric_revisions.set(projection.numeric_revisions);
        if projection.invalidate_theme_import {
            self.invalidate_theme_import();
        }
        if projection.refresh_preset {
            self.preset_choice.set(detect_preset_key(&projection.draft.custom_css).to_string());
        }
        if projection.clear_css_preview {
            self.preview_css.set(None);
        }
        self.draft.set(projection.draft);
    }

    /// A save stored `settings`: its edits are done, the ones made since stay on top. The preset
    /// follows the stored CSS unless one was chosen during the save. Returns whether edits are
    /// still pending (even ones whose value is the stored one: the next save sends them).
    pub(crate) fn apply_saved_settings(
        mut self,
        settings: UserSettings,
        submitted_preset: &str,
    ) -> bool {
        self.note_stored();
        self.edits.write().saved();
        self.publish_draft();
        let projection = self.project_stored(
            &settings,
            ProjectionSource::Saved {
                current_preset: self.preset_choice.peek().as_str(),
                submitted_preset,
            },
        );
        let has_unsaved_edits = !self.edits.peek().pending().is_empty();
        self.publish_projection(projection);
        self.stored.set(settings.clone());
        self.theme.store(settings);
        self.refresh_preview();
        has_unsaved_edits
    }

    /// Whether the page (this instance) is still open: its state is dropped with it.
    pub(crate) fn is_open(self) -> bool {
        self.status.try_peek().is_ok()
    }

    /// Applies settings the library stored to the app (its theme), with the page gone.
    pub(crate) fn store_theme(self, settings: UserSettings) {
        self.theme.store(settings);
    }

    pub(crate) fn set_status(self, message: impl Into<String>, tone: impl Into<String>) {
        crate::status::set_status(self.status, self.status_tone, message, tone);
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
    fn harness() -> Element {
        let page = SettingsPageSession::new(crate::theme::use_theme_controller());
        use_context_provider(|| page);
        rsx! {}
    }
    fn setup() -> (VirtualDom, SettingsPageSession) {
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let page = dom.in_scope(ScopeId::APP, consume_context);
        (dom, page)
    }

    #[test]
    fn stored_projection_retires_only_unedited_numeric_fields_even_for_equal_values() {
        let (dom, page) = setup();
        dom.in_scope(ScopeId::APP, || {
            page.edit(|draft| draft.entries_page_size = 80);
            let before = page.numeric_revisions();
            page.apply_stored_settings(UserSettings::default());
            let after = page.numeric_revisions();
            assert_eq!(after[2], before[2], "edited page size stays over the stored value");
            assert_eq!(after[0], before[0] + 1, "equal unedited values still retire feedback");
            assert_eq!(page.draft().entries_page_size, 80);
            page.apply_stored_settings(UserSettings {
                refresh_interval_minutes: 90,
                ..Default::default()
            });
            page.apply_stored_settings(UserSettings::default());
            assert_eq!(
                page.numeric_revisions()[0],
                after[0] + 2,
                "A-B-A cannot revive an old hint"
            );
        });
    }

    #[test]
    fn saved_projection_keeps_later_field_edits_and_retires_acknowledged_fields() {
        let (dom, page) = setup();
        dom.in_scope(ScopeId::APP, || {
            page.edit(|draft| {
                draft.entries_page_size = 0;
                draft.refresh_interval_minutes = 90;
            });
            assert!(page.begin_write());
            let submitted = page.submit_edits();
            page.edit(|draft| draft.refresh_interval_minutes = 60);
            let before = page.numeric_revisions();
            let mut saved = submitted.applied(&UserSettings::default());
            saved.entries_page_size = 100;
            assert!(page.apply_saved_settings(saved, "none"));
            page.end_write();
            assert_eq!(page.draft().refresh_interval_minutes, 60);
            assert_eq!(page.draft().entries_page_size, 100);
            assert_eq!(
                page.numeric_revisions()[0],
                before[0],
                "later edit still owns its feedback"
            );
            assert_eq!(
                page.numeric_revisions()[2],
                before[2] + 1,
                "normalization retires the acknowledged field"
            );
        });
    }

    #[test]
    fn failed_save_and_held_load_keep_submitted_edits_and_their_feedback() {
        let (dom, page) = setup();
        dom.in_scope(ScopeId::APP, || {
            page.edit(|draft| draft.entries_page_size = 80);
            assert!(page.begin_write());
            page.submit_edits();
            let before = page.numeric_revisions();
            let mut requests = page.requests;
            let request = requests.write().begin_load(page.theme.stored_count());
            page.receive_load(request, UserSettings::default());
            page.restore_submitted_edits();
            page.end_write();
            assert_eq!(page.draft().entries_page_size, 80);
            assert_eq!(page.numeric_revisions()[2], before[2]);
            assert_eq!(page.pending_edits().entries_page_size, Some(80));
        });
    }

    #[test]
    fn stored_css_change_invalidates_an_older_file_read_without_discarding_field_edits() {
        let (dom, page) = setup();
        dom.in_scope(ScopeId::APP, || {
            page.edit(|draft| draft.entries_page_size = 80);
            let request = page.begin_theme_import();
            page.apply_stored_settings(UserSettings {
                custom_css: "/* remote */".into(),
                ..Default::default()
            });
            assert!(!page.complete_theme_import(request));
            assert_eq!(page.draft().entries_page_size, 80);
            assert_eq!(page.draft().custom_css, "/* remote */");
        });
    }
}
