use dioxus::prelude::*;
use dioxus_router::Navigator;
use rssr_domain::UserSettings;

use crate::{
    router::AppRoute,
    status::{set_status_error, set_status_info},
    ui::{UiCommand, UiIntent, collect_projected_ui_command, visit_ui_command},
    web_auth::{
        WebAuthState, configured_username, local_auth_state, login, setup_credentials,
        verify_server_gate,
    },
};

#[derive(Clone, Copy)]
pub(crate) struct AppShellState {
    entry_search: Signal<String>,
    nav_hidden: Signal<bool>,
}

impl AppShellState {
    pub(crate) fn entry_search(self) -> String {
        (self.entry_search)()
    }

    pub(crate) fn nav_hidden(self) -> bool {
        (self.nav_hidden)()
    }

    pub(crate) fn set_entry_search(mut self, value: String) {
        remember_entry_search(&value);
        self.entry_search.set(value);
    }

    pub(crate) fn show_nav(mut self) {
        remember_nav_hidden(false);
        self.nav_hidden.set(false);
    }

    pub(crate) fn hide_nav(mut self) {
        remember_nav_hidden(true);
        self.nav_hidden.set(true);
    }

    pub(crate) fn submit_search(self, navigator: Navigator) {
        navigator.push(AppRoute::EntriesPage {});
    }

    pub(crate) fn focus_search(self, navigator: Navigator) {
        navigator.push(AppRoute::EntriesPage {});
    }
}

pub(crate) fn use_app_shell_state() -> AppShellState {
    let entry_search = use_signal(initial_entry_search);
    let nav_hidden = use_signal(initial_nav_hidden);
    AppShellState { entry_search, nav_hidden }
}

#[derive(Clone)]
pub(crate) struct AppNavShell {
    shell: AppShellState,
    navigator: Navigator,
}

impl AppNavShell {
    pub(crate) fn nav_hidden(&self) -> bool {
        self.shell.nav_hidden()
    }

    pub(crate) fn nav_state(&self) -> &'static str {
        if self.nav_hidden() { "collapsed" } else { "expanded" }
    }

    pub(crate) fn entry_search(&self) -> String {
        self.shell.entry_search()
    }

    pub(crate) fn set_entry_search(&self, value: String) {
        self.shell.set_entry_search(value);
    }

    pub(crate) fn show_nav(&self) {
        self.shell.show_nav();
    }

    pub(crate) fn hide_nav(&self) {
        self.shell.hide_nav();
    }

    pub(crate) fn submit_search(&self) {
        self.shell.submit_search(self.navigator.clone());
    }

    pub(crate) fn focus_search(&self) {
        self.shell.focus_search(self.navigator.clone());
    }
}

pub(crate) fn use_app_nav_shell() -> AppNavShell {
    let shell = use_context::<AppShellState>();
    let navigator = use_navigator();
    AppNavShell { shell, navigator }
}

pub(crate) fn use_authenticated_shell_bus(
    mut auth: Signal<WebAuthState>,
    mut settings: Signal<UserSettings>,
) {
    use_resource(move || async move {
        let current_auth = auth();
        if current_auth == WebAuthState::PendingServerProbe {
            if verify_server_gate().await {
                auth.set(WebAuthState::Authenticated);
            } else {
                auth.set(local_auth_state());
            }
            return;
        }

        if current_auth == WebAuthState::Authenticated {
            for snapshot in collect_projected_ui_command(
                UiCommand::LoadAuthenticatedShell,
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
        visit_ui_command(UiCommand::ResolveStartupRoute, |intent| {
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

fn initial_entry_search() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
            && let Ok(Some(value)) = storage.get_item("rssr-entry-search")
        {
            return value;
        }
    }

    String::new()
}

fn remember_entry_search(_value: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
        {
            let _ = storage.set_item("rssr-entry-search", _value);
        }
    }
}

fn initial_nav_hidden() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
            && let Ok(Some(value)) = storage.get_item("rssr-nav-hidden")
        {
            return value == "1";
        }
    }

    false
}

fn remember_nav_hidden(_hidden: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
        {
            let _ = storage.set_item("rssr-nav-hidden", if _hidden { "1" } else { "0" });
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn complete_web_auth_transition(on_authenticated: EventHandler<()>) {
    if let Some(window) = web_sys::window()
        && window.location().reload().is_ok()
    {
        return;
    }

    on_authenticated.call(());
}

#[cfg(not(target_arch = "wasm32"))]
fn complete_web_auth_transition(on_authenticated: EventHandler<()>) {
    on_authenticated.call(());
}
