use dioxus::prelude::*;
use rssr_domain::UserSettings;

use super::{
    effect::SettingsPageSyncEffect, runtime::execute_settings_page_sync_effect,
    state::SettingsPageSyncState,
};
use crate::{
    pages::settings_page::themes::detect_preset_key, status::set_status_info,
    theme::ThemeController,
};

#[derive(Clone, Copy)]
pub(crate) struct SettingsPageSyncSession {
    state: Signal<SettingsPageSyncState>,
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
}

impl SettingsPageSyncSession {
    pub(crate) fn new(
        state: Signal<SettingsPageSyncState>,
        theme: ThemeController,
        draft: Signal<UserSettings>,
        preset_choice: Signal<String>,
        status: Signal<String>,
        status_tone: Signal<String>,
    ) -> Self {
        Self { state, theme, draft, preset_choice, status, status_tone }
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
            let outcome = execute_settings_page_sync_effect(SettingsPageSyncEffect::PushConfig {
                endpoint: snapshot.endpoint,
                remote_path: snapshot.remote_path,
            })
            .await;
            self.apply_runtime_outcome(outcome);
        });
    }

    pub(crate) fn pull(mut self) {
        if !self.snapshot().pending_remote_pull {
            self.state.with_mut(|state| state.pending_remote_pull = true);
            set_status_info(
                self.status,
                self.status_tone,
                "从 WebDAV 下载配置会覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。",
            );
            return;
        }

        let snapshot = self.snapshot();
        spawn(async move {
            let outcome = execute_settings_page_sync_effect(SettingsPageSyncEffect::PullConfig {
                endpoint: snapshot.endpoint,
                remote_path: snapshot.remote_path,
            })
            .await;
            self.apply_runtime_outcome(outcome);
        });
    }

    fn apply_runtime_outcome(mut self, outcome: super::runtime::SettingsPageSyncRuntimeOutcome) {
        self.state.with_mut(|state| state.pending_remote_pull = false);
        if let Some(settings) = outcome.imported_settings {
            self.preset_choice.set(detect_preset_key(&settings.custom_css).to_string());
            self.draft.set(settings.clone());
            self.theme.settings.set(settings);
        }
        if outcome.status_tone == "error" {
            self.status_tone.set("error".to_string());
            self.status.set(outcome.status_message);
        } else {
            set_status_info(self.status, self.status_tone, outcome.status_message);
        }
    }
}
