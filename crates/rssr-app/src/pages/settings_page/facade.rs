use super::themes::{detect_preset_key, preset_css, preset_display_name};
use dioxus::prelude::WritableExt;

use super::{
    save::{SettingsPageSaveSession, SettingsPageSaveState},
    session::SettingsPageSession,
    sync::{SettingsPageSyncSession, SettingsPageSyncState},
};
use rssr_domain::UserSettings;

#[derive(Clone, PartialEq)]
pub(crate) struct SettingsPageFacade {
    page: SettingsPageSession,
    save: SettingsPageSaveSession,
    save_snapshot: SettingsPageSaveState,
    sync: SettingsPageSyncSession,
    sync_snapshot: SettingsPageSyncState,
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

    pub(crate) fn custom_css(&self) -> String {
        self.draft().custom_css
    }

    pub(crate) fn set_custom_css(&self, value: String) {
        let preset = detect_preset_key(&value).to_string();
        self.update_draft(|next| {
            next.custom_css = value;
        });
        self.set_preset_choice(preset);
    }

    pub(crate) fn preset_choice(&self) -> String {
        (self.page.preset_choice())()
    }

    pub(crate) fn set_preset_choice(&self, value: impl Into<String>) {
        let mut preset_choice = self.page.preset_choice();
        preset_choice.set(value.into());
    }

    pub(crate) fn status_message(&self) -> String {
        self.page.status()
    }

    pub(crate) fn status_tone(&self) -> String {
        self.page.status_tone()
    }

    pub(crate) fn has_status_message(&self) -> bool {
        !self.status_message().is_empty()
    }

    pub(crate) fn set_status(&self, message: impl Into<String>, tone: impl Into<String>) {
        self.page.set_status(message, tone);
    }

    pub(crate) fn apply_selected_theme(&self) {
        let choice = self.preset_choice();
        if choice == "none" {
            self.clear_custom_css("已清空自定义 CSS。");
            return;
        }
        if choice == "custom" {
            self.set_status("当前是自定义主题，请直接编辑 CSS 或从文件导入。", "info");
            return;
        }
        self.apply_builtin_theme(choice.as_str());
    }

    pub(crate) fn apply_builtin_theme(&self, preset_key: &str) {
        self.set_custom_css(preset_css(preset_key).to_string());
        self.save_with_message(format!("已应用示例主题：{}。", preset_display_name(preset_key)));
    }

    pub(crate) fn clear_custom_css(&self, success_message: impl Into<String>) {
        self.update_draft(|next| {
            next.custom_css.clear();
        });
        self.set_preset_choice("none");
        self.save_with_message(success_message.into());
    }

    pub(crate) fn is_theme_preset_active(&self, preset_key: &str) -> bool {
        detect_preset_key(&self.custom_css()) == preset_key
    }

    pub(crate) fn theme_card_class(&self, preset_key: &str) -> &'static str {
        if self.is_theme_preset_active(preset_key) { "theme-card is-active" } else { "theme-card" }
    }

    pub(crate) fn theme_apply_button_class(&self, preset_key: &str) -> &'static str {
        if self.is_theme_preset_active(preset_key) { "button" } else { "button secondary" }
    }

    pub(crate) fn theme_apply_button_label(&self, preset_key: &str) -> &'static str {
        if self.is_theme_preset_active(preset_key) { "当前已选" } else { "使用这套主题" }
    }

    pub(crate) fn remove_theme_preset(&self, preset_key: &str, preset_name: &str) {
        if !self.is_theme_preset_active(preset_key) {
            self.set_status(format!("当前并未启用主题：{}。", preset_name), "info");
            return;
        }
        self.clear_custom_css(format!("已移除主题：{}。", preset_name));
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

    pub(crate) fn is_save_pending(&self) -> bool {
        self.save_snapshot.pending_save
    }

    pub(crate) fn save_button_label(&self) -> &'static str {
        if self.is_save_pending() { "正在保存…" } else { "保存设置" }
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

    pub(crate) fn is_remote_pull_pending(&self) -> bool {
        self.sync_snapshot.pending_remote_pull
    }

    pub(crate) fn remote_pull_button_class(&self) -> &'static str {
        if self.is_remote_pull_pending() { "button danger" } else { "button secondary" }
    }

    pub(crate) fn remote_pull_button_label(&self) -> &'static str {
        if self.is_remote_pull_pending() { "确认下载并覆盖" } else { "下载配置" }
    }

    pub(crate) fn push(&self) {
        self.sync.push();
    }

    pub(crate) fn pull(&self) {
        self.sync.pull();
    }
}
