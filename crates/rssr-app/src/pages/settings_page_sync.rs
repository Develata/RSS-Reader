use dioxus::prelude::*;
use rssr_domain::UserSettings;

use super::settings_page_themes::detect_preset_key;
use crate::{
    bootstrap::AppServices,
    status::{set_status_error, set_status_info},
    theme::ThemeController,
};

#[component]
pub(crate) fn WebDavSettingsCard(
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    endpoint: Signal<String>,
    remote_path: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
    rsx! {
        div { class: "settings-card",
            div { class: "settings-card__header",
                h3 { "WebDAV 配置交换" }
                p { class: "settings-card__intro", "这里只负责配置同步，不上传文章正文和本地阅读状态。保持交换边界简单，能减少跨平台故障。" }
            }
            div { class: "settings-card__section",
                div { class: "settings-card__section-header",
                    h4 { class: "settings-card__section-title", "远端配置端点" }
                    p { class: "settings-card__section-intro", "填写 WebDAV 基础地址和远端文件路径。这里只有配置，不包含文章库。" }
                }
                div { class: "settings-form-grid",
                    div {
                        label { class: "field-label", r#for: "settings-webdav-endpoint", "Endpoint" }
                        input {
                            id: "settings-webdav-endpoint",
                            name: "webdav_endpoint",
                            class: "text-input",
                            "data-action": "webdav-endpoint",
                            value: "{endpoint}",
                            placeholder: "https://dav.example.com/base/",
                            oninput: move |event| endpoint.set(event.value())
                        }
                    }
                    div {
                        label { class: "field-label", r#for: "settings-webdav-remote-path", "Remote Path" }
                        input {
                            id: "settings-webdav-remote-path",
                            name: "webdav_remote_path",
                            class: "text-input",
                            "data-action": "webdav-remote-path",
                            value: "{remote_path}",
                            placeholder: "config/rss-reader.json",
                            oninput: move |event| remote_path.set(event.value())
                        }
                    }
                }
            }
            div { class: "settings-card__section",
                div { class: "settings-card__section-header",
                    h4 { class: "settings-card__section-title", "同步动作" }
                    p { class: "settings-card__section-intro", "上传会覆盖远端配置，下载会用远端配置替换当前本地配置。" }
                }
                div { class: "inline-actions settings-card__actions",
                    button {
                        class: "button secondary",
                        "data-action": "push-webdav",
                        onclick: move |_| {
                            let endpoint = endpoint();
                            let remote_path = remote_path();
                            spawn(async move {
                                match AppServices::shared().await {
                                    Ok(services) => match services.push_remote_config(&endpoint, &remote_path).await {
                                        Ok(()) => set_status_info(status, status_tone, "配置已上传到 WebDAV。"),
                                        Err(err) => set_status_error(status, status_tone, format!("上传配置失败：{err}")),
                                    },
                                    Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
                                }
                            });
                        },
                        "上传配置"
                    }
                    button {
                        class: "button secondary",
                        "data-action": "pull-webdav",
                        onclick: move |_| {
                            let endpoint = endpoint();
                            let remote_path = remote_path();
                            let mut draft = draft;
                            spawn(async move {
                                match AppServices::shared().await {
                                    Ok(services) => match services.pull_remote_config(&endpoint, &remote_path).await {
                                        Ok(true) => match services.load_settings().await {
                                            Ok(settings) => {
                                                preset_choice.set(detect_preset_key(&settings.custom_css).to_string());
                                                draft.set(settings.clone());
                                                theme.settings.set(settings);
                                                set_status_info(status, status_tone, "已从 WebDAV 下载并导入配置。");
                                            }
                                            Err(err) => set_status_error(status, status_tone, format!("导入后读取设置失败：{err}")),
                                        },
                                        Ok(false) => set_status_info(status, status_tone, "远端配置不存在。"),
                                        Err(err) => set_status_error(status, status_tone, format!("下载配置失败：{err}")),
                                    },
                                    Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
                                }
                            });
                        },
                        "下载配置"
                    }
                }
            }
        }
    }
}
