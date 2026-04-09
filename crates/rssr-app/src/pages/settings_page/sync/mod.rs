mod session;
mod state;

use dioxus::prelude::*;

use self::{session::SettingsPageSyncSession, state::SettingsPageSyncState};
use super::session::SettingsPageSession;

#[component]
pub(crate) fn WebDavSettingsCard(session: SettingsPageSession) -> Element {
    let state = use_signal(SettingsPageSyncState::new);
    let sync_session = SettingsPageSyncSession::new(state, session);
    let snapshot = sync_session.snapshot();

    rsx! {
        div { class: "settings-card",
            div { class: "settings-card__header",
                h3 { "WebDAV 配置交换" }
            }
            div { class: "settings-card__section",
                div { class: "settings-card__section-header",
                    h4 { class: "settings-card__section-title", "远端配置端点" }
                }
                div { class: "settings-form-grid",
                    div {
                        label { class: "field-label", r#for: "settings-webdav-endpoint", "Endpoint" }
                        input {
                            id: "settings-webdav-endpoint",
                            name: "webdav_endpoint",
                            class: "text-input",
                            "data-field": "webdav-endpoint",
                            value: "{snapshot.endpoint}",
                            placeholder: "https://dav.example.com/base/",
                            oninput: move |event| sync_session.set_endpoint(event.value())
                        }
                    }
                    div {
                        label { class: "field-label", r#for: "settings-webdav-remote-path", "Remote Path" }
                        input {
                            id: "settings-webdav-remote-path",
                            name: "webdav_remote_path",
                            class: "text-input",
                            "data-field": "webdav-remote-path",
                            value: "{snapshot.remote_path}",
                            placeholder: "config/rss-reader.json",
                            oninput: move |event| sync_session.set_remote_path(event.value())
                        }
                    }
                }
            }
            div { class: "settings-card__section",
                div { class: "settings-card__section-header",
                    h4 { class: "settings-card__section-title", "同步动作" }
                }
                div { class: "inline-actions settings-card__actions",
                    button {
                        class: "button secondary",
                        "data-action": "push-webdav",
                        onclick: move |_| sync_session.push(),
                        "上传配置"
                    }
                    button {
                        class: if snapshot.pending_remote_pull {
                            "button danger"
                        } else {
                            "button secondary"
                        },
                        "data-action": "pull-webdav",
                        onclick: move |_| sync_session.pull(),
                        if snapshot.pending_remote_pull { "确认下载并覆盖" } else { "下载配置" }
                    }
                }
            }
        }
    }
}
