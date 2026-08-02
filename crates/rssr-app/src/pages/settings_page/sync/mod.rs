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

    rsx! {
        div { "data-layout": "settings-card", "data-section": "webdav-sync",
            div { "data-slot": "settings-card-header",
                h3 { "data-slot": "card-title", "WebDAV 配置交换" }
            }
            div { "data-layout": "settings-card-section", "data-section": "webdav-endpoint",
                div { "data-slot": "settings-card-section-header",
                    h4 { "data-slot": "settings-card-section-title", "远端配置端点" }
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
                            "需要登录的 WebDAV 服务，请把凭据写进 endpoint（形如 https://用户名:密码@主机/路径/）。该地址只保存在当前会话，不会写入配置包。"
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
            }
            div { "data-layout": "settings-card-section", "data-section": "webdav-actions",
                div { "data-slot": "settings-card-section-header",
                    h4 { "data-slot": "settings-card-section-title", "同步动作" }
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
