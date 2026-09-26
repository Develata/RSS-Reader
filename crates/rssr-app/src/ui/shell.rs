use dioxus::core::{Runtime, current_scope_id};
use dioxus::prelude::*;
use dioxus_router::Navigator;
use rssr_domain::UserSettings;

use crate::{
    router::AppRoute,
    status::{set_status_error, set_status_info},
    ui::shell_browser::{complete_web_auth_transition, wait_for_refresh_feedback},
    ui::shell_prefs::{initial_entry_search, remember_entry_search},
    ui::shell_state::{HomeAction, ManualRefreshState, NavMode, resolve_home_action},
    ui::{ShellCommand, UiCommand, UiIntent, collect_projected_ui_command, visit_ui_command},
    web_auth::{
        ServerGateProbe, WebAuthState, auth_state, configured_username, login, probe_server_gate,
        recover_from_expired_server_session, setup_credentials,
    },
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

pub(crate) fn use_authenticated_shell_bus(
    mut auth: Signal<WebAuthState>,
    mut settings: Signal<UserSettings>,
) {
    use_resource(move || async move {
        let current_auth = auth();
        if current_auth == WebAuthState::PendingServerProbe {
            // 关键：门禁存在时**不能**回落到 local_auth_state()。本地浏览器门禁与服务端登录
            // 是两套互不相关的凭据，回落会把用户引去创建一组永远用不上的本地凭据。
            match probe_server_gate().await {
                ServerGateProbe::Authenticated => auth.set(WebAuthState::Authenticated),
                // 明确收到了「未通过认证」的应答：跳服务端登录页，这是这类部署唯一的恢复路径。
                ServerGateProbe::SessionExpired => recover_from_expired_server_session(),
                // 探测本身失败（离线、网络抖动）时**不跳转**：Web 端的数据在 localStorage 里，
                // 断网时页面本可以继续渲染，跳去 /login 只会得到一个加载失败的页面，
                // 把一个还能用的会话直接毁掉。
                //
                // 直接放行是安全的：客户端这道门禁不是安全边界——服务端的 require_auth 对每个
                // 请求都会重新校验，会话真的失效时相关请求自然会失败。
                ServerGateProbe::Unreachable => auth.set(WebAuthState::Authenticated),
                // 没有服务端门禁，才轮到本地判定（含回环主机检查）。
                ServerGateProbe::Absent => auth.set(auth_state()),
            }
            return;
        }

        if current_auth == WebAuthState::Authenticated {
            for snapshot in collect_projected_ui_command(
                UiCommand::Shell(ShellCommand::LoadAuthenticatedShell),
                UiIntent::into_authenticated_shell_loaded,
            )
            .await
            {
                settings.set(snapshot.settings);
            }
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

#[derive(Clone, Copy)]
pub(crate) struct WebAuthGateShell {
    state: WebAuthState,
    username: Signal<String>,
    password: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
}

impl WebAuthGateShell {
    pub(crate) fn title(self) -> &'static str {
        match self.state {
            WebAuthState::NeedsSetup => "初始化 Web 登录",
            WebAuthState::NeedsLogin => "登录 RSS-Reader",
            WebAuthState::Authenticated | WebAuthState::PendingServerProbe => "验证登录状态",
        }
    }

    pub(crate) fn intro(self) -> &'static str {
        match self.state {
            WebAuthState::NeedsSetup => {
                "当前只在本地浏览器使用场景下启用了数据保护。首次进入这个浏览器环境时，需要先设置一组本地用户名和密码。"
            }
            WebAuthState::NeedsLogin => {
                "请输入先前设置的用户名和密码，解锁当前浏览器里的本地阅读器数据。"
            }
            WebAuthState::Authenticated | WebAuthState::PendingServerProbe => {
                "正在确认当前登录状态，请稍候。"
            }
        }
    }

    pub(crate) fn submit_label(self) -> &'static str {
        match self.state {
            WebAuthState::NeedsSetup => "保存并进入",
            WebAuthState::NeedsLogin => "登录",
            WebAuthState::Authenticated | WebAuthState::PendingServerProbe => "继续",
        }
    }

    pub(crate) fn username(self) -> String {
        (self.username)()
    }

    pub(crate) fn password(self) -> String {
        (self.password)()
    }

    pub(crate) fn status(self) -> String {
        (self.status)()
    }

    pub(crate) fn status_tone(self) -> String {
        (self.status_tone)()
    }

    pub(crate) fn set_username(mut self, value: String) {
        self.username.set(value);
    }

    pub(crate) fn set_password(mut self, value: String) {
        self.password.set(value);
    }

    pub(crate) fn submit(mut self, on_authenticated: EventHandler<()>) {
        let next_username = self.username().trim().to_string();
        let next_password = self.password();
        let result = match self.state {
            WebAuthState::NeedsSetup => setup_credentials(&next_username, &next_password),
            WebAuthState::NeedsLogin => login(&next_username, &next_password),
            WebAuthState::Authenticated | WebAuthState::PendingServerProbe => Ok(()),
        };

        match result {
            Ok(()) => {
                set_status_info(self.status, self.status_tone, "验证通过，正在进入阅读器。");
                self.password.set(String::new());
                complete_web_auth_transition(on_authenticated);
            }
            Err(err) => {
                set_status_error(self.status, self.status_tone, err);
            }
        }
    }
}

pub(crate) fn use_web_auth_gate_shell(state: WebAuthState) -> WebAuthGateShell {
    let mut username = use_signal(String::new);
    let password = use_signal(String::new);
    let status =
        use_signal(|| "当前处于本地浏览器保护模式。首次使用请先设置用户名和密码。".to_string());
    let status_tone = use_signal(|| "info".to_string());

    use_effect(move || {
        if state == WebAuthState::NeedsLogin
            && username().is_empty()
            && let Some(default_username) = configured_username()
            && !default_username.is_empty()
        {
            username.set(default_username);
        }
    });

    WebAuthGateShell { state, username, password, status, status_tone }
}
