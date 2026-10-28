use dioxus::prelude::*;

use super::state::SettingsPageSyncState;
use crate::ui::{UiCommand, UiIntent, spawn_projected_ui_command};
use crate::{pages::settings_page::session::SettingsPageSession, status::set_status_info};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SettingsPageSyncSession {
    state: Signal<SettingsPageSyncState>,
    page: SettingsPageSession,
}

impl SettingsPageSyncSession {
    pub(crate) fn new(state: Signal<SettingsPageSyncState>, page: SettingsPageSession) -> Self {
        Self { state, page }
    }

    pub(crate) fn snapshot(self) -> SettingsPageSyncState {
        (self.state)()
    }

    pub(crate) fn set_endpoint(mut self, endpoint: String) {
        self.state.with_mut(|state| state.set_endpoint(endpoint));
    }

    pub(crate) fn set_remote_path(mut self, remote_path: String) {
        self.state.with_mut(|state| state.set_remote_path(remote_path));
    }

    pub(crate) fn push(mut self) {
        let command = self.state.with_mut(SettingsPageSyncState::prepare_push);
        self.apply_ui_command(UiCommand::Settings(command));
    }

    pub(crate) fn pull(mut self) {
        let command = self.state.with_mut(SettingsPageSyncState::prepare_pull);
        let Some(command) = command else {
            set_status_info(
                self.page.status_signal(),
                self.page.status_tone_signal(),
                "从 WebDAV 下载配置会覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。",
            );
            return;
        };
        self.apply_ui_command(UiCommand::Settings(command));
    }

    fn apply_ui_command(self, command: UiCommand) {
        spawn_projected_ui_command(command, UiIntent::into_settings_page_intent, move |intent| {
            self.page.dispatch(intent);
        });
    }
}
