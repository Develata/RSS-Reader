mod session;
mod state;

use dioxus::prelude::*;

pub(crate) use self::session::SettingsPageSyncSession;
pub(crate) use self::state::SettingsPageSyncState;
use super::facade::SettingsPageFacade;

#[component]
pub(crate) fn WebDavSettingsCard(facade: SettingsPageFacade) -> Element {
    let endpoint_facade = facade.clone();
    let remote_path_facade = facade.clone();
    let push_facade = facade.clone();
    let save_sync_facade = facade.clone();
    let clear_sync_facade = facade.clone();
    let saved = facade.saved_remote_sync();
    let problem = facade.remote_sync_problem();
    let attention = if facade.remote_sync_needs_attention() { "invalid" } else { "idle" };
    let clear_damaged_facade = facade.clone();

    rsx! {
        div { "data-layout": "settings-card", "data-section": "webdav-sync",
            div { "data-slot": "settings-card-header",
                h2 { "data-slot": "card-title", "WebDAV 配置交换" }
            }
            if let Some(problem) = problem {
                div { "data-layout": "settings-card-section", "data-section": "webdav-auto-sync",
                    div { "data-slot": "settings-card-section-header",
                        h3 { "data-slot": "settings-card-section-title", "自动同步" }
                    }
                    p { "data-slot": "page-intro", "data-state": "invalid", "data-field": "webdav-sync-status",
                        "保存的自动同步设置无法读取：{problem}。停用后可重新记住端点。"
                    }
                    div { class: "inline-actions", "data-layout": "settings-card-actions",
                        button {
                            class: "button inline-actions__item",
                            "data-variant": "secondary",
                            "data-action": "clear-webdav-sync",
                            onclick: move |_| clear_damaged_facade.clear_remote_sync(),
                            "停用自动同步"
                        }
                    }
                }
            } else if let Some((endpoint, remote_path, status)) = saved {
                div { "data-layout": "settings-card-section", "data-section": "webdav-auto-sync",
                    div { "data-slot": "settings-card-section-header",
                        h3 { "data-slot": "settings-card-section-title", "自动同步" }
                    }
                    p { "data-slot": "page-intro", "data-field": "webdav-saved-endpoint",
                        "已记住 {endpoint} · {remote_path}。启动后和修改配置后自动同步；两边都改过时不会自动处理，需要手动上传或下载。"
                    }
                    if !status.is_empty() {
                        p {
                            "data-slot": "page-intro",
                            "data-state": "{attention}",
                            "data-field": "webdav-sync-status",
                            "{status}"
                        }
                    }
                    div { class: "inline-actions", "data-layout": "settings-card-actions",
                        button {
                            class: "button inline-actions__item",
                            "data-variant": "secondary",
                            "data-action": "clear-webdav-sync",
                            onclick: move |_| clear_sync_facade.clear_remote_sync(),
                            "停用自动同步"
                        }
                    }
                }
            } else {
                div { "data-layout": "settings-card-section", "data-section": "webdav-endpoint",
                    div { "data-slot": "settings-card-section-header",
                        h3 { "data-slot": "settings-card-section-title", "远端配置端点" }
                    }
                    div { "data-layout": "settings-form-grid",
                        div { "data-slot": "settings-form-grid-item",
                            label { class: "field-label", r#for: "settings-webdav-endpoint", "Endpoint" }
                            input {
                                id: "settings-webdav-endpoint",
                                name: "webdav_endpoint",
                                class: "text-input",
                                "data-field": "webdav-endpoint",
                                value: "{facade.endpoint()}",
                                placeholder: "https://用户名:密码@dav.example.com/base/",
                                oninput: move |event| endpoint_facade.set_endpoint(event.value())
                            }
                            p { "data-slot": "page-intro",
                                "需要登录的 WebDAV 服务，请把凭据写进 endpoint（形如 https://用户名:密码@主机/路径/）。不记住时只保存在当前会话；记住后以明文保存在本机数据目录（自托管 Web 为服务器数据目录），不会写入配置包。"
                            }
                        }
                        div { "data-slot": "settings-form-grid-item",
                            label { class: "field-label", r#for: "settings-webdav-remote-path", "Remote Path" }
                            input {
                                id: "settings-webdav-remote-path",
                                name: "webdav_remote_path",
                                class: "text-input",
                                "data-field": "webdav-remote-path",
                                value: "{facade.remote_path()}",
                                placeholder: "config/rss-reader.json",
                                oninput: move |event| remote_path_facade.set_remote_path(event.value())
                            }
                        }
                    }
                    if facade.remote_sync_available() {
                        div { class: "inline-actions", "data-layout": "settings-card-actions",
                            button {
                                class: "button inline-actions__item",
                                "data-variant": "secondary",
                                "data-action": "save-webdav-sync",
                                onclick: move |_| save_sync_facade.save_remote_sync(),
                                "记住并自动同步"
                            }
                        }
                    }
                }
            }
            div { "data-layout": "settings-card-section", "data-section": "webdav-actions",
                div { "data-slot": "settings-card-section-header",
                    h3 { "data-slot": "settings-card-section-title", "同步动作" }
                }
                div { class: "inline-actions", "data-layout": "settings-card-actions",
                    button {
                        class: "button inline-actions__item",
                        "data-variant": "secondary",
                        "data-action": "push-webdav",
                        onclick: move |_| push_facade.push(),
                        "上传配置"
                    }
                    button {
                        class: "button inline-actions__item",
                        "data-variant": "{facade.remote_pull_button_variant()}",
                        "data-state": "{facade.remote_pull_state()}",
                        "data-action": "pull-webdav",
                        onclick: move |_| facade.pull(),
                        "{facade.remote_pull_button_label()}"
                    }
                }
                if facade.is_remote_pull_pending() {
                    p { "data-slot": "confirm-hint",
                        "WebDAV 下载配置会覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击「确认下载并覆盖」才会执行。"
                    }
                }
            }
        }
    }
}
