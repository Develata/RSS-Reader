use dioxus::prelude::*;

use crate::pages::feeds_page::session::FeedsPageSession;

#[component]
pub(crate) fn ConfigExchangeSection(session: FeedsPageSession) -> Element {
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
                    value: "{session.config_text()}",
                    placeholder: "{{\n  \"version\": 1,\n  ...\n}}",
                    oninput: move |event| session.set_config_text(event.value())
                }
                div { class: "inline-actions",
                    button {
                        class: "button secondary",
                        "data-action": "export-config",
                        onclick: move |_| session.export_config(),
                        "导出配置"
                    }
                    button {
                        class: if session.pending_config_import() { "button danger" } else { "button secondary" },
                        "data-action": "import-config",
                        onclick: move |_| session.import_config(),
                        if session.pending_config_import() { "确认覆盖导入" } else { "导入配置" }
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
                    value: "{session.opml_text()}",
                    placeholder: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
                    oninput: move |event| session.set_opml_text(event.value())
                }
                div { class: "inline-actions",
                    button {
                        class: "button secondary",
                        "data-action": "export-opml",
                        onclick: move |_| session.export_opml(),
                        "导出 OPML"
                    }
                    button {
                        class: "button secondary",
                        "data-action": "import-opml",
                        onclick: move |_| session.import_opml(),
                        "导入 OPML"
                    }
                }
            }
        }
    }
}
