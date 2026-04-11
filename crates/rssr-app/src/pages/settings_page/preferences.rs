use dioxus::prelude::*;
use rssr_domain::{ListDensity, StartupView, ThemeMode};

use super::facade::SettingsPageFacade;

#[component]
pub(crate) fn ReadingPreferencesSection(facade: SettingsPageFacade) -> Element {
    let draft = facade.draft();
    let theme_facade = facade.clone();
    let density_facade = facade.clone();
    let startup_facade = facade.clone();
    let refresh_facade = facade.clone();
    let archive_facade = facade.clone();
    let font_scale_facade = facade.clone();

    rsx! {
        div { class: "settings-card__section", "data-layout": "settings-card-section", "data-section": "reading-preferences",
            div { class: "settings-card__section-header", "data-slot": "settings-card-section-header",
                h4 { class: "settings-card__section-title", "data-slot": "settings-card-section-title", "阅读节奏" }
            }
            div { class: "settings-form-grid", "data-layout": "settings-form-grid",
                div { class: "settings-form-grid__item", "data-slot": "settings-form-grid-item",
                    label { class: "field-label", r#for: "settings-theme-mode", "主题" }
                    select {
                        id: "settings-theme-mode",
                        name: "theme_mode",
                        class: "select-input",
                        "data-field": "theme-mode",
                        value: "{theme_value(draft.theme)}",
                        onchange: move |event| {
                            theme_facade.update_draft(|next| {
                                next.theme = parse_theme_mode(&event.value());
                            });
                        },
                        option { value: "system", "跟随系统" }
                        option { value: "light", "浅色" }
                        option { value: "dark", "深色" }
                    }
                }
                div { class: "settings-form-grid__item", "data-slot": "settings-form-grid-item",
                    label { class: "field-label", r#for: "settings-list-density", "列表密度" }
                    select {
                        id: "settings-list-density",
                        name: "list_density",
                        class: "select-input",
                        "data-field": "list-density",
                        value: "{density_value(draft.list_density)}",
                        onchange: move |event| {
                            density_facade.update_draft(|next| {
                                next.list_density = parse_list_density(&event.value());
                            });
                        },
                        option { value: "comfortable", "舒适" }
                        option { value: "compact", "紧凑" }
                    }
                }
                div { class: "settings-form-grid__item", "data-slot": "settings-form-grid-item",
                    label { class: "field-label", r#for: "settings-startup-view", "启动视图" }
                    select {
                        id: "settings-startup-view",
                        name: "startup_view",
                        class: "select-input",
                        "data-field": "startup-view",
                        value: "{startup_value(draft.startup_view)}",
                        onchange: move |event| {
                            startup_facade.update_draft(|next| {
                                next.startup_view = parse_startup_view(&event.value());
                            });
                        },
                        option { value: "all", "全部文章" }
                        option { value: "last_feed", "上次订阅" }
                    }
                }
                div { class: "settings-form-grid__item", "data-slot": "settings-form-grid-item",
                    label { class: "field-label", r#for: "settings-refresh-interval", "刷新间隔（分钟）" }
                    input {
                        id: "settings-refresh-interval",
                        name: "refresh_interval_minutes",
                        class: "text-input",
                        "data-field": "refresh-interval",
                        value: "{draft.refresh_interval_minutes}",
                        oninput: move |event| {
                            if let Ok(minutes) = event.value().parse::<u32>() {
                                refresh_facade.update_draft(|next| {
                                    next.refresh_interval_minutes = minutes;
                                });
                            }
                        }
                    }
                }
                div { class: "settings-form-grid__item", "data-slot": "settings-form-grid-item",
                    label { class: "field-label", r#for: "settings-archive-after-months", "自动归档阈值（月）" }
                    input {
                        id: "settings-archive-after-months",
                        name: "archive_after_months",
                        class: "text-input",
                        "data-field": "archive-after-months",
                        value: "{draft.archive_after_months}",
                        oninput: move |event| {
                            if let Ok(months) = event.value().parse::<u32>() {
                                archive_facade.update_draft(|next| {
                                    next.archive_after_months = months;
                                });
                            }
                        }
                    }
                }
                div { class: "settings-form-grid__item", "data-slot": "settings-form-grid-item",
                    label { class: "field-label", r#for: "settings-reader-font-scale", "阅读字号缩放" }
                    input {
                        id: "settings-reader-font-scale",
                        name: "reader_font_scale",
                        class: "text-input",
                        "data-field": "reader-font-scale",
                        value: "{draft.reader_font_scale}",
                        oninput: move |event| {
                            if let Ok(scale) = event.value().parse::<f32>() {
                                font_scale_facade.update_draft(|next| {
                                    next.reader_font_scale = scale;
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

fn theme_value(value: ThemeMode) -> &'static str {
    match value {
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
        ThemeMode::System => "system",
    }
}

fn parse_theme_mode(raw: &str) -> ThemeMode {
    match raw {
        "light" => ThemeMode::Light,
        "dark" => ThemeMode::Dark,
        _ => ThemeMode::System,
    }
}

fn density_value(value: ListDensity) -> &'static str {
    match value {
        ListDensity::Comfortable => "comfortable",
        ListDensity::Compact => "compact",
    }
}

fn parse_list_density(raw: &str) -> ListDensity {
    match raw {
        "compact" => ListDensity::Compact,
        _ => ListDensity::Comfortable,
    }
}

fn startup_value(value: StartupView) -> &'static str {
    match value {
        StartupView::All => "all",
        StartupView::LastFeed => "last_feed",
    }
}

fn parse_startup_view(raw: &str) -> StartupView {
    match raw {
        "last_feed" => StartupView::LastFeed,
        _ => StartupView::All,
    }
}
