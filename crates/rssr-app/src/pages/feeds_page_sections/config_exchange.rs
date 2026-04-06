use dioxus::prelude::*;

use crate::{
    bootstrap::AppServices,
    status::{set_status_error, set_status_info},
};

#[component]
pub(crate) fn ConfigExchangeSection(
    config_text: Signal<String>,
    opml_text: Signal<String>,
    pending_config_import: Signal<bool>,
    reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
    rsx! {
        div { class: "exchange-header",
            h3 { "配置交换" }
        }
        div { class: "exchange-grid",
            div { class: "exchange-card",
                div { class: "settings-card__header",
                    h3 { "配置包 JSON" }
                }
                label { class: "sr-only", r#for: "config-text", "配置包 JSON 文本" }
                textarea {
                    id: "config-text",
                    name: "config_text",
                    class: "text-area",
                    "data-action": "config-text",
                    value: "{config_text}",
                    placeholder: "{{\n  \"version\": 1,\n  ...\n}}",
                    oninput: move |event| {
                        pending_config_import.set(false);
                        config_text.set(event.value());
                    }
                }
                div { class: "inline-actions",
                    button {
                        class: "button secondary",
                        "data-action": "export-config",
                        onclick: move |_| export_config(config_text, status, status_tone),
                        "导出配置"
                    }
                    button {
                        class: if pending_config_import() { "button danger" } else { "button secondary" },
                        "data-action": "import-config",
                        onclick: move |_| import_config(
                            config_text,
                            pending_config_import,
                            reload_tick,
                            status,
                            status_tone,
                        ),
                        if pending_config_import() { "确认覆盖导入" } else { "导入配置" }
                    }
                }
            }
            div { class: "exchange-card",
                div { class: "settings-card__header",
                    h3 { "OPML" }
                }
                label { class: "sr-only", r#for: "opml-text", "OPML 文本" }
                textarea {
                    id: "opml-text",
                    name: "opml_text",
                    class: "text-area",
                    "data-action": "opml-text",
                    value: "{opml_text}",
                    placeholder: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
                    oninput: move |event| opml_text.set(event.value())
                }
                div { class: "inline-actions",
                    button {
                        class: "button secondary",
                        "data-action": "export-opml",
                        onclick: move |_| export_opml(opml_text, status, status_tone),
                        "导出 OPML"
                    }
                    button {
                        class: "button secondary",
                        "data-action": "import-opml",
                        onclick: move |_| import_opml(opml_text, reload_tick, status, status_tone),
                        "导入 OPML"
                    }
                }
            }
        }
    }
}

fn export_config(
    mut config_text: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
) {
    spawn(async move {
        match AppServices::shared().await {
            Ok(services) => match services.export_config_json().await {
                Ok(raw) => {
                    config_text.set(raw);
                    set_status_info(status, status_tone, "已导出配置包 JSON。".to_string());
                }
                Err(err) => set_status_error(status, status_tone, format!("导出配置包失败：{err}")),
            },
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });
}

fn import_config(
    config_text: Signal<String>,
    mut pending_config_import: Signal<bool>,
    mut reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) {
    if !pending_config_import() {
        pending_config_import.set(true);
        set_status_info(
            status,
            status_tone,
            "导入配置会按配置包覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。"
                .to_string(),
        );
        return;
    }

    let raw = config_text();
    spawn(async move {
        match AppServices::shared().await {
            Ok(services) => match services.import_config_json(&raw).await {
                Ok(()) => {
                    pending_config_import.set(false);
                    set_status_info(status, status_tone, "配置包已导入。".to_string());
                    reload_tick += 1;
                }
                Err(err) => {
                    pending_config_import.set(false);
                    set_status_error(status, status_tone, format!("导入配置包失败：{err}"));
                }
            },
            Err(err) => {
                pending_config_import.set(false);
                set_status_error(status, status_tone, format!("初始化应用失败：{err}"));
            }
        }
    });
}

fn export_opml(mut opml_text: Signal<String>, status: Signal<String>, status_tone: Signal<String>) {
    spawn(async move {
        match AppServices::shared().await {
            Ok(services) => match services.export_opml().await {
                Ok(raw) => {
                    opml_text.set(raw);
                    set_status_info(status, status_tone, "已导出 OPML。".to_string());
                }
                Err(err) => set_status_error(status, status_tone, format!("导出 OPML 失败：{err}")),
            },
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });
}

fn import_opml(
    opml_text: Signal<String>,
    mut reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) {
    let raw = opml_text();
    spawn(async move {
        match AppServices::shared().await {
            Ok(services) => match services.import_opml(&raw).await {
                Ok(()) => {
                    set_status_info(status, status_tone, "OPML 已导入。".to_string());
                    reload_tick += 1;
                }
                Err(err) => set_status_error(status, status_tone, format!("导入 OPML 失败：{err}")),
            },
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });
}
