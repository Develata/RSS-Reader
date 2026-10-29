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
    preview_owner: u64,
    /// Explicit CSS preview, or the current stored CSS when none was requested on this page.
    /// Typing and restoring an editor draft never implicitly apply that draft.
    preview_css: Signal<Option<String>>,
    stored: Signal<UserSettings>,
    edits: Signal<Edits>,
    /// `stored` with the edits on top; what the form shows.
    draft: Signal<UserSettings>,
    writes: Signal<Writes>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
    /// The app's shell, which keeps the edits not stored yet across a reload or a sign-in
    /// (C4b item 4); none in tests of the page alone.
    shell: Option<AppShellState>,
}

/// The user's edits that are not stored yet.
#[derive(Clone, Default, PartialEq)]
struct Edits {
    /// Sent by the save in flight: stored by its reply, or edits again if it fails.
    submitted: Option<SettingsPatch>,
    /// Made since (all of them while no save is in flight).
    pending: SettingsPatch,
}

impl Edits {
    fn over(&self, stored: &UserSettings) -> UserSettings {
        let submitted = self.submitted.as_ref().map(|patch| patch.applied(stored));
        self.pending.applied(submitted.as_ref().unwrap_or(stored))
    }

    fn edit_custom_css(&self) -> bool {
        self.pending.custom_css.is_some()
            || self.submitted.as_ref().is_some_and(|patch| patch.custom_css.is_some())
    }

    /// All of them: the ones a save in flight sent, then the ones made since (C4b item 4).
    fn unsaved(&self) -> Option<SettingsPatch> {
        let all = match &self.submitted {
            Some(submitted) => submitted.clone().followed_by(self.pending.clone()),
            None => self.pending.clone(),
        };
        (!all.is_empty()).then_some(all)
    }
}

/// The page's writes (saves, pulls) as its loads see them.
#[derive(Clone, Default, PartialEq)]
struct Writes {
    /// One write at a time: a pull replaces the stored settings a save in flight is measured
    /// against, and a save would store over a pull it does not know of.
    in_flight: bool,
    /// Writes whose reply carried the stored settings. A load issued before one of them may
    /// predate it, so its reply is discarded.
    stored: u64,
    /// A load answered while a write was in flight, with the app's count of applied settings when
    /// it was issued and its read generation. It waits for the write: dropped if the write stores
    /// settings or a later load was issued meanwhile (review C4B-B-R2-02), applied if not.
    held_load: Option<(UserSettings, u64, u64)>,
    /// Loads issued: only the latest one's reply applies, whatever order the replies come in
    /// (C4b item 2; review C4B-B-01).
    reads: u64,
}

