use dioxus::prelude::*;

use crate::pages::settings_page::facade::SettingsPageFacade;

use super::{
    theme_apply::{apply_builtin_theme, apply_custom_css_from_raw, clear_custom_css},
    theme_preset::{builtin_theme_presets, detect_preset_key},
};

#[component]
pub(super) fn ThemePresetSections(facade: SettingsPageFacade) -> Element {
    let draft = facade.draft();
    let preset_choice = facade.preset_choice();
    let atlas_facade = facade.clone();
    let newsprint_facade = facade.clone();
    let forest_facade = facade.clone();
    let midnight_facade = facade.clone();
    let clear_facade = facade.clone();
    let apply_selected_facade = facade.clone();

    rsx! {
        div { class: "settings-card__section",
            div { class: "settings-card__section-header",
                h4 { class: "settings-card__section-title", "内置主题预设" }
            }
            div { class: "inline-actions settings-card__actions",
                select {
                    id: "settings-preset-theme",
                    name: "preset_theme",
                    class: "select-input",
                    "data-field": "preset-theme-select",
                    value: "{preset_choice}",
                    onchange: move |event| facade.set_preset_choice(event.value()),
                    option { value: "none", "无预设" }
                    option { value: "custom", "自定义主题" }
                    option { value: "atlas-sidebar", "Atlas Sidebar" }
                    option { value: "newsprint", "Newsprint" }
                    option { value: "forest-desk", "Amethyst Glass" }
                    option { value: "midnight-ledger", "Midnight Ledger" }
                }
                button {
                    class: "button secondary",
                    "data-action": "apply-selected-theme",
                    onclick: move |_| {
                        let choice = apply_selected_facade.preset_choice();
                        if choice == "none" {
                            clear_custom_css(&apply_selected_facade, "已清空自定义 CSS。");
                            return;
                        }
                        if choice == "custom" {
                            apply_selected_facade
                                .set_status("当前是自定义主题，请直接编辑 CSS 或从文件导入。", "info");
                            return;
                        }
                        apply_builtin_theme(&apply_selected_facade, choice.as_str());
                    },
                    "载入所选主题"
                }
            }
            div { class: "preset-grid",
                button {
                    class: "button secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "atlas-sidebar",
                    onclick: move |_| apply_builtin_theme(&atlas_facade, "atlas-sidebar"),
                    "Atlas Sidebar"
                }
                button {
                    class: "button secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "newsprint",
                    onclick: move |_| apply_builtin_theme(&newsprint_facade, "newsprint"),
                    "Newsprint"
                }
                button {
                    class: "button secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "forest-desk",
                    onclick: move |_| apply_builtin_theme(&forest_facade, "forest-desk"),
                    "Amethyst Glass"
                }
                button {
                    class: "button secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "midnight-ledger",
                    onclick: move |_| { apply_builtin_theme(&midnight_facade, "midnight-ledger") },
                    "Midnight Ledger"
                }
                button {
                    class: "button secondary danger-outline",
                    "data-action": "clear-custom-css",
                    onclick: move |_| clear_custom_css(&clear_facade, "已清空自定义 CSS。"),
                    "清空 CSS"
                }
            }
            div { class: "theme-gallery",
                for preset in builtin_theme_presets() {
                    {
                        let is_active = detect_preset_key(&draft.custom_css) == preset.key;
                        let apply_card_facade = facade.clone();
                        let remove_card_facade = facade.clone();
                        let preset_key = preset.key.to_string();
                        let remove_preset_key = preset_key.clone();
                        let preset_name = preset.name;
                        let preset_swatches = preset.swatches;
                        rsx! {
                            article {
                                class: if is_active { "theme-card is-active" } else { "theme-card" },
                                key: "{preset.key}",
                                "data-theme-preset": "{preset.key}",
                                h4 { class: "theme-card__title", "{preset_name}" }
                                div { class: "theme-card__swatches",
                                    for swatch in preset_swatches {
                                        span {
                                            class: "theme-card__swatch",
                                            style: "background:{swatch}",
                                        }
                                    }
                                }
                                button {
                                    class: if is_active { "button" } else { "button secondary" },
                                    "data-action": "apply-theme-preset",
                                    "data-theme-preset": "{preset.key}",
                                    onclick: move |_| {
                                        apply_custom_css_from_raw(
                                            &apply_card_facade,
                                            super::theme_preset::preset_css(preset_key.as_str()).to_string(),
                                            format!("已从主题卡片应用：{}。", preset_name),
                                        );
                                    },
                                    if is_active { "当前已选" } else { "使用这套主题" }
                                }
                                button {
                                    class: "button secondary danger-outline",
                                    "data-action": "remove-theme-preset",
                                    "data-theme-preset": "{preset.key}",
                                    onclick: move |_| {
                                        if detect_preset_key(&remove_card_facade.draft().custom_css) != remove_preset_key.as_str() {
                                            remove_card_facade.set_status(
                                                format!("当前并未启用主题：{}。", preset_name),
                                                "info",
                                            );
                                            return;
                                        }
                                        clear_custom_css(&remove_card_facade, format!("已移除主题：{}。", preset_name));
                                    },
                                    "移除这套主题"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
