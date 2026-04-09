use dioxus::prelude::*;

use crate::pages::feeds_page::facade::FeedsPageFacade;

#[component]
pub(crate) fn ConfigExchangeSection(facade: FeedsPageFacade) -> Element {
    let config_input_facade = facade.clone();
    let export_config_facade = facade.clone();
    let import_config_facade = facade.clone();
    let opml_input_facade = facade.clone();
    let export_opml_facade = facade.clone();
    let import_opml_facade = facade.clone();

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
                    "data-field": "config-text",
                    value: "{facade.config_text()}",
                    placeholder: "{{\n  \"version\": 1,\n  ...\n}}",
                    oninput: move |event| config_input_facade.set_config_text(event.value())
                }
                div { class: "inline-actions",
                    button {
                        class: "button secondary",
                        "data-action": "export-config",
                        onclick: move |_| export_config_facade.export_config(),
                        "导出配置"
                    }
                    button {
                        class: if facade.pending_config_import() { "button danger" } else { "button secondary" },
                        "data-action": "import-config",
                        onclick: move |_| import_config_facade.import_config(),
                        if facade.pending_config_import() { "确认覆盖导入" } else { "导入配置" }
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
                    "data-field": "opml-text",
                    value: "{facade.opml_text()}",
                    placeholder: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
                    oninput: move |event| opml_input_facade.set_opml_text(event.value())
                }
                div { class: "inline-actions",
                    button {
                        class: "button secondary",
                        "data-action": "export-opml",
                        onclick: move |_| export_opml_facade.export_opml(),
                        "导出 OPML"
                    }
                    button {
                        class: "button secondary",
                        "data-action": "import-opml",
                        onclick: move |_| import_opml_facade.import_opml(),
                        "导入 OPML"
                    }
                }
            }
        }
    }
}
