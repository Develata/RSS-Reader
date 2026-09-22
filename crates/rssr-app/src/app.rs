use dioxus::prelude::*;

use crate::{
    bootstrap::AppServices,
    components::status_banner::StatusBanner,
    router::{AppRoute, RoutableApp},
    theme::{ThemeController, density_state, theme_class},
    ui::{
        use_app_nav_shell, use_app_shell_state, use_authenticated_shell_bus,
        use_web_auth_gate_shell,
    },
    web_auth::{WebAuthState, auth_state},
};

const APP_NAME: &str = "RSS-Reader";
const WEB_AUTH_MARKUP: &str = include_str!("../../../assets/branding/rssr-mark.svg");
const APP_STYLESHEET: &str = concat!(
    include_str!("../../../assets/styles/tokens.css"),
    "\n",
    include_str!("../../../assets/styles/shell.css"),
    "\n",
    include_str!("../../../assets/styles/workspaces.css"),
    "\n",
    include_str!("../../../assets/styles/entries.css"),
    "\n",
    include_str!("../../../assets/styles/reader.css"),
    "\n",
    include_str!("../../../assets/styles/responsive.css"),
);

#[component]
#[allow(non_snake_case)]
pub fn App() -> Element {
    let settings = use_signal(AppServices::default_settings);
    let mut auth = use_signal(auth_state);
    let shell = use_app_shell_state();
    use_context_provider(|| ThemeController { settings });
    use_context_provider(|| shell);

    use_authenticated_shell_bus(auth, settings);

    // 每次 `settings()` 都会克隆一整份 UserSettings（含完整 custom_css 文本），
    // 此前这里一次渲染要读 5 次。改为只读一次后面复用。
    //
    // 仍然只在已认证时读：读取会让本组件订阅 settings 信号，未认证时无条件读会把登录门禁
    // 也挂到 settings 的变更上——保持原来的短路语义。
    let authenticated = auth() == WebAuthState::Authenticated;
    let current_settings = if authenticated { Some(settings()) } else { None };

    rsx! {
        document::Meta {
            name: "viewport",
            content: "width=device-width, initial-scale=1, viewport-fit=cover"
        }
        style { {APP_STYLESHEET} }
        if let Some(settings) = current_settings {
            if !settings.custom_css.trim().is_empty() {
                style { id: "user-custom-css", "{settings.custom_css}" }
            }
            div {
                class: "app-shell {theme_class(settings.theme)}",
                "data-density": "{density_state(settings.list_density)}",
                style: "--reader-font-scale: {settings.reader_font_scale};",
                RoutableApp {}
            }
        } else if auth() == WebAuthState::PendingServerProbe {
            WebAuthLoadingGate {}
        } else {
            WebAuthGate {
                state: auth(),
                on_authenticated: move || auth.set(WebAuthState::Authenticated),
            }
        }
    }
}

#[component]
pub fn AppNav(on_back: Option<EventHandler<()>>) -> Element {
    let shell = use_app_nav_shell();
    let home_shell = shell.clone();
    let mut toggle_shell = shell.clone();
    let mut escape_shell = shell.clone();
    let submit_shell = shell.clone();
    let update_shell = shell.clone();
    let refresh = shell.refresh_state();
    let navigator = use_navigator();

    rsx! {
        nav { "data-layout": "app-nav-shell", "data-state": shell.nav_state(), aria_label: "主导航",
            div { "data-layout": "app-nav-topline",
                if shell.is_reader() {
                    button {
                        class: "icon-link-button", "data-nav": "back", r#type: "button",
                        aria_label: "返回", title: "返回",
                        onclick: move |_| {
                            if let Some(on_back) = on_back { on_back.call(()); }
                            else { crate::hooks::use_mobile_back_navigation::navigate_back(navigator, Some(AppRoute::EntriesPage {})); }
                        },
                        span { aria_hidden: "true", "←" }
                    }
                }
                button {
                    class: "icon-link-button", "data-slot": "app-nav-brand",
                    "data-action": "activate-home", "data-refresh-state": refresh.phase(),
                    r#type: "button", aria_label: "Read / 首页", title: "Read / 首页；在首页再次点击刷新全部订阅",
                    aria_busy: refresh.is_refreshing(),
                    onclick: move |_| home_shell.activate_home(),
                    span { "data-slot": "app-nav-brand-mark", aria_hidden: "true", "R" }
                }
                button {
                    class: "icon-link-button", "data-action": "toggle-search", r#type: "button",
                    aria_label: "搜索", title: "展开 / 收起搜索", aria_expanded: shell.is_search(),
                    aria_controls: "app-nav-search-input",
                    onclick: move |_| toggle_shell.toggle_search(),
                    svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", "aria-hidden": "true",
                        circle { cx: "10", cy: "10", r: "6" }
                        path { d: "m15 15 6 6" }
                    }
                }
                if shell.is_search() {
                    form {
                        "data-layout": "app-nav-search",
                        onsubmit: move |event| { event.prevent_default(); submit_shell.submit_search(); },
                        input {
                            id: "app-nav-search-input", "data-slot": "app-nav-search-input", "data-field": "entry-search",
                            r#type: "search", placeholder: "搜索文章标题", aria_label: "搜索文章标题",
                            value: shell.entry_search(),
                            onmounted: move |event| async move { let _ = event.set_focus(true).await; },
                            oninput: move |event| update_shell.set_entry_search(event.value()),
                            onkeydown: move |event| {
                                if event.key() == Key::Escape && event.modifiers().is_empty() && !event.is_composing() {
                                    escape_shell.close_search();
                                    document::eval("document.querySelector('[data-action=toggle-search]')?.focus()");
                                }
                            },
                        }
                    }
                } else {
                    div { "data-layout": "app-nav-links",
                        Link { class: "icon-link-button", "data-nav": "feeds", to: AppRoute::FeedsPage {}, aria_label: "订阅", title: "Subscribe / 订阅", span { aria_hidden: "true", "S" } }
                        Link { class: "icon-link-button", "data-nav": "settings", to: AppRoute::SettingsPage {}, aria_label: "设置", title: "设置", span { aria_hidden: "true", "⚙" } }
                    }
                }
            }
            output { "data-slot": "manual-refresh-status", "data-state": refresh.phase(), role: "status", aria_live: "polite", "{refresh.label()}" }
        }
    }
}

