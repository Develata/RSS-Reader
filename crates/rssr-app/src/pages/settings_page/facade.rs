use super::themes::{detect_preset_key, preset_css, preset_display_name};

use super::{
    save::{SettingsPageSaveSession, SettingsPageSaveState},
    session::{SettingsPageSession, ThemeImportRequest},
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
        self.page.draft()
    }

    pub(crate) fn update_draft(&self, update: impl FnOnce(&mut UserSettings)) {
        self.page.edit(update);
    }

    pub(crate) fn note_draft_edit(&self) {
        self.page.invalidate_theme_import();
    }

    pub(crate) fn numeric_revisions(&self) -> [u64; 4] {
        self.page.numeric_revisions()
    }

    pub(crate) fn begin_theme_import(&self) -> ThemeImportRequest {
        self.page.begin_theme_import()
    }

    pub(crate) fn invalidate_theme_import(&self) {
        self.page.invalidate_theme_import();
    }

    pub(crate) fn is_theme_import_current(&self, request: ThemeImportRequest) -> bool {
        self.page.is_theme_import_current(request)
    }

    pub(crate) fn complete_theme_import(&self, request: ThemeImportRequest) -> bool {
        self.page.complete_theme_import(request)
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
        self.page.set_preset_choice(value);
    }

    pub(crate) fn status_message(&self) -> String {
        self.page.status()
    }

    pub(crate) fn status_tone(&self) -> String {
        self.page.status_tone()
    }

    pub(crate) fn set_status(&self, message: impl Into<String>, tone: impl Into<String>) {
        self.page.set_status(message, tone);
    }

    pub(crate) fn apply_builtin_theme(&self, preset_key: &str) {
        self.preview_custom_css(
            preset_css(preset_key).to_string(),
            format!("正在预览 {}。", preset_display_name(preset_key)),
        );
    }

    pub(crate) fn clear_custom_css(&self, success_message: impl Into<String>) {
        self.preview_custom_css(String::new(), success_message);
    }

    pub(crate) fn preview_custom_css(&self, raw: String, message: impl Into<String>) {
        self.invalidate_theme_import();
        if let Err(err) = super::themes::validate_custom_css(&raw) {
            self.set_status(format!("自定义 CSS 格式无效：{err}"), "error");
            return;
        }
        self.set_custom_css(raw.clone());
        self.page.preview_custom_css(raw);
        let mut message = message.into();
        if self.has_unsaved_changes() {
            message.push_str(" 尚未保存。");
        }
        self.set_status(message, "info");
    }

    pub(crate) fn is_theme_preset_active(&self, preset_key: &str) -> bool {
        detect_preset_key(&self.custom_css()) == preset_key
    }

    pub(crate) fn theme_card_state(&self, preset_key: &str) -> &'static str {
        if self.is_theme_preset_active(preset_key) { "active" } else { "inactive" }
    }

    pub(crate) fn theme_apply_button_variant(&self, preset_key: &str) -> &'static str {
        if self.is_theme_preset_active(preset_key) { "primary" } else { "secondary" }
    }

    pub(crate) fn theme_apply_button_label(&self, preset_key: &str) -> &'static str {
        if self.is_theme_preset_active(preset_key) { "当前已选" } else { "预览主题" }
    }

    pub(crate) fn open_repository(&self) {
        self.page.open_repository();
    }

    pub(crate) fn save(&self) {
        self.invalidate_theme_import();
        self.save.save();
    }

    pub(crate) fn has_unsaved_changes(&self) -> bool {
        self.page.has_unsaved_changes()
    }

    pub(crate) fn is_save_pending(&self) -> bool {
        self.save_snapshot.pending_save
    }

    pub(crate) fn save_button_label(&self) -> &'static str {
        if self.is_save_pending() { "正在保存…" } else { "保存设置" }
    }

    pub(crate) fn save_state(&self) -> &'static str {
        if self.is_save_pending() { "pending" } else { "idle" }
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

    pub(crate) fn remote_pull_button_variant(&self) -> &'static str {
        if self.is_remote_pull_pending() { "danger" } else { "secondary" }
    }

    pub(crate) fn remote_pull_button_label(&self) -> &'static str {
        if self.is_remote_pull_pending() { "确认下载并覆盖" } else { "下载配置" }
    }

    pub(crate) fn remote_pull_state(&self) -> &'static str {
        if self.is_remote_pull_pending() { "confirm" } else { "idle" }
    }

    pub(crate) fn push(&self) {
        self.sync.push();
    }

    pub(crate) fn pull(&self) {
        self.sync.pull();
    }
}
