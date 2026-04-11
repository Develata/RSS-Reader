use crate::pages::settings_page::facade::SettingsPageFacade;
use dioxus::prelude::*;

use super::{theme_apply::apply_custom_css_from_raw, theme_io::export_css_file};

#[cfg(not(target_arch = "wasm32"))]
use super::theme_io::import_css_file;

#[cfg(target_arch = "wasm32")]
use super::theme_io::trigger_css_file_input_in_browser;

#[component]
pub(super) fn ThemeLabSection(facade: SettingsPageFacade) -> Element {
    #[cfg(target_arch = "wasm32")]
    let import_file_facade = facade.clone();
    let import_trigger_facade = facade.clone();
    let input_facade = facade.clone();
    let apply_facade = facade.clone();
    let export_facade = facade.clone();

    #[cfg(target_arch = "wasm32")]
    let file_import_input = rsx! {
        input {
            id: "custom-css-file-input",
            class: "sr-only-file-input",
            style: "display:none",
            r#type: "file",
            accept: ".css,text/css",
            onchange: move |event| {
                let Some(file) = event.files().into_iter().next() else {
                    return;
                };
                let import_facade = import_file_facade.clone();

                spawn(async move {
                    match file.read_string().await {
                        Ok(raw) => apply_custom_css_from_raw(
                            &import_facade,
                            raw,
                            "已从文件载入并应用自定义 CSS。",
                        ),
                        Err(err) => import_facade
                            .set_status(format!("载入 CSS 文件失败：{err}"), "error"),
                    }
                });
            },
        }
    };

    #[cfg(not(target_arch = "wasm32"))]
    let file_import_input = rsx! {};

    #[cfg(target_arch = "wasm32")]
    let import_css_trigger = rsx! {
        button {
            class: "button inline-actions__item",
            "data-variant": "secondary",
            "data-action": "import-custom-css-file",
            onclick: move |_| {
                if let Err(err) = trigger_css_file_input_in_browser() {
                    import_trigger_facade
                        .set_status(format!("载入 CSS 文件失败：{err}"), "error");
                }
            },
            "导入主题文件"
        }
    };

    #[cfg(not(target_arch = "wasm32"))]
    let import_css_trigger = rsx! {
        button {
            class: "button inline-actions__item",
            "data-variant": "secondary",
            "data-action": "import-custom-css-file",
            onclick: move |_| {
                import_css_file(&import_trigger_facade);
            },
            "导入主题文件"
        }
    };

    rsx! {
        div {
            class: "settings-card__section settings-card__section--theme-lab",
            "data-layout": "theme-lab",
            "data-section": "settings-theme-lab",
            div { class: "settings-card__section-header", "data-slot": "settings-card-section-header",
                h4 { class: "settings-card__section-title", "data-slot": "settings-card-section-title", "主题实验室" }
            }
            label { class: "field-label", r#for: "settings-custom-css", "自定义 CSS" }
            textarea {
                id: "settings-custom-css",
                name: "custom_css",
                class: "text-area",
                "data-field": "custom-css",
                value: "{facade.custom_css()}",
                placeholder: "[data-page=\"reader\"] .reader-body {{ max-width: 72ch; }}",
                oninput: move |event| {
                    input_facade.set_custom_css(event.value());
                }
            }
            div { class: "inline-actions settings-card__actions", "data-layout": "theme-lab-actions",
                {import_css_trigger}
                button {
                    class: "button inline-actions__item",
                    "data-variant": "secondary",
                    "data-action": "apply-custom-css",
                    onclick: move |_| {
                        apply_custom_css_from_raw(
                            &apply_facade,
                            apply_facade.custom_css(),
                            "已应用当前输入框中的自定义 CSS。",
                        );
                    },
                    "应用当前 CSS"
                }
                button {
                    class: "button inline-actions__item",
                    "data-variant": "secondary",
                    "data-action": "export-custom-css-file",
                    onclick: move |_| {
                        export_css_file(export_facade.custom_css(), &export_facade);
                    },
                    "导出当前 CSS"
                }
            }
        }
        {file_import_input}
    }
}
