use dioxus::prelude::*;

use crate::pages::settings_page::facade::SettingsPageFacade;

use super::theme_preset::builtin_theme_presets;

#[component]
pub(super) fn ThemePresetSections(facade: SettingsPageFacade) -> Element {
    let default_facade = facade.clone();
    let default_active = facade.is_theme_preset_active("none");
    let default_state = facade.theme_card_state("none");
    let default_variant = facade.theme_apply_button_variant("none");
    let default_label =
        if default_active { "当前已选：默认主题" } else { "恢复默认主题" };

    rsx! {
        div {
            "data-layout": "theme-presets",
            "data-section": "settings-theme-presets",
            div { "data-slot": "settings-card-section-header",
                h3 { "data-slot": "settings-card-section-title", "主题" }
                p { "data-slot": "page-intro", "选择后预览，点击保存设置以保留。" }
            }
            if facade.preset_choice() == "custom" {
                p { "data-slot": "page-intro", "当前草稿使用自定义 CSS，可在高级设置中编辑。" }
            }
            div { "data-layout": "theme-gallery",
                article {
                    "data-layout": "theme-card",
                    "data-state": default_state,
                    "data-theme-preset": "none",
                    h4 { "data-slot": "theme-card-title", "默认主题" }
                    p { "data-slot": "page-intro", "使用上方选择的浅色、深色或系统外观。" }
                    button {
                        class: "button",
                        "data-variant": default_variant,
                        "data-action": "clear-custom-css",
                        aria_pressed: default_active,
                        aria_label: default_label,
                        onclick: move |_| default_facade.clear_custom_css("正在预览默认主题。"),
                        if default_active { "当前已选" } else { "恢复默认" }
                    }
                }
                for preset in builtin_theme_presets() {
                    {
                        let apply_card_facade = facade.clone();
                        let preset_key = preset.key.to_string();
                        let preset_name = preset.name;
                        let preset_swatches = preset.swatches;
                        let button_label = facade.theme_apply_button_label(preset.key);
                        rsx! {
                            article {
                                key: "{preset.key}",
                                "data-layout": "theme-card",
                                "data-state": "{facade.theme_card_state(preset.key)}",
                                "data-theme-preset": "{preset.key}",
                                h4 { "data-slot": "theme-card-title", "{preset_name}" }
                                div { "data-slot": "theme-card-swatches", aria_hidden: "true",
                                    for swatch in preset_swatches {
                                        span { "data-slot": "theme-card-swatch", style: "background:{swatch}" }
                                    }
                                }
                                button {
                                    class: "button",
                                    "data-variant": "{facade.theme_apply_button_variant(preset.key)}",
                                    "data-action": "apply-theme-preset",
                                    "data-theme-preset": "{preset.key}",
                                    aria_pressed: facade.is_theme_preset_active(preset.key),
                                    aria_label: "{button_label}：{preset_name}",
                                    onclick: move |_| apply_card_facade.apply_builtin_theme(&preset_key),
                                    "{button_label}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
