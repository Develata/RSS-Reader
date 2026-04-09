use dioxus::prelude::*;

use crate::pages::settings_page::facade::SettingsPageFacade;

use super::{theme_apply::apply_custom_css_from_raw, theme_preset::builtin_theme_presets};

#[component]
pub(super) fn ThemePresetSections(facade: SettingsPageFacade) -> Element {
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
                    class: "button",
                    "data-variant": "secondary",
                    "data-action": "apply-selected-theme",
                    onclick: move |_| apply_selected_facade.apply_selected_theme(),
                    "载入所选主题"
                }
            }
            div { class: "preset-grid",
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "atlas-sidebar",
                    onclick: move |_| atlas_facade.apply_builtin_theme("atlas-sidebar"),
                    "Atlas Sidebar"
                }
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "newsprint",
                    onclick: move |_| newsprint_facade.apply_builtin_theme("newsprint"),
                    "Newsprint"
                }
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "forest-desk",
                    onclick: move |_| forest_facade.apply_builtin_theme("forest-desk"),
                    "Amethyst Glass"
                }
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "midnight-ledger",
                    onclick: move |_| { midnight_facade.apply_builtin_theme("midnight-ledger") },
                    "Midnight Ledger"
                }
                button {
                    class: "button",
                    "data-variant": "danger-outline",
                    "data-action": "clear-custom-css",
                    onclick: move |_| clear_facade.clear_custom_css("已清空自定义 CSS。"),
                    "清空 CSS"
                }
            }
            div { class: "theme-gallery",
                for preset in builtin_theme_presets() {
                    {
                        let apply_card_facade = facade.clone();
                        let remove_card_facade = facade.clone();
                        let preset_key = preset.key.to_string();
                        let remove_preset_key = preset_key.clone();
                        let preset_name = preset.name;
                        let preset_swatches = preset.swatches;
                        rsx! {
                            article {
                                class: "theme-card",
                                key: "{preset.key}",
                                "data-state": "{facade.theme_card_state(preset.key)}",
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
                                    class: "button",
                                    "data-variant": "{facade.theme_apply_button_variant(preset.key)}",
                                    "data-action": "apply-theme-preset",
                                    "data-theme-preset": "{preset.key}",
                                    onclick: move |_| {
                                        apply_custom_css_from_raw(
                                            &apply_card_facade,
                                            super::theme_preset::preset_css(preset_key.as_str()).to_string(),
                                            format!("已从主题卡片应用：{}。", preset_name),
                                        );
                                    },
                                    "{facade.theme_apply_button_label(preset.key)}"
                                }
                                button {
                                    class: "button",
                                    "data-variant": "danger-outline",
                                    "data-action": "remove-theme-preset",
                                    "data-theme-preset": "{preset.key}",
                                    onclick: move |_| remove_card_facade.remove_theme_preset(remove_preset_key.as_str(), preset_name),
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
