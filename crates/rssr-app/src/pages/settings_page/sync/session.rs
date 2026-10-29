use dioxus::prelude::*;
use std::future::Future;

use super::state::SettingsPageSyncState;
use crate::ui::{
    SettingsCommand, UiCommand, UiIntent, execute_ui_command, spawn_projected_ui_command,
};
use crate::{
    pages::settings_page::{
        intent::SettingsPageIntent, request_lifecycle::SettingsWrite, session::SettingsPageSession,
    },
    status::set_status_info,
};

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
        spawn_projected_ui_command(
            UiCommand::Settings(command),
            UiIntent::into_settings_page_intent,
            move |intent| {
                if let SettingsPageIntent::SetStatus { message, tone } = intent {
                    self.page.set_status(message, tone);
                }
            },
        );
    }

    pub(crate) fn pull(self) {
        self.pull_with(|command| execute_ui_command(UiCommand::Settings(command)));
    }

    fn pull_with<F: Future<Output = Vec<UiIntent>> + 'static>(
        mut self,
        execute: impl FnOnce(SettingsCommand) -> F,
    ) {
        if self.page.reject_pending_write() {
            // A blocked destructive action must require fresh confirmation on retry.
            self.state.with_mut(|state| state.pending_remote_pull = false);
            return;
        }
        let command = self.state.with_mut(SettingsPageSyncState::prepare_pull);
        let Some(command) = command else {
            set_status_info(
                self.page.status_signal(),
                self.page.status_tone_signal(),
                "从 WebDAV 下载配置会覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。",
            );
            return;
        };
        let Some(request) = self.page.begin_write(SettingsWrite::Download) else {
            return;
        };
        self.page.invalidate_theme_import();
        let downloading = execute(command);
        spawn(async move {
            let intents = downloading.await;
            if self.page.complete_write(request) {
                self.page.apply_load_result(intents, request.editing);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeController;
    use rssr_domain::UserSettings;
    use tokio::sync::oneshot;

    fn harness() -> Element {
        let theme = ThemeController { settings: use_signal(UserSettings::default) };
        let page = SettingsPageSession::new(theme);
        let state = use_signal(SettingsPageSyncState::new);
        use_context_provider(|| SettingsPageSyncSession::new(state, page));
        use_context_provider(|| theme);
        rsx! {}
    }

    fn setup() -> (VirtualDom, SettingsPageSyncSession, ThemeController) {
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (session, theme) =
            dom.in_scope(ScopeId::APP, || (consume_context(), consume_context()));
        (dom, session, theme)
    }

    fn no_command(_: SettingsCommand) -> std::future::Ready<Vec<UiIntent>> {
        panic!("confirmation or busy rejection must not construct a command")
    }

    fn start_pull(session: SettingsPageSyncSession) -> oneshot::Sender<Vec<UiIntent>> {
        let (sender, receiver) = oneshot::channel();
        session.pull_with(no_command);
        session.pull_with(|command| {
            assert!(matches!(command, SettingsCommand::PullConfig { .. }));
            assert_eq!(session.page.pending_write(), Some(SettingsWrite::Download));
            assert!(!session.snapshot().pending_remote_pull);
            async move { receiver.await.unwrap() }
        });
        sender
    }

    fn imported() -> Vec<UiIntent> {
        vec![
            UiIntent::SettingsPage(SettingsPageIntent::SettingsLoaded(UserSettings {
                custom_css: "/* downloaded */".into(),
                ..Default::default()
            })),
            UiIntent::SetStatus { message: "downloaded".into(), tone: "info".into() },
        ]
    }

    #[test]
    fn confirmed_download_replaces_existing_draft_but_keeps_later_edits_and_preset_events() {
        for later_edit in [false, true] {
            let (mut dom, session, theme) = setup();
            let sender = dom.in_scope(ScopeId::APP, || {
                session.page.update_draft(|draft| draft.custom_css = "/* existing */".into());
                let sender = start_pull(session);
                if later_edit {
                    session.page.update_draft(|draft| draft.custom_css = "/* temporary */".into());
                    session.page.update_draft(|draft| draft.custom_css = "/* existing */".into());
                }
                session.page.set_preset_choice("newsprint");
                session.page.set_preset_choice("none");
                sender
            });
            sender.send(imported()).unwrap();
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                assert_eq!(
                    session.page.draft().peek().custom_css,
                    if later_edit { "/* existing */" } else { "/* downloaded */" }
                );
                assert_eq!(*session.page.preset_choice().peek(), "none");
                assert_eq!(theme.settings.peek().custom_css, "/* downloaded */");
                assert_eq!(session.page.status().contains("尚未保存"), later_edit);
                assert_eq!(session.page.pending_write(), None);
            });
        }
    }

    #[test]
    fn download_captures_confirmed_target_and_busy_clicks_cannot_queue_another_write() {
        let (mut dom, session, _) = setup();
        let (sender, receiver) = oneshot::channel();
        dom.in_scope(ScopeId::APP, || {
            session.set_endpoint("https://dav.example/selected/".into());
            session.set_remote_path("selected.json".into());
            session.pull_with(no_command);
            session.pull_with(|command| {
                let SettingsCommand::PullConfig { endpoint, remote_path } = command else {
                    panic!("pull")
                };
                assert_eq!(endpoint, "https://dav.example/selected/");
                assert_eq!(remote_path, "selected.json");
                async move { receiver.await.unwrap() }
            });
            session.set_endpoint("https://dav.example/later/".into());
            session.pull_with(no_command);
            session.pull_with(no_command);
            assert!(!session.snapshot().pending_remote_pull);
            assert!(session.page.status().starts_with("配置仍在下载"));
        });
        sender.send(imported()).unwrap();
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(session.snapshot().endpoint, "https://dav.example/later/");
            assert_eq!(session.page.pending_write(), None);
            session.pull_with(no_command);
            assert!(session.snapshot().pending_remote_pull, "retry needs a new confirmation");
        });
    }

    #[test]
    fn pending_save_rejects_download_and_consumes_old_confirmation() {
        let (dom, session, _) = setup();
        dom.in_scope(ScopeId::APP, || {
            session.pull_with(no_command);
            let save = session.page.begin_write(SettingsWrite::Save).unwrap();
            session.pull_with(no_command);
            assert!(!session.snapshot().pending_remote_pull);
            assert!(session.page.status().starts_with("上一份设置仍在保存"));
            assert!(session.page.complete_write(save));
            session.pull_with(no_command);
            assert!(session.snapshot().pending_remote_pull);
        });
    }

    #[test]
    fn confirmed_download_supersedes_old_file_reads_but_allows_a_later_picker() {
        let (dom, session, _) = setup();
        dom.in_scope(ScopeId::APP, || {
            let old_file = session.page.begin_theme_import();
            let _sender = start_pull(session);
            assert!(!session.page.complete_theme_import(old_file));
            let later_file = session.page.begin_theme_import();
            assert!(session.page.is_theme_import_current(later_file));
        });
    }

    #[test]
    fn missing_failed_and_empty_download_results_release_the_gate_without_replacing_drafts() {
        for result in [
            vec![],
            vec![UiIntent::SetStatus { message: "not found".into(), tone: "info".into() }],
            vec![UiIntent::SetStatus { message: "download failed".into(), tone: "error".into() }],
        ] {
            let (mut dom, session, theme) = setup();
            let sender = dom.in_scope(ScopeId::APP, || {
                let sender = start_pull(session);
                session.page.update_draft(|draft| draft.custom_css = "/* later */".into());
                sender
            });
            sender.send(result).unwrap();
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                assert_eq!(session.page.draft().peek().custom_css, "/* later */");
                assert_eq!(*theme.settings.peek(), UserSettings::default());
                assert!(session.page.begin_write(SettingsWrite::Save).is_some());
            });
        }
    }

    #[test]
    fn disposing_the_page_drops_its_pending_download_future() {
        let (mut dom, session, _) = setup();
        let sender = dom.in_scope(ScopeId::APP, || start_pull(session));
        dom.render_immediate_to_vec();
        assert!(!sender.is_closed());
        drop(dom);
        assert!(sender.is_closed());
    }
}
