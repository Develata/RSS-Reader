use std::{cell::Cell, rc::Rc, sync::Arc};

use dioxus::core::{Runtime, current_scope_id};
use dioxus::prelude::*;
use dioxus_router::Navigator;

mod settings_read_lifecycle;
use settings_read_lifecycle::{ReadMode, ReadTrigger, SettingsReadLifecycle};

use crate::{
    router::AppRoute,
    status::set_status,
    theme::ThemeController,
    ui::failure::{TONE_UNKNOWN, UiFailure},
    ui::recovery::{APP_RECOVERY, DRAFT_NOT_KEPT},
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
    /// The settings page is open: it re-reads the settings itself, the shell does not.
    settings_page_open: Signal<bool>,
    /// A job's result whose page was left (C4b item 3), until dismissed or it times out.
    job_notice: Signal<Option<JobNotice>>,
    /// Bumped by every job notice: a timeout dismisses only its own.
    job_notice_revision: Signal<u64>,
    /// The open settings page's edits not stored yet, kept before the page is reloaded or leaves
    /// to sign in (C4b item 4).
    settings_draft: Signal<Option<rssr_application::SettingsPatch>>,
    /// The App's settings read lifecycle; settings values remain owned by ThemeController.
    settings_read: Signal<SettingsReadLifecycle<UiFailure>>,
    /// Why the startup page could not be resolved, until dismissed: the startup page is replaced
    /// at once, its own status with it (C4b item 5; review C4B-A-R3-03).
    startup_failure: Signal<Option<UiFailure>>,
    /// Preference writes and feed URL edits share one ordering boundary. A URL edit
    /// must migrate the last selected source, including saves from a page just left.
    /// New page reads wait at this boundary before loading their preference baseline.
    workspace_write_order: Signal<Arc<tokio::sync::Mutex<()>>>,
    owner: ScopeId,
}

impl AppShellState {
    pub(crate) fn workspace_write_order(self) -> Arc<tokio::sync::Mutex<()>> {
        Arc::clone(&self.workspace_write_order.peek())
    }

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

    /// Asks every page to re-read what it shows (read-only). A request waits [`REREAD_WINDOW`]
    /// and re-reads only if no re-read was issued since it was made: requests arriving together
    /// (the page becoming visible, the window gaining focus, the network coming back) make one,
    /// and a re-read issued meanwhile (a manual refresh ended) covers them.
    pub(crate) fn request_reread(mut self) {
        let requested_at = *self.library_revision.peek();
        self.spawn_in_shell(async move {
            sleep(REREAD_WINDOW).await;
            if *self.library_revision.peek() == requested_at {
                self.library_revision += 1;
            }
        });
    }

    /// Asks every page to re-read at once: a job ended (success or failure may have committed
    /// part of it, C4b item 3). Requests still waiting are covered by it.
    pub(crate) fn reread_now(mut self) {
        self.library_revision += 1;
    }

    /// Shows the result of a job whose page was left, above every page (C4b item 3). It goes like
    /// the manual refresh's feedback: after 3 s, or 6 s for a failure or an unknown outcome,
    /// unless dismissed or replaced before.
    pub(crate) fn report_job(mut self, message: String, tone: String) {
        let lasting = tone == "error" || tone == TONE_UNKNOWN;
        self.job_notice.set(Some(JobNotice { message, tone }));
        self.job_notice_revision += 1;
        let shown = *self.job_notice_revision.peek();
        self.spawn_in_shell(async move {
            sleep(std::time::Duration::from_secs(if lasting { 6 } else { 3 })).await;
            if *self.job_notice_revision.peek() == shown {
                self.job_notice.set(None);
            }
        });
    }

    pub(crate) fn job_notice(self) -> Option<JobNotice> {
        self.job_notice.read().clone()
    }

    pub(crate) fn dismiss_job_notice(mut self) {
        self.job_notice.set(None);
    }

    /// The settings page is open (`true`) or was closed: while it is open it re-reads the
    /// settings itself and applies them, so the shell does not read them a second time.
    pub(crate) fn set_settings_page_open(mut self, open: bool) {
        self.settings_page_open.set(open);
        if !open {
            self.set_settings_draft(None);
        }
    }

