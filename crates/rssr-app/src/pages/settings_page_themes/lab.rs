use dioxus::prelude::*;
use rssr_domain::UserSettings;

use crate::theme::ThemeController;

use super::{theme_apply::apply_custom_css_from_raw, theme_io::export_css_file};

#[cfg(not(target_arch = "wasm32"))]
use super::theme_io::import_css_file;

#[cfg(target_arch = "wasm32")]
use super::theme_io::trigger_css_file_input_in_browser;

#[component]
pub(super) fn ThemeLabSection(
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
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

                spawn(async move {
                    match file.read_string().await {
                        Ok(raw) => apply_custom_css_from_raw(
                            theme,
                            draft,
                            preset_choice,
                            status,
                            status_tone,
                            raw,
                            "已从文件载入并应用自定义 CSS。".to_string(),
                        ),
                        Err(err) => crate::status::set_status_error(
                            status,
                            status_tone,
                            format!("载入 CSS 文件失败：{err}"),
                        ),
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
            class: "button secondary",
            "data-action": "import-custom-css-file",
            onclick: move |_| {
                if let Err(err) = trigger_css_file_input_in_browser() {
                    crate::status::set_status_error(
                        status,
                        status_tone,
                        format!("载入 CSS 文件失败：{err}"),
                    );
                }
            },
            "导入主题文件"
        }
    };

    #[cfg(not(target_arch = "wasm32"))]
    let import_css_trigger = rsx! {
        button {
            class: "button secondary",
            "data-action": "import-custom-css-file",
            onclick: move |_| {
                import_css_file(theme, draft, preset_choice, status, status_tone);
            },
            "导入主题文件"
        }
    };

    rsx! {
        div { class: "settings-card__section settings-card__section--theme-lab",
            div { class: "settings-card__section-header",
                h4 { class: "settings-card__section-title", "主题实验室" }
            }
            label { class: "field-label", r#for: "settings-custom-css", "自定义 CSS" }
            textarea {
                id: "settings-custom-css",
                name: "custom_css",
                class: "text-area",
                "data-field": "custom-css",
                value: "{draft().custom_css}",
                placeholder: "[data-page=\"reader\"] .reader-body {{ max-width: 72ch; }}",
                oninput: move |event| {
                    let mut next = draft();
                    next.custom_css = event.value();
                    preset_choice.set(super::detect_preset_key(&next.custom_css).to_string());
                    draft.set(next);
                }
            }
            div { class: "inline-actions settings-card__actions",
                {import_css_trigger}
                button {
                    class: "button secondary",
                    "data-action": "apply-custom-css",
                    onclick: move |_| {
                        apply_custom_css_from_raw(
                            theme,
                            draft,
                            preset_choice,
                            status,
                            status_tone,
                            draft().custom_css,
                            "已应用当前输入框中的自定义 CSS。".to_string(),
                        );
                    },
                    "应用当前 CSS"
                }
                button {
                    class: "button secondary",
                    "data-action": "export-custom-css-file",
                    onclick: move |_| {
                        export_css_file(draft().custom_css, status, status_tone);
                    },
                    "导出当前 CSS"
                }
            }
        }
        {file_import_input}
    }
}
