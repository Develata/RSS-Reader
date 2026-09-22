use std::future::Future;

use dioxus::prelude::*;
use rssr_domain::DEFAULT_ENTRIES_PAGE_SIZE;

use super::state::SettingsPageSaveState;
use crate::{
    pages::settings_page::{
        intent::SettingsPageIntent, session::SettingsPageSession, themes::validate_custom_css,
    },
    status::{set_status_error, set_status_info},
    ui::{SettingsCommand, UiCommand, UiIntent, apply_projected_ui_intents, execute_ui_command},
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SettingsPageSaveSession {
    state: Signal<SettingsPageSaveState>,
    page: SettingsPageSession,
}

impl SettingsPageSaveSession {
    pub(crate) fn new(state: Signal<SettingsPageSaveState>, page: SettingsPageSession) -> Self {
        Self { state, page }
    }

    pub(crate) fn snapshot(self) -> SettingsPageSaveState {
        (self.state)()
    }

    pub(crate) fn save(self) {
        self.save_with_message("设置已保存。");
    }

    pub(crate) fn save_with_message(self, success_message: impl Into<String>) {
        self.save_with(success_message.into(), |command| {
            execute_ui_command(UiCommand::Settings(command))
        });
    }

    fn save_with<F: Future<Output = Vec<UiIntent>> + 'static>(
        self,
        mut success_message: String,
        execute: impl FnOnce(SettingsCommand) -> F,
    ) {
        // Every entry point, including theme preset buttons, passes this synchronous gate.
        if self.state.peek().pending_save {
            self.page
                .set_status("上一份设置仍在保存；新修改已保留在草稿中，完成后请再次保存。", "info");
            return;
        }
        let submitted_draft = self.page.draft().peek().clone();
        let submitted_preset = self.page.preset_choice().peek().clone();
        let mut next = submitted_draft.clone();
        if let Err(err) = validate_custom_css(&next.custom_css) {
            set_status_error(
                self.page.status_signal(),
                self.page.status_tone_signal(),
                format!("自定义 CSS 格式无效：{err}"),
            );
            return;
        }

        let mut state = self.state;
        state.with_mut(|state| state.pending_save = true);

        let status = self.page.status_signal();
        let status_tone = self.page.status_tone_signal();
        if next.entries_page_size == 0 {
            next.entries_page_size = DEFAULT_ENTRIES_PAGE_SIZE;
            success_message.push_str(&format!(
                " 文章页每页数量输入为 0，已回退为默认值 {DEFAULT_ENTRIES_PAGE_SIZE}。"
            ));
        }

        let saving = execute(SettingsCommand::SaveAppearance { settings: next, success_message });
        spawn(async move {
            let intents = saving.await;
            state.with_mut(|state| state.pending_save = false);
            let mut saved_settings = None;
            let mut status_message = String::new();
            apply_projected_ui_intents(intents, UiIntent::into_settings_page_intent, |intent| {
                match intent {
                    SettingsPageIntent::SettingsLoaded(settings) => {
                        saved_settings = Some(settings);
                    }
                    SettingsPageIntent::SetStatus { message, .. } => {
                        status_message = message;
                    }
                }
            });
            if let Some(saved_settings) = saved_settings {
                if self.page.apply_saved_settings(
                    saved_settings,
                    &submitted_draft,
                    &submitted_preset,
                ) {
                    status_message.push_str(" 保存期间的新修改仍保留在草稿中，尚未保存。");
                }
                set_status_info(status, status_tone, status_message);
            } else {
                status_message.push_str(" 草稿已保留，可修改后重试。");
                set_status_error(status, status_tone, status_message);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use super::*;
    use crate::theme::ThemeController;
    use rssr_domain::{ThemeMode, UserSettings};
    use tokio::sync::oneshot;

    fn harness() -> Element {
        let theme = ThemeController { settings: use_signal(UserSettings::default) };
        let page = SettingsPageSession::new(theme);
        let state = use_signal(SettingsPageSaveState::new);
        use_context_provider(|| SettingsPageSaveSession::new(state, page));
        use_context_provider(|| theme);
        rsx! {}
    }

    fn setup() -> (VirtualDom, SettingsPageSaveSession, ThemeController) {
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (session, theme) =
            dom.in_scope(ScopeId::APP, || (consume_context(), consume_context()));
        (dom, session, theme)
    }

    fn start_save(session: SettingsPageSaveSession) -> oneshot::Sender<bool> {
        let (sender, receiver) = oneshot::channel();
        session.save_with("设置已保存。".into(), |command| async move {
            let SettingsCommand::SaveAppearance { settings, success_message } = command else {
                panic!("expected appearance save")
            };
            if receiver.await.expect("controlled save completion") {
                vec![
                    UiIntent::SettingsPage(SettingsPageIntent::SettingsLoaded(settings)),
                    UiIntent::SettingsPage(SettingsPageIntent::SetStatus {
                        message: success_message,
                        tone: "info".into(),
                    }),
                ]
            } else {
                vec![UiIntent::SettingsPage(SettingsPageIntent::SetStatus {
                    message: "保存设置失败：test failure".into(),
                    tone: "error".into(),
                })]
            }
        });
        sender
    }

    #[test]
    fn save_keeps_later_edits_and_synchronously_gates_duplicate_requests() {
        let (mut dom, session, theme) = setup();
        let sender = dom.in_scope(ScopeId::APP, || {
            session.page.draft().write().theme = ThemeMode::Dark;
            start_save(session)
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            session.page.draft().write().theme = ThemeMode::Light;
            session.page.preset_choice().set("newsprint".into());
            let called = Rc::new(Cell::new(false));
            let check = Rc::clone(&called);
            session.save_with("second save".into(), move |_| {
                called.set(true);
                std::future::ready(Vec::new())
            });
            assert!(!check.get(), "a second persistence command must not even be constructed");
            assert!(session.snapshot().pending_save);
        });
        sender.send(true).expect("complete first save");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.settings.peek().theme, ThemeMode::Dark);
            assert_eq!(session.page.draft().peek().theme, ThemeMode::Light);
            assert_eq!(session.page.preset_choice().peek().as_str(), "newsprint");
            assert!(session.page.status().contains("尚未保存"));
            assert!(!session.snapshot().pending_save);
        });
        let retry = dom.in_scope(ScopeId::APP, || start_save(session));
        retry.send(true).expect("save later draft");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.settings.peek().theme, ThemeMode::Light);
            assert!(!session.page.status().contains("尚未保存"));
        });
    }

    #[test]
    fn failed_save_preserves_draft_and_can_be_retried() {
        let (mut dom, session, theme) = setup();
        let sender = dom.in_scope(ScopeId::APP, || {
            session.page.draft().write().theme = ThemeMode::Dark;
            start_save(session)
        });
        dom.render_immediate_to_vec();
        sender.send(false).expect("fail persistence");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.page.draft().peek().theme, ThemeMode::Dark);
            assert_eq!(theme.settings.peek().theme, UserSettings::default().theme);
            assert_eq!(session.page.status_tone(), "error");
            assert!(session.page.status().contains("草稿已保留"));
            assert!(!session.snapshot().pending_save);
        });
        let retry = dom.in_scope(ScopeId::APP, || start_save(session));
        retry.send(true).expect("retry persistence");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.settings.peek().theme, ThemeMode::Dark);
            assert_eq!(session.page.status_tone(), "info");
        });
    }

    #[test]
    fn unedited_submission_receives_normalized_value_without_losing_preset_selection() {
        let (mut dom, session, theme) = setup();
        let sender = dom.in_scope(ScopeId::APP, || {
            session.page.draft().write().entries_page_size = 0;
            start_save(session)
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            session.page.preset_choice().set("atlas-sidebar".into());
        });
        sender.send(true).expect("persist normalized value");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.page.draft().peek().entries_page_size, DEFAULT_ENTRIES_PAGE_SIZE);
            assert_eq!(theme.settings.peek().entries_page_size, DEFAULT_ENTRIES_PAGE_SIZE);
            assert_eq!(session.page.preset_choice().peek().as_str(), "atlas-sidebar");
            assert!(!session.page.status().contains("尚未保存"));
        });
    }
}
