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
        div { class: "exchange-header", "data-layout": "exchange-header", "data-section": "config-exchange",
            h3 { class: "card-title", "data-slot": "card-title", "配置交换" }
        }
        div { class: "exchange-grid", "data-layout": "exchange-grid",
            div { class: "exchange-card", "data-layout": "exchange-card", "data-section": "config-json",
                div { class: "settings-card__header", "data-slot": "settings-card-header",
                    h3 { class: "card-title", "data-slot": "card-title", "配置包 JSON" }
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
                div { class: "inline-actions", "data-layout": "exchange-card-actions",
                    button {
                        class: "button inline-actions__item",
                        "data-variant": "secondary",
                        "data-action": "export-config",
                        onclick: move |_| export_config_facade.export_config(),
                        "导出配置"
                    }
                    button {
                        class: "button inline-actions__item",
                        "data-variant": "{facade.config_import_button_variant()}",
                        "data-state": "{facade.config_import_state()}",
                        "data-action": "import-config",
                        onclick: move |_| import_config_facade.import_config(),
                        "{facade.config_import_button_label()}"
                    }
                }
            }
            div { class: "exchange-card", "data-layout": "exchange-card", "data-section": "opml",
                div { class: "settings-card__header", "data-slot": "settings-card-header",
                    h3 { class: "card-title", "data-slot": "card-title", "OPML" }
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
                div { class: "inline-actions", "data-layout": "exchange-card-actions",
                    button {
                        class: "button inline-actions__item",
                        "data-variant": "secondary",
                        "data-action": "export-opml",
                        onclick: move |_| export_opml_facade.export_opml(),
                        "导出 OPML"
                    }
                    button {
                        class: "button inline-actions__item",
                        "data-variant": "secondary",
                        "data-action": "import-opml",
                        onclick: move |_| import_opml_facade.import_opml(),
                        "导入 OPML"
                    }
                }
            }
        }
    }
}