#[component]
fn WebAuthLoadingGate() -> Element {
    rsx! {
        div { "data-layout": "web-auth-shell",
            div { "data-layout": "web-auth-card",
                div { "data-layout": "web-auth-brand",
                    div { "data-slot": "web-auth-brand-mark", dangerous_inner_html: "{WEB_AUTH_MARKUP}" }
                    p { "data-slot": "web-auth-brand-name", "{APP_NAME}" }
                }
                h1 { "data-slot": "web-auth-title", "验证登录状态" }
                p { "data-slot": "web-auth-intro", "正在确认当前 Web 部署的服务端登录会话，请稍候。" }
                StatusBanner {
                    message: "正在与服务端确认登录状态...".to_string(),
                    tone: "info".to_string(),
                }
            }
        }
    }
}

#[component]
fn WebAuthGate(state: WebAuthState, on_authenticated: EventHandler<()>) -> Element {
    // hook 必须在任何提前 return 之前调用：`use_web_auth_gate_shell` 内部有多个
    // `use_signal` 和一个 `use_effect`，一旦某次渲染走了下面的提前 return 而另一次没有，
    // hook 调用顺序就会错位。当前那条分支实际不可达，但顺序依赖不该留成隐雷。
    let shell = use_web_auth_gate_shell(state);

    if matches!(state, WebAuthState::Authenticated | WebAuthState::PendingServerProbe) {
        return rsx! { WebAuthLoadingGate {} };
    }

    rsx! {
        div { "data-layout": "web-auth-shell",
            div { "data-layout": "web-auth-card",
                div { "data-layout": "web-auth-brand",
                    div { "data-slot": "web-auth-brand-mark", dangerous_inner_html: "{WEB_AUTH_MARKUP}" }
                    p { "data-slot": "web-auth-brand-name", "{APP_NAME}" }
                }
                h1 { "data-slot": "web-auth-title", "{shell.title()}" }
                p { "data-slot": "web-auth-intro", "{shell.intro()}" }
                StatusBanner { message: shell.status(), tone: shell.status_tone() }
                form {
                    "data-layout": "web-auth-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        shell.submit(on_authenticated);
                    },
                    label {
                        class: "field-label",
                        r#for: "web-auth-username",
                        "用户名"
                    }
                    input {
                        id: "web-auth-username",
                        name: "username",
                        class: "text-input",
                        value: "{shell.username()}",
                        autocomplete: "username",
                        oninput: move |event| shell.set_username(event.value()),
                    }
                    label {
                        class: "field-label",
                        r#for: "web-auth-password",
                        "密码"
                    }
                    input {
                        id: "web-auth-password",
                        name: "password",
                        class: "text-input",
                        r#type: "password",
                        value: "{shell.password()}",
                        autocomplete: if state == WebAuthState::NeedsSetup { "new-password" } else { "current-password" },
                        oninput: move |event| shell.set_password(event.value()),
                    }
                    button {
                        class: "button",
                        "data-variant": "primary",
                        r#type: "submit",
                        "{shell.submit_label()}"
                    }
                }
                p {
                    "data-slot": "web-auth-note",
                    "说明：这层门禁只用于 localhost 等本地浏览器场景下保护本地数据。对外部署时，真正的访问控制仍应由 rssr-web 服务端登录承担。"
                }
            }
        }
    }
}
