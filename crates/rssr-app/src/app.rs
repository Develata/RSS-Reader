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

    rsx! {
        document::Meta {
            name: "viewport",
            content: "width=device-width, initial-scale=1, viewport-fit=cover"
        }
        style { {APP_STYLESHEET} }
        if auth() == WebAuthState::Authenticated && !settings().custom_css.trim().is_empty() {
            style { id: "user-custom-css", "{settings().custom_css}" }
        }
        if auth() == WebAuthState::Authenticated {
            div {
                class: "app-shell {theme_class(settings().theme)}",
                "data-density": "{density_state(settings().list_density)}",
                style: "--reader-font-scale: {settings().reader_font_scale};",
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
pub fn AppNav() -> Element {
    let shell = use_app_nav_shell();
    let show_nav_shell = shell.clone();
    let hide_nav_shell = shell.clone();
    let submit_search_shell = shell.clone();
    let focus_search_shell = shell.clone();
    let update_search_shell = shell.clone();

    if shell.nav_hidden() {
        return rsx! {
            div { "data-layout": "app-nav-reveal", "data-state": "{shell.nav_state()}",
                button {
                    "data-slot": "app-nav-reveal-button",
                    "data-action": "show-top-nav",
                    onclick: move |_| {
                        show_nav_shell.show_nav();
                    },
                    span { "data-slot": "app-nav-reveal-icon", "≡" }
                }
            }
        };
    }

    rsx! {
        nav { "data-layout": "app-nav-shell", "data-state": "{shell.nav_state()}",
            div { "data-layout": "app-nav-topline",
                Link {
                    "data-slot": "app-nav-brand",
                    to: AppRoute::EntriesPage {},
                    aria_label: APP_NAME,
                    span { "data-slot": "app-nav-brand-mark", "R" }
                    span { "data-slot": "app-nav-brand-name", "{APP_NAME}" }
                }
                div { "data-layout": "app-nav-links",
                    Link { "data-nav": "feeds", to: AppRoute::FeedsPage {}, "订阅" }
                    Link { "data-nav": "entries", to: AppRoute::EntriesPage {}, "文章" }
                    Link { "data-nav": "settings", to: AppRoute::SettingsPage {}, "设置" }
                }
                button {
                    "data-slot": "app-nav-collapse",
                    "data-action": "hide-top-nav",
                    r#type: "button",
                    aria_label: "收起顶部导航",
                    title: "收起顶部导航",
                    onclick: move |_| {
                        hide_nav_shell.hide_nav();
                    },
                    "×"
                }
            }
            form {
                "data-layout": "app-nav-search",
                onsubmit: move |event| {
                    event.prevent_default();
                    submit_search_shell.submit_search();
                },
                label {
                    "data-slot": "app-nav-search-icon",
                    r#for: "app-nav-search-input",
                    "⌕"
                }
                input {
                    id: "app-nav-search-input",
                    "data-slot": "app-nav-search-input",
                    "data-field": "entry-search",
                    r#type: "search",
                    placeholder: "搜索文章标题",
                    value: "{shell.entry_search()}",
                    onfocus: move |_| focus_search_shell.focus_search(),
                    oninput: move |event| {
                        update_search_shell.set_entry_search(event.value());
                    },
                }
                span { "data-slot": "app-nav-search-hint", "Enter" }
            }
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
    if matches!(state, WebAuthState::Authenticated | WebAuthState::PendingServerProbe) {
        return rsx! { WebAuthLoadingGate {} };
    }

    let shell = use_web_auth_gate_shell(state);

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
