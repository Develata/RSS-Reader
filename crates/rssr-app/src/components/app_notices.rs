//! Notices about the whole app above every page (phase C, C4b items 1 and 5): the session ended
//! (the page goes to the sign-in page), the page is of another build than the server (reload it),
//! or the settings could not be read at start (retry).

use dioxus::prelude::*;

use crate::components::status_banner::StatusBanner;
use crate::theme::ThemeController;
use crate::ui::{
    AppShellState,
    recovery::{APP_RECOVERY, AppRecovery},
    shell_browser::{reload_page, sign_in_again},
};

#[component]
pub fn AppNotices() -> Element {
    let shell = use_context::<AppShellState>();
    let theme = use_context::<ThemeController>();
    use_effect(move || {
        if APP_RECOVERY() == Some(AppRecovery::SignIn) {
            sign_in_again();
        }
    });
    let recovery = APP_RECOVERY();
    let settings_failure = shell.settings_failure();
    let loading = shell.settings_loading();

    rsx! {
        match recovery {
            Some(AppRecovery::SignIn) => rsx! {
                aside { "data-slot": "app-notice", "data-notice": "sign-in", role: "alert",
                    StatusBanner { message: "登录已失效，请重新登录。".to_string(), tone: "error".to_string() }
                }
            },
            Some(AppRecovery::Reload) => rsx! {
                aside { "data-slot": "app-notice", "data-notice": "reload", role: "alert",
                    StatusBanner {
                        message: "页面版本与服务器不一致，需要重新加载页面。".to_string(),
                        tone: "error".to_string(),
                    }
                    button { class: "button", "data-action": "reload-page", onclick: move |_| reload_page(), "重新加载" }
                }
            },
            None => rsx! {},
        }
        if let Some(failure) = settings_failure {
            aside { "data-slot": "app-notice", "data-notice": "settings-load", role: "alert",
                StatusBanner {
                    message: format!("{}（暂用默认外观）", failure.message()),
                    tone: failure.tone().to_string(),
                }
                button { class: "button", "data-action": "retry-settings-load", disabled: loading, aria_busy: loading,
                    onclick: move |_| shell.load_settings(theme), "重试" }
            }
        }
    }
}
