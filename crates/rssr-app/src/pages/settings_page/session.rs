use std::future::Future;

use dioxus::prelude::*;
use rssr_application::SettingsPatch;
use rssr_domain::UserSettings;

use super::{browser::open_repository_url, intent::SettingsPageIntent, themes::detect_preset_key};
use crate::{
    status::{set_status_error, set_status_info},
    theme::ThemeController,
    ui::{SettingsCommand, UiCommand, UiIntent, apply_projected_ui_intents, execute_ui_command},
};

/// The page edits a draft against a baseline, the stored settings as the page itself last read
/// or wrote them; a save sends only the fields where the two differ (B2-D4). The baseline is the
/// page's own: the app-wide [`ThemeController`] signal, which startup and other pages also set
/// whenever their replies arrive, only renders the settings.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SettingsPageSession {
    theme: ThemeController,
    baseline: Signal<UserSettings>,
    draft: Signal<UserSettings>,
    writes: Signal<Writes>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
}

/// The page's writes (saves, pulls) as its loads see them (B2-D4).
#[derive(Clone, Default, PartialEq)]
struct Writes {
    /// One write at a time: each reply is merged against the baseline, so a second write in
    /// flight would be merged against a baseline the first one changes, and lose edits.
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
        let baseline = use_signal(|| theme.settings.peek().clone());
        let draft = use_signal(|| baseline.peek().clone());
        let writes = use_signal(Writes::default);
        let preset_choice =
            use_signal(|| detect_preset_key(&baseline.peek().custom_css).to_string());
        let status = use_signal(String::new);
        let status_tone = use_signal(|| "info".to_string());

        Self { theme, baseline, draft, writes, preset_choice, status, status_tone }
    }

    pub(crate) fn draft(self) -> Signal<UserSettings> {
        self.draft
    }

    /// The settings as the page last read or wrote them: the baseline a save compares the
    /// draft with.
    pub(crate) fn saved_settings(self) -> UserSettings {
        self.baseline.peek().clone()
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

    /// Records that a write stored `settings`: loads issued before it are stale.
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
        self.load_with(|command| execute_ui_command(UiCommand::Settings(command)));
    }

    /// Reads the stored settings (B2-D4). A write that stores settings after the load was issued
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

    /// Applies a write's reply: settings in it are what the write stored (a pull).
    pub(crate) fn dispatch(self, intent: SettingsPageIntent) {
        match intent {
            SettingsPageIntent::SettingsLoaded(settings) => {
                self.note_stored();
                self.apply_stored_settings(settings);
            }
            SettingsPageIntent::SetStatus { message, tone } => self.set_status(message, tone),
        }
    }

    /// Stored settings that were read or pulled become the baseline; fields edited since the
    /// previous baseline stay in the draft.
    fn apply_stored_settings(mut self, settings: UserSettings) {
        let edits = SettingsPatch::between(&self.baseline.peek(), &self.draft.peek());
        let draft = edits.applied(&settings);
        if edits.custom_css.is_none() {
            self.preset_choice.set(detect_preset_key(&draft.custom_css).to_string());
        }
        self.draft.set(draft);
        self.baseline.set(settings.clone());
        self.theme.settings.set(settings);
    }

    /// Persisted settings and editable input are different snapshots while a save is pending.
    /// The draft takes every stored field except the ones edited since the submission (including
    /// a newly selected preset), which stay for the next explicit save.
    pub(crate) fn apply_saved_settings(
        mut self,
        settings: UserSettings,
        submitted_draft: &UserSettings,
        submitted_preset: &str,
    ) -> bool {
        self.note_stored();
        let later_edits = SettingsPatch::between(submitted_draft, &self.draft.peek());
        self.draft.set(later_edits.applied(&settings));
        if self.preset_choice.peek().as_str() == submitted_preset {
            self.preset_choice.set(detect_preset_key(&self.draft.peek().custom_css).to_string());
        }
        let has_unsaved_edits = *self.draft.peek() != settings;
        self.baseline.set(settings.clone());
        self.theme.settings.set(settings);
        has_unsaved_edits
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
