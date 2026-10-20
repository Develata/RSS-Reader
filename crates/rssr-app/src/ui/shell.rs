use dioxus::core::{Runtime, current_scope_id};
use dioxus::prelude::*;
use dioxus_router::Navigator;

use crate::{
    router::AppRoute,
    status::set_status_error,
    theme::ThemeController,
    ui::failure::{TONE_UNKNOWN, UiFailure},
    ui::shell_browser::wait_for_refresh_feedback,
    ui::shell_prefs::{initial_entry_search, remember_entry_search},
    ui::shell_state::{HomeAction, ManualRefreshState, NavMode, resolve_home_action},
    ui::{ShellCommand, UiCommand, UiIntent, run_ui_command, visit_ui_command},
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct AppShellState {
    entry_search: Signal<String>,
    nav_mode: Signal<NavMode>,
    refresh: Signal<ManualRefreshState>,
    refresh_revision: Signal<u64>,
    /// Why the settings could not be read at start, until they are (C4b item 5).
    settings_failure: Signal<Option<UiFailure>>,
    settings_loading: Signal<bool>,
    owner: ScopeId,
}

impl AppShellState {
    pub(crate) fn entry_search(self) -> String {
        (self.entry_search)()
    }

    pub(crate) fn set_entry_search(mut self, value: String) {
        remember_entry_search(&value);
        self.entry_search.set(value);
    }

    pub(crate) fn refresh_state(self) -> ManualRefreshState {
        self.refresh.read().clone()
    }
    pub(crate) fn refresh_revision(self) -> u64 {
        *self.refresh_revision.read()
    }

    /// All manual refresh triggers use this synchronous gate and the App-owned task.
    /// Completion invalidates list snapshots only; Reader never subscribes to this revision.
    pub(crate) fn manual_refresh(mut self) {
        if !self.refresh.write().begin() {
            return;
        }
        Runtime::current().in_scope(self.owner, || {
            spawn(async move {
                let intents =
                    crate::ui::run_ui_command(UiCommand::Shell(ShellCommand::ManualRefresh)).await;
                let (message, tone) = intents
                    .into_iter()
                    .rev()
                    .find_map(UiIntent::into_status)
                    .unwrap_or_else(|| ("刷新未返回结果。".to_string(), "error".to_string()));
                // A failure, or an outcome that may have happened, stays longer.
                let failed = tone == "error" || tone == TONE_UNKNOWN;
                self.refresh.set(ManualRefreshState::Finished { message, tone });
                // Failed batches can still have committed some subscriptions.
                self.refresh_revision += 1;
                let completed_revision = *self.refresh_revision.peek();
                spawn(async move {
                    wait_for_refresh_feedback(std::time::Duration::from_secs(if failed {
                        6
                    } else {
                        3
                    }))
                    .await;
                    self.refresh
                        .write()
                        .dismiss_if_current(*self.refresh_revision.peek(), completed_revision);
                });
            })
        });
    }

    /// Reads the settings the shell applies (at start, and again on retry). The failure stays
    /// shown until settings from the library are applied, by this read or by a page (the settings
    /// page reads them, a configuration import replaces them); a read issued before such an
    /// application does not overwrite it (C4b items 2 and 5; design review C4D-R2 R2-03, R2-04).
    pub(crate) fn load_settings(mut self, theme: ThemeController) {
        if *self.settings_loading.peek() {
            return;
        }
        self.settings_loading.set(true);
        let issued = theme.stored_count();
        Runtime::current().in_scope(self.owner, || {
            spawn(async move {
                let intents =
                    run_ui_command(UiCommand::Shell(ShellCommand::LoadAuthenticatedShell)).await;
                self.settings_loading.set(false);
                for intent in intents {
                    match intent {
                        UiIntent::AuthenticatedShellLoaded(snapshot)
                            if theme.stored_count() == issued =>
                        {
                            theme.store(snapshot.settings);
                        }
                        UiIntent::Failed(failure) if theme.stored_count() == 0 => {
                            self.settings_failure.set(Some(failure));
                        }
                        _ => {}
                    }
                }
                if theme.stored_count() > 0 {
                    self.settings_failure.set(None);
                }
            });
        });
    }

    /// Why the settings could not be read, while they have not been: settings applied from the
    /// library by any page end it, not only this read (design review C4D-R2 R2-04).
    pub(crate) fn settings_failure(self, theme: ThemeController) -> Option<UiFailure> {
        if theme.has_stored() {
            return None;
        }
        self.settings_failure.read().clone()
    }

    pub(crate) fn settings_loading(self) -> bool {
        *self.settings_loading.read()
    }

    pub(crate) fn submit_search(self, navigator: Navigator) {
        navigator.push(AppRoute::EntriesPage {});
    }

    /// 在 App 作用域里运行任务，页面卸载不会取消它。只给完成后还要改全局状态的命令用
    /// （例如配置导入替换了设置，审计 E1）；页面自己的回执要先确认页面仍然挂载。
    pub(crate) fn spawn_in_shell(self, task: impl std::future::Future<Output = ()> + 'static) {
        Runtime::current().in_scope(self.owner, || {
            spawn(task);
        });
    }
}

pub(crate) fn use_app_shell_state() -> AppShellState {
    AppShellState {
        entry_search: use_signal(initial_entry_search),
        nav_mode: use_signal(NavMode::default),
        refresh: use_signal(ManualRefreshState::default),
        refresh_revision: use_signal(|| 0),
        settings_failure: use_signal(|| None),
        settings_loading: use_signal(|| false),
        owner: current_scope_id(),
    }
}

#[derive(Clone)]
pub(crate) struct AppNavShell {
    shell: AppShellState,
    navigator: Navigator,
    route: AppRoute,
}

impl AppNavShell {
    pub(crate) fn is_search(&self) -> bool {
        *self.shell.nav_mode.read() == NavMode::Search
    }
    pub(crate) fn nav_state(&self) -> &'static str {
        match *self.shell.nav_mode.read() {
            NavMode::Normal => "normal",
            NavMode::Search => "search",
            NavMode::Collapsed => "collapsed",
        }
    }
    pub(crate) fn is_collapsed(&self) -> bool {
        *self.shell.nav_mode.read() == NavMode::Collapsed
    }
    pub(crate) fn toggle_collapsed(&mut self) {
        let mode = self.shell.nav_mode.peek().toggle_collapsed();
        self.shell.nav_mode.set(mode);
    }
    pub(crate) fn is_reader(&self) -> bool {
        matches!(self.route, AppRoute::ReaderPage { .. })
    }
    pub(crate) fn entry_search(&self) -> String {
        self.shell.entry_search()
    }
    pub(crate) fn set_entry_search(&self, value: String) {
        self.shell.set_entry_search(value);
    }
    pub(crate) fn refresh_state(&self) -> ManualRefreshState {
        self.shell.refresh_state()
    }

    pub(crate) fn toggle_search(&mut self) {
        let mode = self.shell.nav_mode.peek().toggle_search();
        self.shell.nav_mode.set(mode);
    }
    pub(crate) fn close_search(&mut self) {
        let mode = self.shell.nav_mode.peek().close_search();
        self.shell.nav_mode.set(mode);
    }
    pub(crate) fn submit_search(&self) {
        self.shell.submit_search(self.navigator);
    }
    pub(crate) fn activate_home(&self) {
        match resolve_home_action(&self.route) {
            HomeAction::Navigate => {
                self.navigator.push(AppRoute::EntriesPage {});
            }
            HomeAction::ManualRefresh => self.shell.manual_refresh(),
        }
    }
}

