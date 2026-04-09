use dioxus::prelude::WritableExt;

use super::{
    save::{SettingsPageSaveSession, SettingsPageSaveState},
    session::SettingsPageSession,
    sync::{SettingsPageSyncSession, SettingsPageSyncState},
};
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

    pub(crate) fn draft(&self) -> UserSettings {
        (self.page.draft())()
    }

    pub(crate) fn update_draft(&self, update: impl FnOnce(&mut UserSettings)) {
        let mut draft = self.page.draft();
        let mut next = draft();
        update(&mut next);
        draft.set(next);
    }

    pub(crate) fn preset_choice(&self) -> String {
        (self.page.preset_choice())()
    }

    pub(crate) fn set_preset_choice(&self, value: impl Into<String>) {
        let mut preset_choice = self.page.preset_choice();
        preset_choice.set(value.into());
    }

    pub(crate) fn status(&self) -> String {
        self.page.status()
    }

    pub(crate) fn status_tone(&self) -> String {
        self.page.status_tone()
    }

    pub(crate) fn set_status(&self, message: impl Into<String>, tone: impl Into<String>) {
        self.page.set_status(message, tone);
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

    pub(crate) fn endpoint(&self) -> &str {
        &self.sync_snapshot.endpoint
    }

    pub(crate) fn set_endpoint(&self, endpoint: String) {
        self.sync.set_endpoint(endpoint);
    }

    pub(crate) fn remote_path(&self) -> &str {
        &self.sync_snapshot.remote_path
    }

    pub(crate) fn set_remote_path(&self, remote_path: String) {
        self.sync.set_remote_path(remote_path);
    }

    pub(crate) fn pending_remote_pull(&self) -> bool {
        self.sync_snapshot.pending_remote_pull
    }

    pub(crate) fn push(&self) {
        self.sync.push();
    }

    pub(crate) fn pull(&self) {
        self.sync.pull();
    }
}
