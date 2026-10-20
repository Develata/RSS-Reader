use dioxus::core::{Runtime, current_scope_id};
use dioxus::prelude::*;
use dioxus_router::Navigator;
use rssr_domain::UserSettings;

use crate::{
    router::AppRoute,
    status::set_status_error,
    ui::shell_browser::wait_for_refresh_feedback,
    ui::shell_prefs::{initial_entry_search, remember_entry_search},
    ui::shell_state::{HomeAction, ManualRefreshState, NavMode, resolve_home_action},
    ui::{ShellCommand, UiCommand, UiIntent, collect_projected_ui_command, visit_ui_command},
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct AppShellState {
    entry_search: Signal<String>,
    nav_mode: Signal<NavMode>,
    refresh: Signal<ManualRefreshState>,
    refresh_revision: Signal<u64>,
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
                    crate::ui::execute_ui_command(UiCommand::Shell(ShellCommand::ManualRefresh))
                        .await;
                let mut failed = true;
                let mut completed = false;
                for intent in intents {
                    if let Some((message, tone)) = intent.into_status() {
                        failed = tone == "error";
                        completed = true;
                        self.refresh.set(ManualRefreshState::Finished { message, failed });
                    }
                }
                if !completed {
                    self.refresh.set(ManualRefreshState::Finished {
                        message: "刷新未返回结果。".to_string(),
                        failed,
                    });
                }
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

/// Loads the shell's settings once. The Web page is served only after the server's login, so the
/// page itself has no gate (phase C, C4a).
pub(crate) fn use_authenticated_shell_bus(mut settings: Signal<UserSettings>) {
    use_resource(move || async move {
        for snapshot in collect_projected_ui_command(
            UiCommand::Shell(ShellCommand::LoadAuthenticatedShell),
            UiIntent::into_authenticated_shell_loaded,
        )
        .await
        {
            settings.set(snapshot.settings);
        }
    });
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
