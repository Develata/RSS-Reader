use std::future::Future;

use dioxus::prelude::*;

use super::state::SettingsPageSyncState;
use crate::pages::settings_page::intent::SettingsPageIntent;
use crate::ui::{AppShellState, SettingsCommand, UiCommand, UiIntent, run_ui_command};
use crate::{pages::settings_page::session::SettingsPageSession, status::set_status_info};

/// WebDAV upload and download: jobs of the app (C4b item 3). They run in the shell's task, so
/// leaving the page does not stop the wait; their results reach this page instance only, while it
/// is open, else the shell shows them; the settings a download stored apply either way, and its
/// end re-reads every page.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SettingsPageSyncSession {
    state: Signal<SettingsPageSyncState>,
    page: SettingsPageSession,
    shell: AppShellState,
}

impl SettingsPageSyncSession {
    pub(crate) fn new(
        state: Signal<SettingsPageSyncState>,
        page: SettingsPageSession,
        shell: AppShellState,
    ) -> Self {
        Self { state, page, shell }
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

    pub(crate) fn push(self) {
        self.push_with(run_ui_command);
    }

    pub(super) fn push_with<F: Future<Output = Vec<UiIntent>> + 'static>(
        mut self,
        execute: impl FnOnce(UiCommand) -> F,
    ) {
        let snapshot = self.snapshot();
        self.state.with_mut(|state| state.clear_pending_remote_pull());
        let pushing = execute(UiCommand::Settings(SettingsCommand::PushConfig {
            endpoint: snapshot.endpoint,
            remote_path: snapshot.remote_path,
        }));
        self.shell.spawn_in_shell(async move {
            let intents = pushing.await;
            self.finish(intents, false);
        });
    }

    pub(crate) fn pull(self) {
        self.pull_with(run_ui_command);
    }

