use std::future::Future;

use dioxus::prelude::*;
use rssr_domain::DEFAULT_ENTRIES_PAGE_SIZE;

use super::state::SettingsPageSaveState;
use crate::{
    pages::settings_page::{
        intent::SettingsPageIntent, session::SettingsPageSession, themes::validate_custom_css,
    },
    status::{set_status_error, set_status_info},
    ui::{SettingsCommand, UiCommand, UiIntent, apply_projected_ui_intents, run_ui_command},
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
            run_ui_command(UiCommand::Settings(command))
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
        // Only the edited fields are sent, so a change saved elsewhere meanwhile (an import,
        // another device) is not overwritten; the CSS is checked only when it is edited.
        if let Some(custom_css) = &self.page.pending_edits().custom_css
            && let Err(err) = validate_custom_css(custom_css)
        {
            set_status_error(
                self.page.status_signal(),
                self.page.status_tone_signal(),
                format!("自定义 CSS 格式无效：{err}"),
            );
            return;
        }

        // One settings write at a time: a pull in flight replaces the stored settings.
        if !self.page.begin_write() {
            self.page.set_status(
                "正在从 WebDAV 下载配置；修改已保留在草稿中，完成后请再次保存。",
                "info",
            );
            return;
        }
        let mut state = self.state;
        state.with_mut(|state| state.pending_save = true);

        let status = self.page.status_signal();
        let status_tone = self.page.status_tone_signal();
        let submitted_preset = self.page.preset_choice().peek().clone();
        let mut patch = self.page.submit_edits();
        if patch.entries_page_size == Some(0) {
            patch.entries_page_size = Some(DEFAULT_ENTRIES_PAGE_SIZE);
            success_message.push_str(&format!(
                " 文章页每页数量输入为 0，已回退为默认值 {DEFAULT_ENTRIES_PAGE_SIZE}。"
            ));
        }
        let saving = execute(SettingsCommand::UpdateSettings { patch, success_message });
        spawn(async move {
            let intents = saving.await;
            state.with_mut(|state| state.pending_save = false);
            let mut saved_settings = None;
            let mut status_message = String::new();
            let mut failure_tone = "error";
            apply_projected_ui_intents(intents, UiIntent::into_settings_page_intent, |intent| {
                match intent {
                    SettingsPageIntent::SettingsLoaded(settings) => {
                        saved_settings = Some(settings);
                    }
                    SettingsPageIntent::SetStatus { message, .. } => {
                        status_message = message;
                    }
                    SettingsPageIntent::Failed(failure) => {
                        status_message = failure.message();
                        failure_tone = failure.tone();
                    }
                }
            });
            if let Some(saved_settings) = saved_settings {
                if self.page.apply_saved_settings(saved_settings, &submitted_preset) {
                    status_message.push_str(" 保存期间的新修改仍保留在草稿中，尚未保存。");
                }
                set_status_info(status, status_tone, status_message);
            } else {
                self.page.restore_submitted_edits();
                status_message.push_str(" 草稿已保留，可修改后重试。");
                crate::status::set_status(status, status_tone, status_message, failure_tone);
            }
            self.page.end_write();
        });
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use super::*;
    use crate::theme::ThemeController;
    use rssr_application::SettingsPatch;
    use rssr_domain::{ListDensity, ThemeMode, UserSettings};
    use tokio::sync::oneshot;

    fn harness() -> Element {
        let theme = crate::theme::use_theme_controller();
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
        start_save_over(session, UserSettings::default())
    }

    /// Saves against a store holding `stored`: the reply is the patch applied to it.
    fn start_save_over(
        session: SettingsPageSaveSession,
        stored: UserSettings,
    ) -> oneshot::Sender<bool> {
        let (sender, receiver) = oneshot::channel();
        session.save_with("设置已保存。".into(), |command| async move {
            let SettingsCommand::UpdateSettings { patch, success_message } = command else {
                panic!("expected a settings update")
            };
            if receiver.await.expect("controlled save completion") {
                vec![
                    UiIntent::SettingsPage(SettingsPageIntent::SettingsLoaded(
                        patch.applied(&stored),
                    )),
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
            session.page.edit(|draft| draft.theme = ThemeMode::Dark);
            start_save(session)
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            session.page.edit(|draft| draft.theme = ThemeMode::Light);
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
            assert_eq!(session.page.draft().theme, ThemeMode::Light);
            assert_eq!(session.page.preset_choice().peek().as_str(), "newsprint");
            assert!(session.page.status().contains("尚未保存"));
            assert!(!session.snapshot().pending_save);
        });
        let retry =
            dom.in_scope(ScopeId::APP, || start_save_over(session, theme.settings.peek().clone()));
        retry.send(true).expect("save later draft");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.settings.peek().theme, ThemeMode::Light);
            assert!(!session.page.status().contains("尚未保存"));
        });
    }

    /// A save that fails, of any class, through the real runtime: the draft stays, the save ends,
    /// the status has the class's tone (an unknown outcome is not called a failure) and says the
    /// draft was kept (C4b item 1).
    #[tokio::test(flavor = "current_thread")]
    async fn a_failed_save_keeps_the_draft_whatever_the_class() {
        use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};
        use rssr_api::{BackendError, ErrorCode};

        let classes = [
            (BackendError::new(ErrorCode::Network, "网络中断"), "error", true),
            (BackendError::new(ErrorCode::OutcomeUnknown, "结果未知"), "unknown", false),
            (BackendError::new(ErrorCode::Unauthenticated, "未登录"), "error", true),
            (BackendError::new(ErrorCode::BuildMismatch, "版本不一致"), "error", true),
        ];
        for (error, tone, called_failed) in classes {
            let backend = Scripted::over_new_library();
            backend.fail("update_settings", error.clone());
            let _host = test_host::install(backend.clone());
            let (mut dom, session, theme) = setup();
            dom.in_scope(ScopeId::APP, || {
                session.page.edit(|draft| draft.theme = ThemeMode::Dark);
                session.save_with("设置已保存。".into(), |command| {
                    crate::ui::execute_ui_command(UiCommand::Settings(command))
                });
            });
            for _ in 0..200 {
                let wait = dom.wait_for_work();
                let _ = tokio::time::timeout(std::time::Duration::from_millis(20), wait).await;
                dom.render_immediate_to_vec();
                if !dom.in_scope(ScopeId::APP, || session.snapshot().pending_save) {
                    break;
                }
            }
            dom.in_scope(ScopeId::APP, || {
                let status = session.page.status();
                assert!(!session.snapshot().pending_save, "{}: the save ended", error.message);
                assert_eq!(session.page.draft().theme, ThemeMode::Dark, "{}", error.message);
                assert_eq!(theme.settings.peek().theme, UserSettings::default().theme);
                assert_eq!(session.page.status_tone(), tone, "{}", error.message);
                assert!(
                    status.contains(&error.message) && status.contains("草稿已保留"),
                    "{status}"
                );
                assert_eq!(status.contains("失败"), called_failed, "{status}");
            });
            assert_eq!(backend.calls("update_settings"), 1);
        }
    }

    #[test]
    fn failed_save_preserves_draft_and_can_be_retried() {
        let (mut dom, session, theme) = setup();
        let sender = dom.in_scope(ScopeId::APP, || {
            session.page.edit(|draft| draft.theme = ThemeMode::Dark);
            start_save(session)
        });
        dom.render_immediate_to_vec();
        sender.send(false).expect("fail persistence");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.page.draft().theme, ThemeMode::Dark);
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
            session.page.edit(|draft| draft.entries_page_size = 0);
            start_save(session)
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            session.page.preset_choice().set("atlas-sidebar".into());
        });
        sender.send(true).expect("persist normalized value");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.page.draft().entries_page_size, DEFAULT_ENTRIES_PAGE_SIZE);
            assert_eq!(theme.settings.peek().entries_page_size, DEFAULT_ENTRIES_PAGE_SIZE);
            assert_eq!(session.page.preset_choice().peek().as_str(), "atlas-sidebar");
            assert!(!session.page.status().contains("尚未保存"));
        });
    }

    /// Only edited fields are sent: a field changed elsewhere since the page loaded (here the
    /// refresh interval) survives the save and reaches the draft, while an edit made during the
    /// save stays in the draft.
    #[test]
    fn save_sends_only_edited_fields_and_takes_the_rest_from_the_store() {
        let (mut dom, session, theme) = setup();
        let stored_elsewhere =
            UserSettings { refresh_interval_minutes: 90, ..UserSettings::default() };
        let sender = dom.in_scope(ScopeId::APP, || {
            session.page.edit(|draft| draft.theme = ThemeMode::Dark);
            let (sender, receiver) = oneshot::channel::<()>();
            let stored = stored_elsewhere.clone();
            session.save_with("设置已保存。".into(), move |command| async move {
                let SettingsCommand::UpdateSettings { patch, success_message } = command else {
                    panic!("expected a settings update")
                };
                assert_eq!(
                    patch,
                    rssr_application::SettingsPatch {
                        theme: Some(ThemeMode::Dark),
                        ..Default::default()
                    }
                );
                receiver.await.expect("controlled save completion");
                vec![
                    UiIntent::SettingsPage(SettingsPageIntent::SettingsLoaded(
                        patch.applied(&stored),
                    )),
                    UiIntent::SettingsPage(SettingsPageIntent::SetStatus {
                        message: success_message,
                        tone: "info".into(),
                    }),
                ]
            });
            sender
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            session.page.edit(|draft| draft.list_density = ListDensity::Compact)
        });
        sender.send(()).expect("complete save");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let saved = theme.settings.peek().clone();
            assert_eq!((saved.theme, saved.refresh_interval_minutes), (ThemeMode::Dark, 90));
            let draft = session.page.draft();
            assert_eq!(draft.refresh_interval_minutes, 90, "the stored field reaches the draft");
            assert_eq!(draft.list_density, ListDensity::Compact, "the later edit stays");
            assert!(session.page.status().contains("尚未保存"));
        });
    }

    /// Issues the page's load; the reply is whatever the returned sender delivers.
    fn start_load(session: SettingsPageSaveSession) -> oneshot::Sender<UserSettings> {
        let (sender, receiver) = oneshot::channel();
        session.page.load_with(|command| async move {
            assert!(matches!(command, SettingsCommand::Load));
            let settings = receiver.await.expect("controlled load completion");
            vec![UiIntent::SettingsPage(SettingsPageIntent::SettingsLoaded(settings))]
        });
        sender
    }

    /// The patch a save of the current draft would send.
    fn submitted_patch(session: SettingsPageSaveSession) -> SettingsPatch {
        let seen = Rc::new(std::cell::RefCell::new(None));
        let sink = Rc::clone(&seen);
        session.save_with("设置已保存。".into(), move |command| {
            let SettingsCommand::UpdateSettings { patch, .. } = command else {
                panic!("expected a settings update")
            };
            *sink.borrow_mut() = Some(patch);
            std::future::ready(Vec::new())
        });
        seen.take().expect("the save was submitted")
    }

    fn customized() -> UserSettings {
        UserSettings {
            theme: ThemeMode::Dark,
            refresh_interval_minutes: 90,
            custom_css: "body { color: red; }".into(),
            ..UserSettings::default()
        }
    }

    /// Opening the page before startup has read the settings: the app-wide settings arrive
    /// first, then the page's own load. Saving without an edit sends nothing (B2-D4); measuring
    /// the default draft against the app-wide settings would send the defaults over the stored
    /// theme, interval and CSS.
    #[test]
    fn a_cold_start_save_without_edits_sends_nothing() {
        let (mut dom, session, theme) = setup();
        let load = dom.in_scope(ScopeId::APP, || start_load(session));
        dom.in_scope(ScopeId::APP, || {
            let mut app_wide = theme.settings;
            app_wide.set(customized());
        });
        load.send(customized()).expect("deliver the load");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.page.draft(), customized());
            assert_eq!(submitted_patch(session), SettingsPatch::default());
        });
    }

    /// Edits made while the load is in flight stay; the other fields come from the store.
    #[test]
    fn a_load_reply_keeps_the_edits_made_while_it_was_in_flight() {
        let (mut dom, session, theme) = setup();
        let load = dom.in_scope(ScopeId::APP, || start_load(session));
        dom.in_scope(ScopeId::APP, || {
            session.page.edit(|draft| draft.list_density = ListDensity::Compact);
        });
        load.send(customized()).expect("deliver the load");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            let draft = session.page.draft();
            assert_eq!(draft, UserSettings { list_density: ListDensity::Compact, ..customized() });
            assert_eq!(*theme.settings.peek(), customized());
            assert_eq!(
                submitted_patch(session),
                SettingsPatch { list_density: Some(ListDensity::Compact), ..Default::default() }
            );
        });
    }

    /// A load that overlaps a save may have read the store before the save; whether its reply
    /// arrives during the save or after it, the save's reply is what the page keeps (B2-D4).
    #[test]
    fn a_load_that_overlaps_a_save_is_discarded() {
        for reply_after_the_save in [false, true] {
            let (mut dom, session, theme) = setup();
            let load = dom.in_scope(ScopeId::APP, || start_load(session));
            let save = dom.in_scope(ScopeId::APP, || {
                session.page.edit(|draft| draft.theme = ThemeMode::Dark);
                start_save(session)
            });
            dom.render_immediate_to_vec();
            if reply_after_the_save {
                save.send(true).expect("complete the save");
                dom.render_immediate_to_vec();
                load.send(customized()).expect("deliver the load");
            } else {
                load.send(customized()).expect("deliver the load");
                dom.render_immediate_to_vec();
                save.send(true).expect("complete the save");
            }
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                let saved = UserSettings { theme: ThemeMode::Dark, ..UserSettings::default() };
                assert_eq!(*theme.settings.peek(), saved, "after the save: {reply_after_the_save}");
                assert_eq!(session.page.draft(), saved);
                assert_eq!(submitted_patch(session), SettingsPatch::default());
            });
        }
    }

    /// Changing a field back while its save is pending is an edit like any other: it stays in
    /// the draft over the saved value, even with a load in flight.
    #[test]
    fn an_edit_reverted_during_its_save_stays_in_the_draft() {
        let (mut dom, session, theme) = setup();
        let load = dom.in_scope(ScopeId::APP, || start_load(session));
        let save = dom.in_scope(ScopeId::APP, || {
            session.page.edit(|draft| draft.theme = ThemeMode::Dark);
            start_save(session)
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || session.page.edit(|draft| draft.theme = ThemeMode::System));
        load.send(UserSettings { theme: ThemeMode::Light, ..UserSettings::default() })
            .expect("deliver the load");
        dom.render_immediate_to_vec();
        save.send(true).expect("complete the save");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.settings.peek().theme, ThemeMode::Dark);
            assert_eq!(session.page.draft().theme, ThemeMode::System);
            assert!(session.page.status().contains("尚未保存"));
            assert_eq!(
                submitted_patch(session),
                SettingsPatch { theme: Some(ThemeMode::System), ..Default::default() }
            );
        });
    }

    /// A load overlapped by a save that stores nothing (here it fails) still applies, whether
    /// it is answered during the save or after it: no write superseded the read, and nothing
    /// else would show the stored settings.
    #[test]
    fn a_load_overlapped_by_a_failed_save_still_applies() {
        for reply_after_the_save in [false, true] {
            let (mut dom, session, theme) = setup();
            let load = dom.in_scope(ScopeId::APP, || start_load(session));
            let save = dom.in_scope(ScopeId::APP, || {
                session.page.edit(|draft| draft.list_density = ListDensity::Compact);
                start_save(session)
            });
            dom.render_immediate_to_vec();
            if reply_after_the_save {
                save.send(false).expect("fail the save");
                dom.render_immediate_to_vec();
                load.send(customized()).expect("deliver the load");
            } else {
                load.send(customized()).expect("deliver the load");
                dom.render_immediate_to_vec();
                save.send(false).expect("fail the save");
            }
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                let edited = UserSettings { list_density: ListDensity::Compact, ..customized() };
                assert_eq!(session.page.draft(), edited, "after: {reply_after_the_save}");
                assert_eq!(*theme.settings.peek(), customized());
                assert_eq!(
                    submitted_patch(session),
                    SettingsPatch {
                        list_density: Some(ListDensity::Compact),
                        ..Default::default()
                    }
                );
            });
        }
    }

    /// One settings write at a time: a save while a pull is in flight is not sent, so the two
    /// replies are never merged against a baseline the other one changed.
    #[test]
    fn a_save_is_not_sent_while_a_pull_is_in_flight() {
        let (dom, session, _theme) = setup();
        dom.in_scope(ScopeId::APP, || {
            let _ = session.page.begin_write();
            session.page.edit(|draft| draft.theme = ThemeMode::Dark);
            let called = Rc::new(Cell::new(false));
            let check = Rc::clone(&called);
            session.save_with("设置已保存。".into(), move |_| {
                called.set(true);
                std::future::ready(Vec::new())
            });
            assert!(!check.get(), "the save must wait for the pull");
            assert!(!session.snapshot().pending_save);
            assert!(session.page.status().contains("WebDAV"));

            // The pull stores other settings; the draft keeps its edit, which a save now sends.
            let pulled = UserSettings { refresh_interval_minutes: 45, ..UserSettings::default() };
            session.page.dispatch(SettingsPageIntent::SettingsLoaded(pulled.clone()));
            session.page.end_write();
            assert_eq!(session.page.draft(), UserSettings { theme: ThemeMode::Dark, ..pulled });
            assert_eq!(
                submitted_patch(session),
                SettingsPatch { theme: Some(ThemeMode::Dark), ..Default::default() }
            );
        });
    }

    /// A pull waits for a save in flight, as a save waits for a pull.
    #[test]
    fn a_pull_cannot_start_while_a_save_is_in_flight() {
        let (mut dom, session, theme) = setup();
        let save = dom.in_scope(ScopeId::APP, || {
            session.page.edit(|draft| draft.theme = ThemeMode::Dark);
            start_save(session)
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || assert!(!session.page.begin_write()));
        save.send(true).expect("complete the save");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.settings.peek().theme, ThemeMode::Dark);
            assert!(session.page.begin_write(), "the save has ended");
        });
    }

    /// A load overlapped by a pull that finds no remote configuration applies when the pull
    /// ends; one overlapped by a pull that stores settings is dropped.
    #[test]
    fn a_load_overlapped_by_a_pull_applies_only_if_the_pull_stored_nothing() {
        for pull_stores in [false, true] {
            let (mut dom, session, _theme) = setup();
            let load = dom.in_scope(ScopeId::APP, || start_load(session));
            dom.in_scope(ScopeId::APP, || assert!(session.page.begin_write()));
            load.send(customized()).expect("deliver the load");
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                assert_eq!(session.page.draft(), UserSettings::default(), "held");
                let pulled =
                    UserSettings { refresh_interval_minutes: 45, ..UserSettings::default() };
                if pull_stores {
                    session.page.dispatch(SettingsPageIntent::SettingsLoaded(pulled.clone()));
                } else {
                    session.page.dispatch(SettingsPageIntent::SetStatus {
                        message: "远端配置不存在。".into(),
                        tone: "info".into(),
                    });
                }
                session.page.end_write();
                let expected = if pull_stores { pulled } else { customized() };
                assert_eq!(session.page.draft(), expected, "pull stores: {pull_stores}");
                assert_eq!(submitted_patch(session), SettingsPatch::default());
            });
        }
    }

    /// An edit reverted during a save that then fails is still an edit: the load the save
    /// overlapped (answered during the save or after it) takes the stored value only for the
    /// untouched fields.
    #[test]
    fn an_edit_reverted_during_a_failed_save_survives_the_overlapped_load() {
        for reply_after_the_save in [false, true] {
            let (mut dom, session, theme) = setup();
            let load = dom.in_scope(ScopeId::APP, || start_load(session));
            let save = dom.in_scope(ScopeId::APP, || {
                session.page.edit(|draft| draft.theme = ThemeMode::Dark);
                start_save(session)
            });
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                session.page.edit(|draft| draft.theme = ThemeMode::System)
            });
            let stored = UserSettings { theme: ThemeMode::Light, ..customized() };
            if reply_after_the_save {
                save.send(false).expect("fail the save");
                dom.render_immediate_to_vec();
                load.send(stored.clone()).expect("deliver the load");
            } else {
                load.send(stored.clone()).expect("deliver the load");
                dom.render_immediate_to_vec();
                save.send(false).expect("fail the save");
            }
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                assert_eq!(*theme.settings.peek(), stored, "after: {reply_after_the_save}");
                assert_eq!(
                    session.page.draft(),
                    UserSettings { theme: ThemeMode::System, ..stored.clone() },
                    "after: {reply_after_the_save}"
                );
                assert_eq!(
                    submitted_patch(session),
                    SettingsPatch { theme: Some(ThemeMode::System), ..Default::default() }
                );
            });
        }
    }

    /// Edits are tracked, not inferred: a field set back to the value it had is still an edit
    /// and stays over a load that brings another stored value; a field never set takes it.
    #[test]
    fn a_field_set_back_to_its_value_is_still_an_edit() {
        let (mut dom, session, _theme) = setup();
        let load = dom.in_scope(ScopeId::APP, || start_load(session));
        dom.in_scope(ScopeId::APP, || {
            session.page.edit(|draft| draft.theme = ThemeMode::Dark);
            session.page.edit(|draft| draft.theme = ThemeMode::System);
            session.page.edit(|draft| draft.list_density = ListDensity::Comfortable);
        });
        load.send(UserSettings { theme: ThemeMode::Light, ..customized() }).expect("load");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(
                session.page.draft(),
                UserSettings { theme: ThemeMode::System, ..customized() }
            );
            assert_eq!(
                submitted_patch(session),
                SettingsPatch { theme: Some(ThemeMode::System), ..Default::default() },
                "setting a field to the value it shows is no edit"
            );
        });
    }

    /// An edit made during a save stays pending even when its value is the one the save stored
    /// (changed away and back): the status says so, as the edit is still sent by the next save.
    #[test]
    fn an_edit_matching_the_saved_value_is_still_reported_unsaved() {
        let (mut dom, session, theme) = setup();
        let save = dom.in_scope(ScopeId::APP, || {
            session.page.edit(|draft| draft.theme = ThemeMode::Dark);
            start_save(session)
        });
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            session.page.edit(|draft| draft.theme = ThemeMode::Light);
            session.page.edit(|draft| draft.theme = ThemeMode::Dark);
        });
        save.send(true).expect("complete the save");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.settings.peek().theme, ThemeMode::Dark);
            assert_eq!(session.page.draft().theme, ThemeMode::Dark);
            assert!(session.page.status().contains("尚未保存"), "{}", session.page.status());
            assert_eq!(
                submitted_patch(session),
                SettingsPatch { theme: Some(ThemeMode::Dark), ..Default::default() }
            );
        });
    }
}