    /// The settings page's edits not stored yet (`None`: none), as they change (C4b item 4). Once
    /// the page is leaving, the kept draft follows them (review C4C2-02); when it cannot, the page
    /// stays and says so (review C4C2-R2-01).
    pub(crate) fn set_settings_draft(mut self, draft: Option<rssr_application::SettingsPatch>) {
        if crate::ui::drafts::follow(draft.as_ref()).is_err() {
            *DRAFT_NOT_KEPT.write() = *APP_RECOVERY.peek();
        }
        self.settings_draft.set(draft);
    }

    /// The draft to keep before the page leaves (not reactive).
    pub(crate) fn settings_draft(self) -> Option<rssr_application::SettingsPatch> {
        self.settings_draft.peek().clone()
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
                self.reread_now();
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
        self.read_settings(theme, ReadTrigger::Explicit);
    }

    /// Reads the settings again after a re-read failed (the notice's retry), whether or not the
    /// settings page is open.
    pub(crate) fn retry_settings_read(self, theme: ThemeController) {
        self.read_settings(theme, ReadTrigger::Explicit);
    }

    /// The shell's read-only refresh: the settings again, applied like the start's (the same
    /// ordering against settings a page applied meanwhile), unless the settings page is open and
    /// reads them itself (C4b item 2).
    pub(crate) fn reread_settings(self, theme: ThemeController) {
        self.read_settings(theme, ReadTrigger::Reread);
    }

    fn read_settings(mut self, theme: ThemeController, trigger: ReadTrigger) {
        let Some(read) = self.settings_read.write().begin(
            trigger,
            *self.settings_page_open.peek(),
            theme.stored_count(),
        ) else {
            return;
        };
        let command = match read.mode {
            ReadMode::Start => ShellCommand::LoadAuthenticatedShell,
            ReadMode::ReadOnly => ShellCommand::LoadSettings,
        };
        Runtime::current().in_scope(self.owner, || {
            spawn(async move {
                let intents = run_ui_command(UiCommand::Shell(command)).await;
                for intent in intents {
                    match intent {
                        UiIntent::AutoRefreshStarted => {
                            self.settings_read.write().auto_refresh_started();
                        }
                        UiIntent::AuthenticatedShellLoaded(snapshot) => {
                            let apply = self
                                .settings_read
                                .write()
                                .accept_loaded(&read, theme.stored_count());
                            if apply {
                                theme.store(snapshot.settings);
                            }
                        }
                        UiIntent::Failed(failure) => {
                            self.settings_read.write().failed(failure, theme.stored_count());
                        }
                        _ => {}
                    }
                }
                let reread = self.settings_read.write().finish(read, theme.stored_count());
                if reread {
                    self.reread_settings(theme);
                }
            });
        });
    }

    /// Why reading the settings again failed, while no settings were applied since: the app
    /// keeps showing the ones it had (review C4B-B-08; reactive).
    pub(crate) fn settings_reread_failure(self, theme: ThemeController) -> Option<UiFailure> {
        self.settings_read.read().reread_failure(theme.stored_revision()).cloned()
    }