    pub(super) fn pull_with<F: Future<Output = Vec<UiIntent>> + 'static>(
        mut self,
        execute: impl FnOnce(UiCommand) -> F,
    ) {
        let confirmation_required = {
            let mut confirmation_required = false;
            self.state.with_mut(|state| {
                confirmation_required = state.request_remote_pull_confirmation();
            });
            confirmation_required
        };

        if confirmation_required {
            set_status_info(
                self.page.status_signal(),
                self.page.status_tone_signal(),
                "从 WebDAV 下载配置会覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。",
            );
            return;
        }

        // A pull replaces the stored settings: it waits for a save in flight, like a save waits
        // for it (the confirmation stays, so the next click pulls).
        if !self.page.begin_write() {
            set_status_info(
                self.page.status_signal(),
                self.page.status_tone_signal(),
                "设置仍在保存，完成后再从 WebDAV 下载配置。",
            );
            return;
        }
        let snapshot = self.snapshot();
        self.state.with_mut(|state| state.clear_pending_remote_pull());
        let pulling = execute(UiCommand::Settings(SettingsCommand::PullConfig {
            endpoint: snapshot.endpoint,
            remote_path: snapshot.remote_path,
        }));
        self.shell.spawn_in_shell(async move {
            let intents = pulling.await;
            self.finish(intents, true);
        });
    }

    /// A job's end: the page re-reads; the results go to this page instance while it is open,
    /// else the settings a download stored still apply and the shell shows the status.
    fn finish(self, intents: Vec<UiIntent>, pull: bool) {
        self.shell.reread_now();
        let intents = intents.into_iter().filter_map(UiIntent::into_settings_page_intent);
        if self.page.is_open() {
            for intent in intents {
                self.page.dispatch(intent);
            }
            if pull {
                self.page.end_write();
            }
            return;
        }
        let mut notice = None;
        for intent in intents {
            match intent {
                SettingsPageIntent::SettingsLoaded(settings) => self.page.store_theme(settings),
                SettingsPageIntent::SetStatus { message, tone } => notice = Some((message, tone)),
                SettingsPageIntent::Failed(failure) => {
                    notice = Some((failure.message(), failure.tone().to_string()))
                }
            }
        }
        if let Some((message, tone)) = notice {
            self.shell.report_job(message, tone);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use dioxus::core::current_scope_id;
    use rssr_domain::{ThemeMode, UserSettings};
    use tokio::sync::oneshot;

    use super::*;
    use crate::theme::{ThemeController, use_theme_controller};
    use crate::ui::use_app_shell_state;

    #[derive(Clone, Copy)]
    struct Probe {
        show_page: Signal<bool>,
        page: Signal<Option<(SettingsPageSyncSession, ScopeId)>>,
    }

    fn app() -> Element {
        let shell = use_app_shell_state();
        let theme = use_theme_controller();
        let show_page = use_signal(|| true);
        let page = use_signal(|| None);
        use_context_provider(|| shell);
        use_context_provider(|| theme);
        use_context_provider(|| Probe { show_page, page });
        rsx! {
            if show_page() {
                SyncProbe {}
            }
        }
    }

    #[component]
    fn SyncProbe() -> Element {
        let theme = use_context::<ThemeController>();
        let shell = use_context::<AppShellState>();
        let mut probe = use_context::<Probe>();
        let page = SettingsPageSession::new(theme);
        let state = use_signal(SettingsPageSyncState::new);
        use_hook(|| {
            let session = SettingsPageSyncSession::new(state, page, shell);
            probe.page.set(Some((session, current_scope_id())));
        });
        rsx! {}
    }

    fn pulled(theme: ThemeMode) -> Vec<UiIntent> {
        vec![
            UiIntent::SettingsPage(SettingsPageIntent::SettingsLoaded(UserSettings {
                theme,
                ..UserSettings::default()
            })),
            UiIntent::SettingsPage(SettingsPageIntent::SetStatus {
                message: "已从 WebDAV 下载并导入配置。".into(),
                tone: "info".into(),
            }),
        ]
    }

    /// Starts a confirmed download whose answer comes from `receiver`.
    fn start_pull(sync: SettingsPageSyncSession, receiver: oneshot::Receiver<Vec<UiIntent>>) {
        sync.pull_with(|_| -> std::future::Ready<Vec<UiIntent>> {
            panic!("the first click asks to confirm")
        });
        sync.pull_with(move |command| {
            assert!(matches!(command, UiCommand::Settings(SettingsCommand::PullConfig { .. })));
            async move { receiver.await.expect("the download's answer") }
        });
    }

    /// A download ending after the page was left still applies the settings it stored, shows its
    /// result as the shell's notice and re-reads every page (C4b item 3).
    #[tokio::test(flavor = "current_thread")]
    async fn a_download_ending_after_the_page_was_left_still_applies() {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let (probe, shell, theme) = dom.in_scope(ScopeId::APP, || {
            (consume_context::<Probe>(), consume_context::<AppShellState>(), consume_context())
        });
        let theme: ThemeController = theme;
        let (sync, scope) = probe.page.peek().expect("settings page");
        let (sender, receiver) = oneshot::channel();
        dom.in_scope(scope, || start_pull(sync, receiver));
        dom.render_immediate_to_vec();

        let mut show_page = probe.show_page;
        dom.in_scope(ScopeId::APP, || show_page.set(false));
        dom.render_immediate_to_vec();
        assert!(!sync.page.is_open());

        sender.send(pulled(ThemeMode::Dark)).expect("answer");
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.settings.peek().theme, ThemeMode::Dark);
            let notice = shell.job_notice().map(|notice| (notice.message, notice.tone));
            assert_eq!(notice, Some(("已从 WebDAV 下载并导入配置。".into(), "info".into())));
            assert_eq!(shell.library_revision(), 1);
        });
    }

    /// With the page open, the download's results are the page's: its stored settings, its
    /// status, and the write it held ends (the next one may start).
    #[test]
    fn a_download_ending_on_its_page_is_the_pages() {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let (probe, shell) = dom.in_scope(ScopeId::APP, || {
            (consume_context::<Probe>(), consume_context::<AppShellState>())
        });
        let (sync, scope) = probe.page.peek().expect("settings page");
        let (sender, receiver) = oneshot::channel();
        dom.in_scope(scope, || start_pull(sync, receiver));
        dom.render_immediate_to_vec();
        sender.send(pulled(ThemeMode::Dark)).expect("answer");
        dom.render_immediate_to_vec();
        dom.in_scope(scope, || {
            assert_eq!(sync.page.status(), "已从 WebDAV 下载并导入配置。");
            assert_eq!(sync.page.draft().theme, ThemeMode::Dark);
            assert!(sync.page.begin_write(), "the download's write ended");
        });
        dom.in_scope(ScopeId::APP, || assert_eq!(shell.job_notice(), None));
    }
}
