//! Notices about the whole app above every page (phase C, C4b items 1, 3 and 5): the session
//! ended (the page goes to the sign-in page), the page is of another build than the server
//! (reload it), a job ended after its page was left, or the settings could not be read at start
//! (retry).

use dioxus::prelude::*;

use crate::components::status_banner::StatusBanner;
use crate::theme::ThemeController;
use crate::ui::{
    AppShellState, drafts,
    recovery::{APP_RECOVERY, AppRecovery},
    shell_browser::{reload_page, sign_in_again},
};

#[component]
pub fn AppNotices() -> Element {
    let shell = use_context::<AppShellState>();
    let theme = use_context::<ThemeController>();
    // Which way out could not keep the settings page's draft: the page stays (C4b item 4).
    let mut not_kept = use_signal(|| None::<AppRecovery>);
    use_effect(move || {
        if APP_RECOVERY() == Some(AppRecovery::SignIn)
            && !drafts::leave_keeping(shell.settings_draft().as_ref(), sign_in_again)
        {
            not_kept.set(Some(AppRecovery::SignIn));
        }
    });
    let recovery = APP_RECOVERY();
    let kept = not_kept() != recovery || recovery.is_none();
    let settings_failure = shell.settings_failure(theme);
    let reread_failure = shell.settings_reread_failure(theme);
    let startup_failure = shell.startup_failure();
    let job_notice = shell.job_notice();
    let loading = shell.settings_loading();

    rsx! {
        match recovery {
            Some(AppRecovery::SignIn) => rsx! {
                aside { "data-slot": "app-notice", "data-notice": "sign-in", role: "alert",
                    if kept {
                        StatusBanner { message: "登录已失效，请重新登录。".to_string(), tone: "error".to_string() }
                    } else {
                        StatusBanner {
                            message: "登录已失效；浏览器暂时无法保留设置草稿，因此没有离开本页。".to_string(),
                            tone: "error".to_string(),
                        }
                        button { class: "button", "data-action": "sign-in-anyway",
                            onclick: move |_| sign_in_again(), "仍然去登录（草稿会丢失）" }
                    }
                }
            },
            Some(AppRecovery::Reload) => rsx! {
                aside { "data-slot": "app-notice", "data-notice": "reload", role: "alert",
                    if kept {
                        StatusBanner {
                            message: "页面版本与服务器不一致，需要重新加载页面。".to_string(),
                            tone: "error".to_string(),
                        }
                        button { class: "button", "data-action": "reload-page",
                            onclick: move |_| {
                                if !drafts::leave_keeping(shell.settings_draft().as_ref(), reload_page) {
                                    not_kept.set(Some(AppRecovery::Reload));
                                }
                            },
                            "重新加载"
                        }
                    } else {
                        StatusBanner {
                            message: "页面版本与服务器不一致；浏览器暂时无法保留设置草稿，因此没有重新加载。".to_string(),
                            tone: "error".to_string(),
                        }
                        button { class: "button", "data-action": "reload-anyway",
                            onclick: move |_| reload_page(), "仍然重新加载（草稿会丢失）" }
                    }
                }
            },
            None => rsx! {},
        }
        if let Some(notice) = job_notice {
            aside { "data-slot": "app-notice", "data-notice": "job",
                StatusBanner { message: notice.message, tone: notice.tone, announce: true }
                button { class: "button", "data-action": "dismiss-job-notice",
                    onclick: move |_| shell.dismiss_job_notice(), "知道了" }
            }
        }
        if let Some(failure) = startup_failure {
            aside { "data-slot": "app-notice", "data-notice": "startup-route", role: "alert",
                StatusBanner {
                    message: format!("{}（已打开全部文章）", failure.message()),
                    tone: failure.tone().to_string(),
                }
                button { class: "button", "data-action": "dismiss-startup-failure",
                    onclick: move |_| shell.dismiss_startup_failure(), "知道了" }
            }
        }
        if let Some(failure) = reread_failure {
            aside { "data-slot": "app-notice", "data-notice": "settings-reread", role: "alert",
                StatusBanner {
                    message: format!("{}（显示的是之前读到的设置）", failure.message()),
                    tone: failure.tone().to_string(),
                }
                button { class: "button", "data-action": "retry-settings-reread", disabled: loading, aria_busy: loading,
                    onclick: move |_| shell.retry_settings_read(theme), "重试" }
            }
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
