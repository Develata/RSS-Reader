use dioxus::prelude::*;
use rssr_domain::UserSettings;

use crate::{status::set_status_info, theme::ThemeController};

use super::{
    theme_apply::{apply_builtin_theme, apply_settings_immediately},
    theme_preset::{builtin_theme_presets, detect_preset_key, preset_css, preset_display_name},
};

#[component]
pub(super) fn ThemePresetSections(
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
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
                    "data-action": "preset-theme-select",
                    value: "{preset_choice}",
                    onchange: move |event| preset_choice.set(event.value()),
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
                        let choice = preset_choice();
                        if choice == "none" {
                            let mut next = draft();
                            next.custom_css.clear();
                            let applied = next.clone();
                            draft.set(next);
                            apply_settings_immediately(
                                theme,
                                draft,
                                preset_choice,
                                status,
                                status_tone,
                                applied,
                                "已清空自定义 CSS。".to_string(),
                            );
                            return;
                        }
                        if choice == "custom" {
                            set_status_info(
                                status,
                                status_tone,
                                "当前是自定义主题，请直接编辑 CSS 或从文件导入。",
                            );
                            return;
                        }
                        let mut next = draft();
                        next.custom_css = preset_css(choice.as_str()).to_string();
                        preset_choice.set(choice.clone());
                        let applied = next.clone();
                        draft.set(next);
                        apply_settings_immediately(
                            theme,
                            draft,
                            preset_choice,
                            status,
                            status_tone,
                            applied,
                            format!("已应用示例主题：{}。", preset_display_name(choice.as_str())),
                        );
                    },
                    "载入所选主题"
                }
            }
            div { class: "preset-grid",
                button {
                    class: "button secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "atlas-sidebar",
                    onclick: move |_| apply_builtin_theme(theme, draft, preset_choice, status, status_tone, "atlas-sidebar", "Atlas Sidebar"),
                    "Atlas Sidebar"
                }
                button {
                    class: "button secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "newsprint",
                    onclick: move |_| apply_builtin_theme(theme, draft, preset_choice, status, status_tone, "newsprint", "Newsprint"),
                    "Newsprint"
                }
                button {
                    class: "button secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "forest-desk",
                    onclick: move |_| apply_builtin_theme(theme, draft, preset_choice, status, status_tone, "forest-desk", "Amethyst Glass"),
                    "Amethyst Glass"
                }
                button {
                    class: "button secondary",
                    "data-action": "apply-theme-preset",
                    "data-theme-preset": "midnight-ledger",
                    onclick: move |_| apply_builtin_theme(theme, draft, preset_choice, status, status_tone, "midnight-ledger", "Midnight Ledger"),
                    "Midnight Ledger"
                }
                button {
                    class: "button secondary danger-outline",
                    "data-action": "clear-custom-css",
                    onclick: move |_| {
                        let mut next = draft();
                        next.custom_css.clear();
                        preset_choice.set("none".to_string());
                        let applied = next.clone();
                        draft.set(next);
                        apply_settings_immediately(
                            theme,
                            draft,
                            preset_choice,
                            status,
                            status_tone,
                            applied,
                            "已清空自定义 CSS。".to_string(),
                        );
                    },
                    "清空 CSS"
                }
            }
            div { class: "theme-gallery", "data-action": "theme-preset-gallery",
                for preset in builtin_theme_presets() {
                    {
                        let is_active = detect_preset_key(&draft().custom_css) == preset.key;
                        let preset_key = preset.key.to_string();
                        let remove_preset_key = preset_key.clone();
                        let preset_name = preset.name;
                        let preset_swatches = preset.swatches;
                        rsx! {
                            article {
                                class: if is_active { "theme-card is-active" } else { "theme-card" },
                                key: "{preset.key}",
                                "data-action": "theme-preset-card",
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
                                        let mut next = draft();
                                        next.custom_css = preset_css(preset_key.as_str()).to_string();
                                        preset_choice.set(preset_key.clone());
                                        let applied = next.clone();
                                        draft.set(next);
                                        apply_settings_immediately(
                                            theme,
                                            draft,
                                            preset_choice,
                                            status,
                                            status_tone,
                                            applied,
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
                                        if detect_preset_key(&draft().custom_css) != remove_preset_key.as_str() {
                                            set_status_info(status, status_tone, format!("当前并未启用主题：{}。", preset_name));
                                            return;
                                        }
                                        let mut next = draft();
                                        next.custom_css.clear();
                                        preset_choice.set("none".to_string());
                                        let applied = next.clone();
                                        draft.set(next);
                                        apply_settings_immediately(
                                            theme,
                                            draft,
                                            preset_choice,
                                            status,
                                            status_tone,
                                            applied,
                                            format!("已移除主题：{}。", preset_name),
                                        );
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