impl SettingsPageSession {
    /// A draft kept before a reload or a sign-in becomes the pending edits, over the settings the
    /// page then reads; it is not saved by itself (C4b item 4). It leaves the storage only once
    /// the shell has it (review C4B-B-09).
    pub(crate) fn new(theme: ThemeController) -> Self {
        let stored = use_signal(|| theme.settings.peek().clone());
        let edits =
            use_signal(|| Edits { submitted: None, pending: drafts::peek().unwrap_or_default() });
        let restored = !edits.peek().pending.is_empty();
        let draft = use_signal(|| edits.peek().over(&stored.peek()));
        let preview_owner = use_hook(move || theme.begin_preview());
        let preview_css = use_signal(|| None);
        use_drop(move || theme.end_preview(preview_owner));
        let writes = use_signal(Writes::default);
        let preset_choice = use_signal(|| detect_preset_key(&draft.peek().custom_css).to_string());
        let status = use_signal(|| {
            if restored {
                let mut message = "已恢复重新加载前未保存的设置修改，尚未保存。".to_string();
                if edits.peek().pending.custom_css.is_some() {
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
            writes,
            preset_choice,
            status,
            status_tone,
            shell,
        };
        // A restored draft is the shell's to keep from now on; then it leaves the storage.
        use_effect(move || {
            session.publish_draft();
            session.refresh_preview();
            if restored {
                drafts::forget();
            }
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
        let mut next = self.draft.peek().clone();
        change(&mut next);
        let changed = SettingsPatch::between(&self.draft.peek(), &next);
        if changed.is_empty() {
            return;
        }
        self.edits.with_mut(|edits| {
            edits.pending = std::mem::take(&mut edits.pending).followed_by(changed);
        });
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
        let edits = self.edits.read();
        !edits.pending.is_empty() || edits.submitted.is_some()
    }

    /// The edits a save would send.
    pub(crate) fn pending_edits(self) -> SettingsPatch {
        self.edits.peek().pending.clone()
    }

    /// Hands the pending edits to a save; call after [`Self::begin_write`] succeeded.
    pub(crate) fn submit_edits(mut self) -> SettingsPatch {
        let submitted = {
            let mut edits = self.edits.write();
            let submitted = std::mem::take(&mut edits.pending);
            edits.submitted = Some(submitted.clone());
            submitted
        };
        self.publish_draft();
        submitted
    }

    /// The save failed: what it sent is an edit again, under the ones made since.
    pub(crate) fn restore_submitted_edits(mut self) {
        {
            let mut edits = self.edits.write();
            if let Some(submitted) = edits.submitted.take() {
                edits.pending = submitted.followed_by(std::mem::take(&mut edits.pending));
            }
        }
        self.publish_draft();
    }

    /// Starts a write whose reply may carry the stored settings (a save, a pull). Returns false,
    /// changing nothing, while another write is in flight.
    pub(crate) fn begin_write(mut self) -> bool {
        if self.writes.peek().in_flight {
            return false;
        }
        self.writes.write().in_flight = true;
        true
    }

    /// Ends the write started by [`Self::begin_write`], after its reply has been applied. A load
    /// answered meanwhile applies now unless the write stored settings.
    pub(crate) fn end_write(mut self) {
        let held_load = {
            let mut writes = self.writes.write();
            writes.in_flight = false;
            writes.held_load.take()
        };
        if let Some((settings, applied, read)) = held_load
            && read == self.writes.peek().reads
        {
            self.apply_load(settings, applied);
        }
    }

    /// Records that a write stored settings: loads issued before it are stale.
    fn note_stored(mut self) {
        let mut writes = self.writes.write();
        writes.stored += 1;
        writes.held_load = None;
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
        let (issued, read) = {
            let mut writes = self.writes.write();
            writes.reads = writes.reads.wrapping_add(1);
            (writes.stored, writes.reads)
        };
        let applied = self.theme.stored_count();
        let loading = execute(SettingsCommand::Load);
        spawn(async move {
            let intents = loading.await;
            let writes = self.writes.peek();
            if writes.stored != issued || writes.reads != read {
                return;
            }
            drop(writes);
            apply_projected_ui_intents(intents, UiIntent::into_settings_page_intent, |intent| {
                match intent {
                    SettingsPageIntent::SettingsLoaded(settings) => {
                        self.receive_load(settings, applied, read)
                    }
                    status => self.dispatch(status),
                }
            });
        });
    }

    fn receive_load(mut self, settings: UserSettings, applied: u64, read: u64) {
        if self.writes.peek().in_flight {
            self.writes.write().held_load = Some((settings, applied, read));
        } else {
            self.apply_load(settings, applied);
        }
    }

    /// A load's settings, unless the app applied settings from the library since the load was
    /// issued (`applied` was its count then): the page then shows those, without applying them
    /// again. They are a write's (an import, a download), which the load may predate, or the
    /// shell's start read; either way nothing written is undone, and the next re-read brings
    /// anything newer.
    fn apply_load(self, settings: UserSettings, applied: u64) {
        if self.theme.stored_count() == applied {
            self.apply_stored_settings(settings);
        } else {
            self.show_stored_settings(self.theme.settings.peek().clone());
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
        let draft = self.edits.peek().over(&settings);
        if !self.edits.peek().edit_custom_css() {
            self.preset_choice.set(detect_preset_key(&draft.custom_css).to_string());
            self.preview_css.set(None);
        }
        self.draft.set(draft);
        self.stored.set(settings);
        self.refresh_preview();
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
        self.edits.write().submitted = None;
        self.publish_draft();
        let draft = self.edits.peek().over(&settings);
        if self.preset_choice.peek().as_str() == submitted_preset {
            self.preset_choice.set(detect_preset_key(&draft.custom_css).to_string());
        }
        let has_unsaved_edits = !self.edits.peek().pending.is_empty();
        if self.edits.peek().pending.custom_css.is_none() {
            self.preview_css.set(None);
        }
        self.draft.set(draft);
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
