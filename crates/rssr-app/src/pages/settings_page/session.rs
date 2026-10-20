use std::future::Future;

use dioxus::prelude::*;
use rssr_application::SettingsPatch;
use rssr_domain::UserSettings;

use super::{browser::open_repository_url, intent::SettingsPageIntent, themes::detect_preset_key};
use crate::{
    theme::ThemeController,
    ui::{SettingsCommand, UiCommand, UiIntent, apply_projected_ui_intents, run_ui_command},
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
    stored: Signal<UserSettings>,
    edits: Signal<Edits>,
    /// `stored` with the edits on top; what the form shows.
    draft: Signal<UserSettings>,
    writes: Signal<Writes>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
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
    /// A load answered while a write was in flight. It waits for the write: dropped if the write
    /// stores settings, applied if it stores nothing (it failed, or the remote had none).
    held_load: Option<UserSettings>,
}

impl SettingsPageSession {
    pub(crate) fn new(theme: ThemeController) -> Self {
        let stored = use_signal(|| theme.settings.peek().clone());
        let edits = use_signal(Edits::default);
        let draft = use_signal(|| stored.peek().clone());
        let writes = use_signal(Writes::default);
        let preset_choice = use_signal(|| detect_preset_key(&stored.peek().custom_css).to_string());
        let status = use_signal(String::new);
        let status_tone = use_signal(|| "info".to_string());

        Self { theme, stored, edits, draft, writes, preset_choice, status, status_tone }
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
    }

    /// The edits a save would send.
    pub(crate) fn pending_edits(self) -> SettingsPatch {
        self.edits.peek().pending.clone()
    }

    /// Hands the pending edits to a save; call after [`Self::begin_write`] succeeded.
    pub(crate) fn submit_edits(mut self) -> SettingsPatch {
        let mut edits = self.edits.write();
        let submitted = std::mem::take(&mut edits.pending);
        edits.submitted = Some(submitted.clone());
        submitted
    }

    /// The save failed: what it sent is an edit again, under the ones made since.
    pub(crate) fn restore_submitted_edits(mut self) {
        let mut edits = self.edits.write();
        if let Some(submitted) = edits.submitted.take() {
            edits.pending = submitted.followed_by(std::mem::take(&mut edits.pending));
        }
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
        if let Some(settings) = held_load {
            self.apply_stored_settings(settings);
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

    /// Reads the stored settings. A write that stores settings after the load was issued
    /// supersedes it: the read may predate the write, so the whole reply is discarded. A reply
    /// that arrives while a write is in flight waits for the write to end.
    pub(super) fn load_with<F: Future<Output = Vec<UiIntent>> + 'static>(
        self,
        execute: impl FnOnce(SettingsCommand) -> F,
    ) {
        let issued = self.writes.peek().stored;
        let loading = execute(SettingsCommand::Load);
        spawn(async move {
            let intents = loading.await;
            if self.writes.peek().stored != issued {
                return;
            }
            apply_projected_ui_intents(intents, UiIntent::into_settings_page_intent, |intent| {
                match intent {
                    SettingsPageIntent::SettingsLoaded(settings) => self.receive_load(settings),
                    status => self.dispatch(status),
                }
            });
        });
    }

    fn receive_load(mut self, settings: UserSettings) {
        if self.writes.peek().in_flight {
            self.writes.write().held_load = Some(settings);
        } else {
            self.apply_stored_settings(settings);
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
    fn apply_stored_settings(mut self, settings: UserSettings) {
        let draft = self.edits.peek().over(&settings);
        if !self.edits.peek().edit_custom_css() {
            self.preset_choice.set(detect_preset_key(&draft.custom_css).to_string());
        }
        self.draft.set(draft);
        self.stored.set(settings.clone());
        self.theme.store(settings);
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
        let draft = self.edits.peek().over(&settings);
        if self.preset_choice.peek().as_str() == submitted_preset {
            self.preset_choice.set(detect_preset_key(&draft.custom_css).to_string());
        }
        let has_unsaved_edits = !self.edits.peek().pending.is_empty();
        self.draft.set(draft);
        self.stored.set(settings.clone());
        self.theme.store(settings);
        has_unsaved_edits
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
