use dioxus::prelude::*;

use crate::{
    bootstrap::AppServices,
    router::{AppRoute, RoutableApp},
    theme::{ThemeController, density_class, theme_class},
    web_auth::{
        WebAuthState, auth_state, configured_username, local_auth_state, login, setup_credentials,
        verify_server_gate,
    },
};

const APP_NAME: &str = "RSS-Reader";
const WEB_AUTH_MARKUP: &str = include_str!("../../../assets/branding/rssr-mark.svg");

#[derive(Clone, Copy)]
pub struct AppUiState {
    pub entry_search: Signal<String>,
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

#[component]
#[allow(non_snake_case)]
pub fn App() -> Element {
    let mut settings = use_signal(AppServices::default_settings);
    let mut auth = use_signal(auth_state);
    let entry_search = use_signal(initial_entry_search);
    use_context_provider(|| ThemeController { settings });
    use_context_provider(|| AppUiState { entry_search });

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

        if current_auth == WebAuthState::Authenticated
            && let Ok(services) = AppServices::shared().await
        {
            if let Ok(loaded) = services.load_settings().await {
                settings.set(loaded);
            }
            services.ensure_auto_refresh_started();
        }
    });

    rsx! {
        style { {include_str!("../../../assets/styles.css")} }
        if auth() == WebAuthState::Authenticated && !settings().custom_css.trim().is_empty() {
            style { id: "user-custom-css", "{settings().custom_css}" }
        }
        if auth() == WebAuthState::Authenticated {
            div {
                class: "app-shell {theme_class(settings().theme)} {density_class(settings().list_density)}",
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
    let mut ui = use_context::<AppUiState>();
    let navigator = use_navigator();

    rsx! {
        nav { class: "app-nav-shell",
            Link { class: "app-nav__brand", to: AppRoute::EntriesPage {}, "{APP_NAME}" }
            form {
                class: "app-nav__search",
                onsubmit: move |event| {
                    event.prevent_default();
                    navigator.push(AppRoute::EntriesPage {});
                },
                label {
                    class: "app-nav__search-icon",
                    r#for: "app-nav-search-input",
                    "⌕"
                }
                input {
                    id: "app-nav-search-input",
                    class: "app-nav__search-input",
                    r#type: "search",
                    placeholder: "搜索文章标题",
                    value: "{(ui.entry_search)()}",
                    onfocus: move |_| { navigator.push(AppRoute::EntriesPage {}); },
                    oninput: move |event| {
                        let value = event.value();
                        remember_entry_search(&value);
                        ui.entry_search.set(value);
                    },
                }
                span { class: "app-nav__search-hint", "Enter" }
            }
            div { class: "app-nav",
                Link { class: "app-nav__link", "data-nav": "feeds", to: AppRoute::FeedsPage {}, "订阅" }
                Link { class: "app-nav__link", "data-nav": "entries", to: AppRoute::EntriesPage {}, "文章" }
                Link { class: "app-nav__link", "data-nav": "settings", to: AppRoute::SettingsPage {}, "设置" }
            }
        }
    }
}

#[component]
fn WebAuthLoadingGate() -> Element {
    rsx! {
        div { class: "web-auth-shell",
            div { class: "web-auth-card",
                div { class: "web-auth-brand",
                    div { class: "web-auth-brand__mark", dangerous_inner_html: "{WEB_AUTH_MARKUP}" }
                    p { class: "web-auth-brand__name", "{APP_NAME}" }
                }
                h1 { class: "web-auth-card__title", "验证登录状态" }
                p { class: "web-auth-card__intro", "正在确认当前 Web 部署的服务端登录会话，请稍候。" }
                p { class: "status-banner info", "正在与服务端确认登录状态..." }
            }
        }
    }
}

#[component]
fn WebAuthGate(state: WebAuthState, on_authenticated: EventHandler<()>) -> Element {
    if matches!(state, WebAuthState::Authenticated | WebAuthState::PendingServerProbe) {
        return rsx! { WebAuthLoadingGate {} };
    }

    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut status =
        use_signal(|| "当前处于本地浏览器保护模式。首次使用请先设置用户名和密码。".to_string());
    let mut status_tone = use_signal(|| "info".to_string());

    use_effect(move || {
        if state == WebAuthState::NeedsLogin
            && username().is_empty()
            && let Some(default_username) = configured_username()
            && !default_username.is_empty()
        {
            username.set(default_username);
        }
    });

    let title = match state {
        WebAuthState::NeedsSetup => "初始化 Web 登录",
        WebAuthState::NeedsLogin => "登录 RSS-Reader",
        WebAuthState::Authenticated | WebAuthState::PendingServerProbe => "验证登录状态",
    };
    let intro = match state {
        WebAuthState::NeedsSetup => {
            "当前只在本地浏览器使用场景下启用了数据保护。首次进入这个浏览器环境时，需要先设置一组本地用户名和密码。"
        }
        WebAuthState::NeedsLogin => {
            "请输入先前设置的用户名和密码，解锁当前浏览器里的本地阅读器数据。"
        }
        WebAuthState::Authenticated | WebAuthState::PendingServerProbe => {
            "正在确认当前登录状态，请稍候。"
        }
    };
    let submit_label = match state {
        WebAuthState::NeedsSetup => "保存并进入",
        WebAuthState::NeedsLogin => "登录",
        WebAuthState::Authenticated | WebAuthState::PendingServerProbe => "继续",
    };

    rsx! {
        div { class: "web-auth-shell",
            div { class: "web-auth-card",
                div { class: "web-auth-brand",
                    div { class: "web-auth-brand__mark", dangerous_inner_html: "{WEB_AUTH_MARKUP}" }
                    p { class: "web-auth-brand__name", "{APP_NAME}" }
                }
                h1 { class: "web-auth-card__title", "{title}" }
                p { class: "web-auth-card__intro", "{intro}" }
                p {
                    class: "status-banner {status_tone()}",
                    "{status()}"
                }
                form {
                    class: "web-auth-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        let next_username = username();
                        let next_password = password();
                        let result = match state {
                            WebAuthState::NeedsSetup => setup_credentials(&next_username, &next_password),
                            WebAuthState::NeedsLogin => login(&next_username, &next_password),
                            WebAuthState::Authenticated | WebAuthState::PendingServerProbe => Ok(()),
                        };

                        match result {
                            Ok(()) => {
                                status.set("验证通过，正在进入阅读器。".to_string());
                                status_tone.set("info".to_string());
                                password.set(String::new());
                                complete_web_auth_transition(on_authenticated);
                            }
                            Err(err) => {
                                status.set(err);
                                status_tone.set("error".to_string());
                            }
                        }
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
                        value: "{username}",
                        autocomplete: "username",
                        oninput: move |event| username.set(event.value()),
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
                        value: "{password}",
                        autocomplete: if state == WebAuthState::NeedsSetup { "new-password" } else { "current-password" },
                        oninput: move |event| password.set(event.value()),
                    }
                    button {
                        class: "button",
                        r#type: "submit",
                        "{submit_label}"
                    }
                }
                p {
                    class: "web-auth-card__note",
                    "说明：这层门禁只用于 localhost 等本地浏览器场景下保护本地数据。对外部署时，真正的访问控制仍应由 rssr-web 服务端登录承担。"
                }
            }
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
