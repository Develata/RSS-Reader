use dioxus::prelude::*;

use crate::pages::feeds_page::{FeedsPageBindings, FeedsPageCommand, execute_feeds_page_command};

#[component]
pub(crate) fn ConfigExchangeSection(
    config_text: Signal<String>,
    opml_text: Signal<String>,
    pending_config_import: Signal<bool>,
    bindings: FeedsPageBindings,
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
                    "data-field": "config-text",
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
                        onclick: move |_| {
                            spawn(async move {
                                let outcome =
                                    execute_feeds_page_command(FeedsPageCommand::ExportConfig).await;
                                bindings.apply_command_outcome(outcome);
                            });
                        },
                        "导出配置"
                    }
                    button {
                        class: if pending_config_import() { "button danger" } else { "button secondary" },
                        "data-action": "import-config",
                        onclick: move |_| {
                            let command = FeedsPageCommand::ImportConfig {
                                raw: config_text(),
                                confirmed: pending_config_import(),
                            };
                            spawn(async move {
                                let outcome = execute_feeds_page_command(command).await;
                                bindings.apply_command_outcome(outcome);
                            });
                        },
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
                    "data-field": "opml-text",
                    value: "{opml_text}",
                    placeholder: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
                    oninput: move |event| opml_text.set(event.value())
                }
                div { class: "inline-actions",
                    button {
                        class: "button secondary",
                        "data-action": "export-opml",
                        onclick: move |_| {
                            spawn(async move {
                                let outcome =
                                    execute_feeds_page_command(FeedsPageCommand::ExportOpml).await;
                                bindings.apply_command_outcome(outcome);
                            });
                        },
                        "导出 OPML"
                    }
                    button {
                        class: "button secondary",
                        "data-action": "import-opml",
                        onclick: move |_| {
                            let command = FeedsPageCommand::ImportOpml { raw: opml_text() };
                            spawn(async move {
                                let outcome = execute_feeds_page_command(command).await;
                                bindings.apply_command_outcome(outcome);
                            });
                        },
                        "导入 OPML"
                    }
                }
            }
        }
    }
}