pub(crate) fn use_app_nav_shell() -> AppNavShell {
    AppNavShell {
        shell: use_context::<AppShellState>(),
        navigator: use_navigator(),
        route: use_route::<AppRoute>(),
    }
}

/// Loads the shell's settings once at start ([`AppShellState::load_settings`]). The Web page is
/// served only after the server's login, so the page itself has no gate (phase C, C4a).
pub(crate) fn use_authenticated_shell_bus(shell: AppShellState, theme: ThemeController) {
    use_hook(move || shell.load_settings(theme));
}

pub(crate) fn use_startup_route_bus(
    navigator: Navigator,
    mut status: Signal<String>,
    mut status_tone: Signal<String>,
) {
    use_resource(move || async move {
        visit_ui_command(UiCommand::Shell(ShellCommand::ResolveStartupRoute), |intent| {
            if let Some(snapshot) = intent.clone().into_startup_route_resolved() {
                let _ = navigator.replace(snapshot.route);
                return;
            }
            if let Some((message, tone)) = intent.into_status() {
                if tone == "error" {
                    set_status_error(status, status_tone, message);
                } else {
                    status_tone.set(tone);
                    status.set(message);
                }
            }
        })
        .await;
    });
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::time::Duration;

    use dioxus::prelude::*;
    use rssr_api::{BackendError, ErrorCode};
    use rssr_domain::{ThemeMode, UserSettings};

    use super::{AppShellState, use_app_shell_state, use_authenticated_shell_bus};
    use crate::theme::{ThemeController, use_theme_controller};
    use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

    fn harness() -> Element {
        let theme = use_theme_controller();
        let shell = use_app_shell_state();
        use_context_provider(|| theme);
        use_context_provider(|| shell);
        use_authenticated_shell_bus(shell, theme);
        rsx! {}
    }

    fn contexts(dom: &VirtualDom) -> (AppShellState, ThemeController) {
        dom.in_scope(ScopeId::APP, || (consume_context(), consume_context()))
    }

    /// Runs the app's tasks until the shell's settings read has ended.
    async fn settle(dom: &mut VirtualDom, shell: AppShellState) {
        for _ in 0..200 {
            let _ = tokio::time::timeout(Duration::from_millis(20), dom.wait_for_work()).await;
            dom.render_immediate_to_vec();
            if !dom.in_scope(ScopeId::APP, || shell.settings_loading()) {
                return;
            }
        }
        panic!("the settings read did not end");
    }

    /// Settings that cannot be read at start are a visible failure, not silent defaults; it stays
    /// until settings are read, and a retry that reads them clears it (C4b item 5).
    #[tokio::test(flavor = "current_thread")]
    async fn a_failed_settings_read_stays_until_settings_are_read() {
        let backend = Scripted::over_new_library();
        backend.fail("load_settings", BackendError::new(ErrorCode::Network, "网络中断"));
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, theme) = contexts(&dom);
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            let failure = shell.settings_failure(theme).expect("a visible failure");
            assert_eq!(failure.message(), "读取设置失败：网络中断");
            assert_eq!(theme.stored_count(), 0, "no settings applied");
        });

        dom.in_scope(ScopeId::APP, || shell.load_settings(theme));
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            assert!(shell.settings_failure(theme).is_some(), "a failed retry keeps it");
        });

        backend.heal("load_settings");
        dom.in_scope(ScopeId::APP, || shell.load_settings(theme));
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(shell.settings_failure(theme), None);
            assert_eq!(theme.stored_count(), 1);
        });
        assert_eq!(backend.calls("load_settings"), 3);
    }

    /// After a failed start, settings a page applies from the library (the settings page read
    /// them, an import replaced them) end the failure as well, with no retry (design review
    /// C4D-R2 R2-04).
    #[tokio::test(flavor = "current_thread")]
    async fn settings_a_page_applies_after_a_failed_start_end_the_failure() {
        let backend = Scripted::over_new_library();
        backend.fail("load_settings", BackendError::new(ErrorCode::Network, "网络中断"));
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, theme) = contexts(&dom);
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            assert!(shell.settings_failure(theme).is_some());
            theme.store(UserSettings { theme: ThemeMode::Dark, ..UserSettings::default() });
            assert_eq!(shell.settings_failure(theme), None);
        });
        assert_eq!(backend.calls("load_settings"), 1);
    }

    /// Settings a page applied while the shell's read was on its way are newer than what the
    /// read returns: the read does not undo them, and the failure is gone (design review C4D-R2
    /// R2-03, R2-04).
    #[tokio::test(flavor = "current_thread")]
    async fn a_read_issued_before_a_page_applied_settings_does_not_undo_them() {
        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, theme) = contexts(&dom);
        let saved = UserSettings { theme: ThemeMode::Dark, ..UserSettings::default() };
        dom.in_scope(ScopeId::APP, || theme.store(saved.clone()));
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.settings.peek().theme, ThemeMode::Dark, "the page's settings stay");
            assert_eq!(theme.stored_count(), 1, "the read was not applied");
            assert_eq!(shell.settings_failure(theme), None);
        });
        assert_eq!(backend.calls("load_settings"), 1);
    }
}
