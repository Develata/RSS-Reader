use std::{cell::Cell, rc::Rc};

use dioxus::core::{Runtime, current_scope_id};
use dioxus::prelude::*;
use dioxus_router::Navigator;

use crate::{
    router::AppRoute,
    status::set_status,
    theme::ThemeController,
    ui::failure::{TONE_UNKNOWN, UiFailure},
    ui::shell_browser::{sleep, wait_for_refresh_feedback},
    ui::shell_prefs::{initial_entry_search, remember_entry_search},
    ui::shell_state::{HomeAction, ManualRefreshState, NavMode, resolve_home_action},
    ui::{ShellCommand, UiCommand, UiIntent, run_ui_command},
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct AppShellState {
    entry_search: Signal<String>,
    nav_mode: Signal<NavMode>,
    refresh: Signal<ManualRefreshState>,
    /// Manual refreshes finished; their feedback is dismissed only while no later one finished.
    refresh_revision: Signal<u64>,
    /// Bumped when what the pages show may be stale: each page then re-reads it, read-only
    /// (C4b item 2). A manual refresh's end bumps it at once; other triggers through
    /// [`AppShellState::request_reread`].
    library_revision: Signal<u64>,
    /// A re-read was requested and is waiting out [`REREAD_WINDOW`].
    reread_pending: Signal<bool>,
    /// The settings page is open: it re-reads the settings itself, the shell does not.
    settings_page_open: Signal<bool>,
    /// Why the settings could not be read at start, until they are (C4b item 5).
    settings_failure: Signal<Option<UiFailure>>,
    /// Why the startup page could not be resolved, until dismissed: the startup page is replaced
    /// at once, its own status with it (C4b item 5; review C4B-A-R3-03).
    startup_failure: Signal<Option<UiFailure>>,
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
    /// What the pages subscribe to for their read-only refresh (C4b item 2).
    pub(crate) fn library_revision(self) -> u64 {
        *self.library_revision.read()
    }

    /// Asks every page to re-read what it shows (read-only). Requests within [`REREAD_WINDOW`]
    /// make one re-read: the page becoming visible, the window gaining focus and the network
    /// coming back often arrive together. A re-read issued meanwhile (a manual refresh ended)
    /// covers them, and none follows.
    pub(crate) fn request_reread(mut self) {
        if *self.reread_pending.peek() {
            return;
        }
        self.reread_pending.set(true);
        let issued = *self.library_revision.peek();
        self.spawn_in_shell(async move {
            sleep(REREAD_WINDOW).await;
            self.reread_pending.set(false);
            if *self.library_revision.peek() == issued {
                self.library_revision += 1;
            }
        });
    }

    /// The settings page is open (`true`) or was closed: while it is open it re-reads the
    /// settings itself and applies them, so the shell does not read them a second time.
    pub(crate) fn set_settings_page_open(mut self, open: bool) {
        self.settings_page_open.set(open);
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
                self.library_revision += 1;
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
    pub(crate) fn load_settings(self, theme: ThemeController) {
        self.read_settings(theme, ShellCommand::LoadAuthenticatedShell);
    }

    /// The shell's read-only refresh: the settings again, applied like the start's (the same
    /// ordering against settings a page applied meanwhile), unless the settings page is open and
    /// reads them itself (C4b item 2).
    pub(crate) fn reread_settings(self, theme: ThemeController) {
        if *self.settings_page_open.peek() {
            return;
        }
        self.read_settings(theme, ShellCommand::LoadSettings);
    }

    fn read_settings(mut self, theme: ThemeController, command: ShellCommand) {
        if *self.settings_loading.peek() {
            return;
        }
        self.settings_loading.set(true);
        let issued = theme.stored_count();
        Runtime::current().in_scope(self.owner, || {
            spawn(async move {
                let intents = run_ui_command(UiCommand::Shell(command)).await;
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

    /// Why the startup page could not be resolved (the app opened all entries instead).
    pub(crate) fn startup_failure(self) -> Option<UiFailure> {
        self.startup_failure.read().clone()
    }

    pub(crate) fn report_startup_failure(mut self, failure: UiFailure) {
        self.startup_failure.set(Some(failure));
    }

    pub(crate) fn dismiss_startup_failure(mut self) {
        self.startup_failure.set(None);
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
        library_revision: use_signal(|| 0),
        reread_pending: use_signal(|| false),
        settings_page_open: use_signal(|| false),
        settings_failure: use_signal(|| None),
        startup_failure: use_signal(|| None),
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
///
/// When the start failed and a page then applies settings from the library, the backend works
/// again: the shell reads once more to complete its start (the periodic refresh starts with it;
/// review C4B-A-03). The read does not undo the page's settings (`load_settings`'s ordering).
pub(crate) fn use_authenticated_shell_bus(shell: AppShellState, theme: ThemeController) {
    use_hook(move || shell.load_settings(theme));
    use_effect(move || {
        if theme.has_stored() && shell.settings_failure.peek().is_some() {
            shell.load_settings(theme);
        }
    });
    // The shell's read-only refresh: on every bump after the start (C4b item 2).
    use_effect(move || {
        if shell.library_revision() > 0 {
            shell.reread_settings(theme);
        }
    });
}

/// How long the shell gathers re-read requests into one (C4b item 2).
pub(crate) const REREAD_WINDOW: std::time::Duration = std::time::Duration::from_millis(200);

/// The page re-reads what it shows when it becomes visible again, the window gains focus or the
/// network comes back (C4b item 2). The page's listeners only name the event; each one is a
/// request to the shell, inside the App's scope, which gathers them. The listeners go with the App.
pub(crate) fn use_reread_triggers(shell: AppShellState) {
    let bridge = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));
    let cleanup = Rc::clone(&bridge);
    use_drop(move || {
        if let Some(eval) = cleanup.get() {
            let _ = eval.send(());
        }
    });
    use_future(move || {
        let bridge = Rc::clone(&bridge);
        async move {
            let mut eval = document::eval(include_str!("reread_triggers.js"));
            bridge.set(Some(eval));
            while eval.recv::<String>().await.is_ok() {
                shell.request_reread();
            }
        }
    });
}

pub(crate) fn use_startup_route_bus(
    navigator: Navigator,
    status: Signal<String>,
    status_tone: Signal<String>,
) {
    let shell = use_context::<AppShellState>();
    use_resource(move || async move {
        shell
            .resolve_startup(status, status_tone, move |route| {
                let _ = navigator.replace(route);
            })
            .await;
    });
}

impl AppShellState {
    /// Resolves where the app starts and goes there; a failure is kept for the notice above every
    /// page, since the startup page goes away with the route (C4b item 5; review C4B-A-R3-03).
    pub(crate) async fn resolve_startup(
        self,
        status: Signal<String>,
        status_tone: Signal<String>,
        navigate: impl FnOnce(AppRoute),
    ) {
        let intents = run_ui_command(UiCommand::Shell(ShellCommand::ResolveStartupRoute)).await;
        let (route, failure) = startup_resolution(intents);
        if let Some(failure) = failure {
            set_status(status, status_tone, failure.message(), failure.tone());
            self.report_startup_failure(failure);
        }
        if let Some(route) = route {
            navigate(route);
        }
    }
}

/// Where the startup resolution goes, and why it failed if it did.
fn startup_resolution(intents: Vec<UiIntent>) -> (Option<AppRoute>, Option<UiFailure>) {
    let (mut route, mut failure) = (None, None);
    for intent in intents {
        match intent {
            UiIntent::StartupRouteResolved(snapshot) => route = Some(snapshot.route),
            UiIntent::Failed(failed) => failure = Some(failed),
            _ => {}
        }
    }
    (route, failure)
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
            assert_eq!(backend.calls("ensure_auto_refresh_started"), 1, "started regardless");
        });
        // The settings page saves (or reads) settings: they are the library's.
        backend.heal("load_settings");
        let patch =
            rssr_application::SettingsPatch { theme: Some(ThemeMode::Dark), ..Default::default() };
        let stored = rssr_api::Backend::update_settings(&*backend, patch).await.unwrap();
        dom.in_scope(ScopeId::APP, || {
            theme.store(stored);
            assert_eq!(shell.settings_failure(theme), None);
        });
        // The page's settings show the backend works again: the shell completes its start; its
        // read gives the library's settings, the ones the page applied.
        dom.render_immediate_to_vec();
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.settings.peek().theme, ThemeMode::Dark);
            assert_eq!(shell.settings_failure.peek().clone(), None);
        });
        assert_eq!(backend.calls("load_settings"), 2);
        assert_eq!(backend.calls("ensure_auto_refresh_started"), 2);
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

    /// `resolve_startup` with a failed resolution, of each class: it goes to all entries and the
    /// shell (not the startup page) keeps the failure until dismissed (review C4B-A-R3-03). It
    /// does not mount the startup page or render the notice: that the notice stays after the
    /// route is replaced is the browser acceptance's (C4c; review C4B-A-R4-01).
    #[tokio::test(flavor = "current_thread")]
    async fn a_failed_startup_resolution_is_kept_by_the_shell() {
        use crate::router::AppRoute;

        let errors = [
            BackendError::new(ErrorCode::Network, "原因"),
            BackendError::new(ErrorCode::OutcomeUnknown, "原因"),
            BackendError::new(ErrorCode::OutcomeUnknown, "原因")
                .with_reason(ErrorCode::Unauthenticated),
            BackendError::new(ErrorCode::Unauthenticated, "原因"),
            BackendError::new(ErrorCode::BuildMismatch, "原因"),
        ];
        for error in errors {
            let code = (error.code, error.reason);
            let backend = Scripted::over_new_library();
            backend.fail("resolve_startup_target", error.clone());
            let _host = test_host::install(backend.clone());
            let mut dom = VirtualDom::new(harness);
            dom.rebuild_in_place();
            let (shell, _) = contexts(&dom);
            settle(&mut dom, shell).await;
            let went = dom.in_scope(ScopeId::APP, || {
                let went = Signal::new(None);
                let status = Signal::new(String::new());
                let tone = Signal::new(String::new());
                spawn(async move {
                    let mut went = went;
                    shell.resolve_startup(status, tone, move |route| went.set(Some(route))).await;
                });
                went
            });
            for _ in 0..200 {
                let _ = tokio::time::timeout(Duration::from_millis(20), dom.wait_for_work()).await;
                if dom.in_scope(ScopeId::APP, || went.peek().is_some()) {
                    break;
                }
            }
            dom.in_scope(ScopeId::APP, || {
                assert_eq!(*went.peek(), Some(AppRoute::EntriesPage {}), "{code:?}");
                let failure = shell.startup_failure().expect("kept for the notice");
                assert_eq!(failure.error, error);
                shell.dismiss_startup_failure();
                assert_eq!(shell.startup_failure(), None);
            });
        }
    }

    /// Runs the app's tasks until `done` holds.
    async fn run_until(dom: &mut VirtualDom, what: &str, mut done: impl FnMut() -> bool) {
        for _ in 0..300 {
            let _ = tokio::time::timeout(Duration::from_millis(20), dom.wait_for_work()).await;
            dom.render_immediate_to_vec();
            if dom.in_scope(ScopeId::APP, &mut done) {
                return;
            }
        }
        panic!("{what}");
    }

    /// Lets the re-read window pass with the app running, then the reads it started end.
    async fn past_the_window(dom: &mut VirtualDom, shell: AppShellState) {
        let until = tokio::time::Instant::now() + super::REREAD_WINDOW * 2;
        while tokio::time::Instant::now() < until {
            let _ = tokio::time::timeout(Duration::from_millis(20), dom.wait_for_work()).await;
            dom.render_immediate_to_vec();
        }
        settle(dom, shell).await;
    }

    /// Re-read requests arriving together (visible, focus, online) make one re-read: one bump of
    /// the library revision, and the shell reads its settings once more, starting nothing
    /// (C4b item 2).
    #[tokio::test(flavor = "current_thread")]
    async fn rereads_requested_together_read_once() {
        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, theme) = contexts(&dom);
        settle(&mut dom, shell).await;
        assert_eq!(backend.calls("load_settings"), 1);

        dom.in_scope(ScopeId::APP, || {
            for _ in 0..3 {
                shell.request_reread();
            }
        });
        run_until(&mut dom, "the re-read", || backend.calls("load_settings") == 2).await;
        past_the_window(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(*shell.library_revision.peek(), 1, "one re-read");
            assert_eq!(theme.stored_count(), 2, "its settings applied");
        });
        assert_eq!(backend.calls("load_settings"), 2);
        assert_eq!(backend.calls("ensure_auto_refresh_started"), 1, "nothing started again");

        // Later requests make another.
        dom.in_scope(ScopeId::APP, || shell.request_reread());
        run_until(&mut dom, "the second re-read", || backend.calls("load_settings") == 3).await;
        dom.in_scope(ScopeId::APP, || assert_eq!(*shell.library_revision.peek(), 2));
    }

    /// While the settings page is open it reads the settings itself: the shell does not read them
    /// a second time for the same re-read.
    #[tokio::test(flavor = "current_thread")]
    async fn the_shell_leaves_the_settings_to_an_open_settings_page() {
        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, _) = contexts(&dom);
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            shell.set_settings_page_open(true);
            shell.request_reread();
        });
        run_until(&mut dom, "the bump", || *shell.library_revision.peek() == 1).await;
        past_the_window(&mut dom, shell).await;
        assert_eq!(backend.calls("load_settings"), 1);
    }

    /// A manual refresh that ends within the window re-reads already: the requests waiting are
    /// covered by it, and no second re-read follows.
    #[tokio::test(flavor = "current_thread")]
    async fn a_manual_refresh_ending_in_the_window_covers_the_requests() {
        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, _) = contexts(&dom);
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            shell.request_reread();
            shell.manual_refresh();
        });
        run_until(&mut dom, "the refresh", || *shell.library_revision.peek() == 1).await;
        past_the_window(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || assert_eq!(*shell.library_revision.peek(), 1));
        assert_eq!(backend.calls("refresh_all"), 1);
        assert_eq!(backend.calls("load_settings"), 2, "the start and one re-read");
    }

    /// A command whose outcome is unknown may have changed the library: the pages re-read
    /// (C4b item 2). A plain failure asks for nothing.
    #[tokio::test(flavor = "current_thread")]
    async fn an_unknown_outcome_asks_for_a_reread() {
        use crate::ui::commands::{SettingsCommand, UiCommand};

        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, _) = contexts(&dom);
        settle(&mut dom, shell).await;
        let save = || {
            UiCommand::Settings(SettingsCommand::UpdateSettings {
                patch: rssr_application::SettingsPatch::default(),
                success_message: String::new(),
            })
        };
        for (code, rereads) in [(ErrorCode::Network, 0), (ErrorCode::OutcomeUnknown, 1)] {
            backend.fail("update_settings", BackendError::new(code, "原因"));
            let done = dom.in_scope(ScopeId::APP, || {
                let mut done = Signal::new(false);
                let command = save();
                spawn(async move {
                    crate::ui::run_ui_command(command).await;
                    done.set(true);
                });
                done
            });
            run_until(&mut dom, "the save", || *done.peek()).await;
            past_the_window(&mut dom, shell).await;
            dom.in_scope(ScopeId::APP, || {
                assert_eq!(*shell.library_revision.peek(), rereads, "{code:?}");
            });
        }
    }
}
