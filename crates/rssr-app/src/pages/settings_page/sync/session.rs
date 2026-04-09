use dioxus::prelude::*;

use super::state::SettingsPageSyncState;
use crate::ui::{UiCommand, UiIntent, execute_ui_command};
use crate::{pages::settings_page::session::SettingsPageSession, status::set_status_info};

#[derive(Clone, Copy)]
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
        spawn(async move {
            let intents = execute_ui_command(UiCommand::SettingsPushConfig {
                endpoint: snapshot.endpoint,
                remote_path: snapshot.remote_path,
            })
            .await;
            self.apply_runtime_intents(intents);
        });
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
        spawn(async move {
            let intents = execute_ui_command(UiCommand::SettingsPullConfig {
                endpoint: snapshot.endpoint,
                remote_path: snapshot.remote_path,
            })
            .await;
            self.apply_runtime_intents(intents);
        });
    }

    fn apply_runtime_intents(mut self, intents: Vec<UiIntent>) {
        self.state.with_mut(|state| state.pending_remote_pull = false);
        for intent in intents {
            if let Some(intent) = intent.into_settings_page_intent() {
                self.page.dispatch(intent);
            }
        }
    }
}
