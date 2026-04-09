use crate::pages::settings_page::facade::SettingsPageFacade;
use dioxus::prelude::*;

use super::{theme_apply::apply_custom_css_from_raw, theme_io::export_css_file};

#[cfg(not(target_arch = "wasm32"))]
use super::theme_io::import_css_file;

#[cfg(target_arch = "wasm32")]
use super::theme_io::trigger_css_file_input_in_browser;

#[component]
pub(super) fn ThemeLabSection(facade: SettingsPageFacade) -> Element {
    let draft_signal = facade.draft_signal();
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
                        Err(err) => crate::status::set_status_error(
                            import_facade.status_signal(),
                            import_facade.status_tone_signal(),
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
                        import_trigger_facade.status_signal(),
                        import_trigger_facade.status_tone_signal(),
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
                import_css_file(&import_trigger_facade);
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
                value: "{draft_signal().custom_css}",
                placeholder: "[data-page=\"reader\"] .reader-body {{ max-width: 72ch; }}",
                oninput: move |event| {
                    let mut draft = input_facade.draft_signal();
                    let mut preset_choice = input_facade.preset_choice_signal();
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
                            &apply_facade,
                            apply_facade.draft_signal()().custom_css,
                            "已应用当前输入框中的自定义 CSS。",
                        );
                    },
                    "应用当前 CSS"
                }
                button {
                    class: "button secondary",
                    "data-action": "export-custom-css-file",
                    onclick: move |_| {
                        export_css_file(
                            export_facade.draft_signal()().custom_css,
                            export_facade.status_signal(),
                            export_facade.status_tone_signal(),
                        );
                    },
                    "导出当前 CSS"
                }
            }
        }
        {file_import_input}
    }
}
