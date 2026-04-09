use super::{
    save::{SettingsPageSaveSession, SettingsPageSaveState},
    session::SettingsPageSession,
    sync::{SettingsPageSyncSession, SettingsPageSyncState},
};
use dioxus::prelude::Signal;
use rssr_domain::UserSettings;

#[derive(Clone, PartialEq)]
pub(crate) struct SettingsPageFacade {
    pub(crate) page: SettingsPageSession,
    pub(crate) save: SettingsPageSaveSession,
    pub(crate) save_snapshot: SettingsPageSaveState,
    pub(crate) sync: SettingsPageSyncSession,
    pub(crate) sync_snapshot: SettingsPageSyncState,
}

impl SettingsPageFacade {
    pub(crate) fn new(
        page: SettingsPageSession,
        save: SettingsPageSaveSession,
        save_snapshot: SettingsPageSaveState,
        sync: SettingsPageSyncSession,
        sync_snapshot: SettingsPageSyncState,
    ) -> Self {
        Self { page, save, save_snapshot, sync, sync_snapshot }
    }

    pub(crate) fn draft_signal(&self) -> Signal<UserSettings> {
        self.page.draft()
    }

    pub(crate) fn preset_choice_signal(&self) -> Signal<String> {
        self.page.preset_choice()
    }

    pub(crate) fn status(&self) -> String {
        self.page.status()
    }

    pub(crate) fn status_tone(&self) -> String {
        self.page.status_tone()
    }

    pub(crate) fn status_signal(&self) -> Signal<String> {
        self.page.status_signal()
    }

    pub(crate) fn status_tone_signal(&self) -> Signal<String> {
        self.page.status_tone_signal()
    }

    pub(crate) fn open_repository(&self) {
        self.page.open_repository();
    }

    pub(crate) fn save(&self) {
        self.save.save();
    }

    pub(crate) fn save_with_message(&self, success_message: impl Into<String>) {
        self.save.save_with_message(success_message);
    }

    pub(crate) fn pending_save(&self) -> bool {
        self.save_snapshot.pending_save
    }

    pub(crate) fn set_endpoint(&self, endpoint: String) {
        self.sync.set_endpoint(endpoint);
    }

    pub(crate) fn set_remote_path(&self, remote_path: String) {
        self.sync.set_remote_path(remote_path);
    }

    pub(crate) fn push(&self) {
        self.sync.push();
    }

    pub(crate) fn pull(&self) {
        self.sync.pull();
    }
}
