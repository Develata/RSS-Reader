use dioxus::prelude::*;

use super::state::SettingsPageSyncState;
use crate::ui::{SettingsCommand, UiCommand, UiIntent, spawn_projected_ui_command};
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
        self.state.with_mut(|state| state.endpoint = endpoint);
    }

    pub(crate) fn set_remote_path(mut self, remote_path: String) {
        self.state.with_mut(|state| state.remote_path = remote_path);
    }

    pub(crate) fn push(self) {
        let snapshot = self.snapshot();
        self.apply_ui_command(UiCommand::Settings(SettingsCommand::PushConfig {
            endpoint: snapshot.endpoint,
            remote_path: snapshot.remote_path,
        }));
    }

    pub(crate) fn pull(mut self) {
        if !self.snapshot().pending_remote_pull {
            self.state.with_mut(|state| state.pending_remote_pull = true);
            set_status_info(
                self.page.status_signal(),
                self.page.status_tone_signal(),
                "从 WebDAV 下载配置会覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。",
            );
            return;
        }

        let snapshot = self.snapshot();
        self.apply_ui_command(UiCommand::Settings(SettingsCommand::PullConfig {
            endpoint: snapshot.endpoint,
            remote_path: snapshot.remote_path,
        }));
    }

    fn apply_ui_command(mut self, command: UiCommand) {
        self.state.with_mut(|state| state.pending_remote_pull = false);
        spawn_projected_ui_command(command, UiIntent::into_settings_page_intent, move |intent| {
            self.page.dispatch(intent);
        });
    }
}