    /// Why the settings could not be read, while they have not been: settings applied from the
    /// library by any page end it, not only this read (design review C4D-R2 R2-04).
    pub(crate) fn settings_failure(self, theme: ThemeController) -> Option<UiFailure> {
        self.settings_read.read().initial_failure(theme.has_stored()).cloned()
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
        self.settings_read.read().is_loading()
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

/// A job's result shown above every page (C4b item 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobNotice {
    pub(crate) message: String,
    pub(crate) tone: String,
}

pub(crate) fn use_app_shell_state() -> AppShellState {
    AppShellState {
        entry_search: use_signal(initial_entry_search),
        nav_mode: use_signal(NavMode::default),
        refresh: use_signal(ManualRefreshState::default),
        refresh_revision: use_signal(|| 0),
        library_revision: use_signal(|| 0),
        settings_page_open: use_signal(|| false),
        job_notice: use_signal(|| None),
        job_notice_revision: use_signal(|| 0),
        settings_draft: use_signal(|| None),
        settings_read: use_signal(SettingsReadLifecycle::default),
        startup_failure: use_signal(|| None),
        workspace_write_order: use_signal(|| Arc::new(tokio::sync::Mutex::new(()))),
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
    pub(crate) fn home_label(&self) -> &'static str {
        match resolve_home_action(&self.route) {
            HomeAction::Navigate => "Read / 首页",
            HomeAction::ManualRefresh => "Read / 刷新全部订阅",
        }
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
/// again: the shell reads once more to complete its start (review C4B-A-03); the periodic refresh
/// starts with the start's read whenever it got to the backend, and is not started again
/// (review C4B-B-07). The read does not undo the page's settings (`load_settings`'s ordering).
pub(crate) fn use_authenticated_shell_bus(shell: AppShellState, theme: ThemeController) {
    use_hook(move || shell.load_settings(theme));
    use_effect(move || {
        // Subscribe to stored settings, not lifecycle mutations: a failed recovery must not
        // schedule itself again when its loading/failure state is published.
        if shell.settings_read.peek().needs_recovery(theme.has_stored()) {
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
            assert!(!shell.settings_read.peek().needs_recovery(theme.has_stored()));
        });
        assert_eq!(backend.calls("load_settings"), 2);
        assert_eq!(backend.calls("ensure_auto_refresh_started"), 1, "started once (C4B-B-07)");
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

    #[tokio::test(flavor = "current_thread")]
    async fn an_equal_value_page_store_still_supersedes_an_in_flight_read() {
        let backend = Scripted::over_new_library();
        let hold = backend.hold("load_settings");
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, theme) = contexts(&dom);
        run_until(&mut dom, "the startup read waits", || hold.waiting() == 1).await;
        dom.in_scope(ScopeId::APP, || {
            let same = theme.settings.peek().clone();
            theme.store(same);
        });
        hold.release();
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.stored_count(), 1, "the old read did not apply a second store");
            assert_eq!(shell.settings_failure(theme), None);
        });
        assert_eq!(backend.calls("load_settings"), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_failed_recovery_does_not_retrigger_itself_through_lifecycle_notifications() {
        let backend = Scripted::over_new_library();
        backend.fail("load_settings", BackendError::new(ErrorCode::Network, "网络中断"));
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, theme) = contexts(&dom);
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            assert!(shell.settings_failure(theme).is_some());
            theme.store(UserSettings::default());
        });
        run_until(&mut dom, "recovery after a page store", || backend.calls("load_settings") == 2)
            .await;
        settle(&mut dom, shell).await;
        past_the_window(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(shell.settings_failure(theme), None);
            assert!(shell.settings_reread_failure(theme).is_some());
        });
        assert_eq!(backend.calls("load_settings"), 2);
        assert_eq!(backend.calls("ensure_auto_refresh_started"), 1);
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

    /// A re-read asked for while the shell's settings read is on its way is not lost: one more
    /// read follows it (review C4B-B-06).
    #[tokio::test(flavor = "current_thread")]
    async fn a_reread_asked_for_during_a_read_follows_it() {
        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, theme) = contexts(&dom);
        settle(&mut dom, shell).await;
        let hold = backend.hold("load_settings");
        dom.in_scope(ScopeId::APP, || shell.request_reread());
        run_until(&mut dom, "the re-read waits", || hold.waiting() == 1).await;
        dom.in_scope(ScopeId::APP, || {
            shell.request_reread();
            for _ in 0..3 {
                shell.retry_settings_read(theme);
            }
        });
        past_the_window_while(&mut dom).await;
        assert_eq!(backend.calls("load_settings"), 2, "one read on its way");
        hold.release();
        run_until(&mut dom, "the follow-up read", || backend.calls("load_settings") == 3).await;
        hold.release();
        settle(&mut dom, shell).await;
        assert_eq!(backend.calls("load_settings"), 3, "one follow-up for all pending requests");
        assert_eq!(backend.calls("ensure_auto_refresh_started"), 1);
        dom.in_scope(ScopeId::APP, || {
            assert_eq!(theme.stored_count(), 3, "follow-up captured the newly stored revision");
        });
    }

    #[tokio::test(flavor = "current_thread")]
    async fn opening_settings_consumes_pending_reads_but_explicit_retry_still_works() {
        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, theme) = contexts(&dom);
        settle(&mut dom, shell).await;
        let hold = backend.hold("load_settings");
        dom.in_scope(ScopeId::APP, || shell.reread_settings(theme));
        run_until(&mut dom, "the re-read waits", || hold.waiting() == 1).await;
        dom.in_scope(ScopeId::APP, || {
            shell.retry_settings_read(theme);
            shell.set_settings_page_open(true);
        });
        hold.release();
        settle(&mut dom, shell).await;
        assert_eq!(backend.calls("load_settings"), 2, "pending request dropped for the open page");
        dom.in_scope(ScopeId::APP, || shell.set_settings_page_open(false));
        past_the_window(&mut dom, shell).await;
        assert_eq!(backend.calls("load_settings"), 2, "closing alone does not restart a read");
        dom.in_scope(ScopeId::APP, || {
            shell.set_settings_page_open(true);
            shell.retry_settings_read(theme);
        });
        run_until(&mut dom, "explicit retry with settings open", || hold.waiting() == 1).await;
        dom.in_scope(ScopeId::APP, || shell.reread_settings(theme));
        hold.release();
        settle(&mut dom, shell).await;
        assert_eq!(backend.calls("load_settings"), 3, "the automatic request was not queued");
        assert_eq!(backend.calls("ensure_auto_refresh_started"), 1);
    }

    /// Reading the settings again can fail after settings were applied: the failure is shown
    /// (the app keeps the settings it had) until settings are applied again (review C4B-B-08).
    #[tokio::test(flavor = "current_thread")]
    async fn a_failed_settings_reread_is_shown_until_settings_are_applied() {
        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, theme) = contexts(&dom);
        settle(&mut dom, shell).await;
        backend.fail("load_settings", BackendError::new(ErrorCode::Network, "网络中断"));
        dom.in_scope(ScopeId::APP, || shell.request_reread());
        run_until(&mut dom, "the failed re-read", || backend.calls("load_settings") == 2).await;
        settle(&mut dom, shell).await;
        dom.in_scope(ScopeId::APP, || {
            let failure = shell.settings_reread_failure(theme).expect("shown");
            assert_eq!(failure.message(), "读取设置失败：网络中断");
            assert_eq!(shell.settings_failure(theme), None, "not the start's failure");
            // A page applies settings from the library: the failure is over.
            theme.store(UserSettings::default());
            assert_eq!(shell.settings_reread_failure(theme), None);
        });

        dom.in_scope(ScopeId::APP, || shell.request_reread());
        run_until(&mut dom, "another failed re-read", || backend.calls("load_settings") == 3).await;
        settle(&mut dom, shell).await;
        assert!(dom.in_scope(ScopeId::APP, || shell.settings_reread_failure(theme)).is_some());
        backend.heal("load_settings");
        dom.in_scope(ScopeId::APP, || shell.retry_settings_read(theme));
        run_until(&mut dom, "the retry", || backend.calls("load_settings") == 4).await;
        settle(&mut dom, shell).await;
        assert_eq!(dom.in_scope(ScopeId::APP, || shell.settings_reread_failure(theme)), None);
    }

    /// Runs the app's tasks for the re-read window without waiting for reads to end.
    async fn past_the_window_while(dom: &mut VirtualDom) {
        let until = tokio::time::Instant::now() + super::REREAD_WINDOW * 2;
        while tokio::time::Instant::now() < until {
            let _ = tokio::time::timeout(Duration::from_millis(20), dom.wait_for_work()).await;
            dom.render_immediate_to_vec();
        }
    }

    /// The start could not open the backend, so the periodic refresh did not start; a later
    /// re-read that works completes the start, whatever path it takes (review C4B-B-R2-05).
    #[tokio::test(flavor = "current_thread")]
    async fn a_reread_completes_a_start_that_never_reached_the_backend() {
        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        test_host::fail_open(Some(BackendError::new(ErrorCode::Network, "打不开")));
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (shell, theme) = contexts(&dom);
        settle(&mut dom, shell).await;
        assert!(dom.in_scope(ScopeId::APP, || shell.settings_failure(theme)).is_some());
        assert_eq!(backend.calls("ensure_auto_refresh_started"), 0);

        test_host::fail_open(None);
        dom.in_scope(ScopeId::APP, || shell.request_reread());
        run_until(&mut dom, "the re-read", || backend.calls("load_settings") == 1).await;
        settle(&mut dom, shell).await;
        assert_eq!(backend.calls("ensure_auto_refresh_started"), 1, "the start completed");
        dom.in_scope(ScopeId::APP, || assert_eq!(shell.settings_failure(theme), None));

        dom.in_scope(ScopeId::APP, || shell.request_reread());
        run_until(&mut dom, "a later re-read", || backend.calls("load_settings") == 2).await;
        settle(&mut dom, shell).await;
        assert_eq!(backend.calls("ensure_auto_refresh_started"), 1, "and only once");
    }
}
