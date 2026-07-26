use dioxus::prelude::*;
use rssr_domain::{
    DEFAULT_ENTRIES_PAGE_SIZE, ListDensity, MAX_ARCHIVE_AFTER_MONTHS, MAX_ENTRIES_PAGE_SIZE,
    MAX_REFRESH_INTERVAL_MINUTES, StartupView, ThemeMode,
    validation::{MAX_READER_FONT_SCALE, MIN_READER_FONT_SCALE},
};

use super::facade::SettingsPageFacade;

#[component]
pub(crate) fn ReadingPreferencesSection(facade: SettingsPageFacade) -> Element {
    let draft = facade.draft();
    let theme_facade = facade.clone();
    let density_facade = facade.clone();
    let startup_facade = facade.clone();
    let refresh_facade = facade.clone();
    let archive_facade = facade.clone();
    let entries_page_size_facade = facade.clone();
    let font_scale_facade = facade.clone();
    let mut invalid = use_signal(InvalidNumericFields::default);
    // 置位时记下当时的草稿值；渲染时只有仍然相等才显示提示。草稿被外部整体替换
    // （从 WebDAV 拉取配置、恢复设置）后，值不再匹配，提示自动失效。
    let refresh_value = draft.refresh_interval_minutes;
    let archive_value = draft.archive_after_months;
    let page_size_value = draft.entries_page_size;
    let font_scale_value = draft.reader_font_scale;

    rsx! {
        div { "data-layout": "settings-card-section", "data-section": "reading-preferences",
            div { "data-slot": "settings-card-section-header",
                h4 { "data-slot": "settings-card-section-title", "阅读节奏" }
            }
            div { "data-layout": "settings-form-grid",
                div { "data-slot": "settings-form-grid-item",
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
                div { "data-slot": "settings-form-grid-item",
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
                div { "data-slot": "settings-form-grid-item",
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
                div { "data-slot": "settings-form-grid-item",
                    label { class: "field-label", r#for: "settings-refresh-interval", "刷新间隔（分钟）" }
                    input {
                        id: "settings-refresh-interval",
                        name: "refresh_interval_minutes",
                        class: "text-input",
                        r#type: "number",
                        min: "1",
                        max: "{MAX_REFRESH_INTERVAL_MINUTES}",
                        step: "1",
                        "data-field": "refresh-interval",
                        value: "{draft.refresh_interval_minutes}",
                        oninput: move |event| {
                            match event.value().parse::<u32>() {
                                Ok(minutes) => {
                                    invalid.write().refresh_interval = None;
                                    refresh_facade
                                        .update_draft(|next| {
                                            next.refresh_interval_minutes = minutes;
                                        });
                                }
                                Err(_) => invalid.write().refresh_interval = Some(refresh_value),
                            }
                        }
                    }
                    if invalid().refresh_interval == Some(refresh_value) {
                        p { "data-slot": "page-intro", "data-state": "invalid", "{NUMERIC_INPUT_HINT}" }
                    }
                }
                div { "data-slot": "settings-form-grid-item",
                    label { class: "field-label", r#for: "settings-archive-after-months", "自动归档阈值（月）" }
                    input {
                        id: "settings-archive-after-months",
                        name: "archive_after_months",
                        class: "text-input",
                        r#type: "number",
                        min: "1",
                        max: "{MAX_ARCHIVE_AFTER_MONTHS}",
                        step: "1",
                        "data-field": "archive-after-months",
                        value: "{draft.archive_after_months}",
                        oninput: move |event| {
                            match event.value().parse::<u32>() {
                                Ok(months) => {
                                    invalid.write().archive_after_months = None;
                                    archive_facade
                                        .update_draft(|next| {
                                            next.archive_after_months = months;
                                        });
                                }
                                Err(_) => invalid.write().archive_after_months = Some(archive_value),
                            }
                        }
                    }
                    if invalid().archive_after_months == Some(archive_value) {
                        p { "data-slot": "page-intro", "data-state": "invalid", "{NUMERIC_INPUT_HINT}" }
                    }
                }
                div { "data-slot": "settings-form-grid-item",
                    label { class: "field-label", r#for: "settings-entries-page-size", "文章页每页数量" }
                    input {
                        id: "settings-entries-page-size",
                        name: "entries_page_size",
                        class: "text-input",
                        r#type: "number",
                        min: "0",
                        max: "{MAX_ENTRIES_PAGE_SIZE}",
                        step: "1",
                        "data-field": "entries-page-size",
                        value: "{draft.entries_page_size}",
                        oninput: move |event| {
                            match event.value().parse::<u32>() {
                                Ok(size) => {
                                    invalid.write().entries_page_size = None;
                                    entries_page_size_facade
                                        .update_draft(|next| {
                                            next.entries_page_size = size;
                                        });
                                }
                                Err(_) => {
                                    invalid.write().entries_page_size = Some(page_size_value);
                                }
                            }
                        }
                    }
                    if invalid().entries_page_size == Some(page_size_value) {
                        p { "data-slot": "page-intro", "data-state": "invalid", "{NUMERIC_INPUT_HINT}" }
                    }
                    p { "data-slot": "page-intro",
                        "建议设置为 80 到 100；输入 0 时保存会自动回退到默认值 {DEFAULT_ENTRIES_PAGE_SIZE}。"
                    }
                }
                div { "data-slot": "settings-form-grid-item",
                    label { class: "field-label", r#for: "settings-reader-font-scale", "阅读字号缩放" }
                    input {
                        id: "settings-reader-font-scale",
                        name: "reader_font_scale",
                        class: "text-input",
                        r#type: "number",
                        min: "{MIN_READER_FONT_SCALE}",
                        max: "{MAX_READER_FONT_SCALE}",
                        step: "0.05",
                        "data-field": "reader-font-scale",
                        value: "{draft.reader_font_scale}",
                        oninput: move |event| {
                            match event.value().parse::<f32>() {
                                Ok(scale) => {
                                    invalid.write().reader_font_scale = None;
                                    font_scale_facade
                                        .update_draft(|next| {
                                            next.reader_font_scale = scale;
                                        });
                                }
                                Err(_) => {
                                    invalid.write().reader_font_scale = Some(font_scale_value);
                                }
                            }
                        }
                    }
                    if invalid().reader_font_scale == Some(font_scale_value) {
                        p { "data-slot": "page-intro", "data-state": "invalid", "{NUMERIC_INPUT_HINT}" }
                    }
                }
            }
        }
    }
}

/// 哪些数值输入框的内容当前无法解析，以及**置位时草稿里的值**。
///
/// 这几个框此前是 `if let Ok(..)` 没有 `else`：清空或输入非数字时草稿悄悄保留旧值，
/// 界面显示的却是用户刚输入的内容，保存写回的是旧值且全程无提示。这里只补提示，
/// 不改变「解析失败就不写入草稿」的既有行为。
///
/// 记的是值而不是一个布尔，是为了让提示能自己失效：草稿被整体替换后
/// （`SettingsPageSession::apply_loaded_settings` / `restore_settings`，对应「从 WebDAV
/// 拉取配置」与「恢复设置」），这里存的值与新草稿不再相等，提示自动不再显示。
/// 用布尔的话，提示会残留在一个已经被换成合法值的输入框下面，说着「当前输入不是有效数字」。
///
/// 这样做不需要 `use_effect`：判定发生在渲染期的比较里，不往信号里写东西，
/// 因此也不会给每次按键多加一次渲染。
#[derive(Clone, Copy, PartialEq, Default)]
struct InvalidNumericFields {
    refresh_interval: Option<u32>,
    archive_after_months: Option<u32>,
    entries_page_size: Option<u32>,
    reader_font_scale: Option<f32>,
}

/// 只在输入无法解析时出现，复用 `page-intro` 槽位，不引入新的样式面。
const NUMERIC_INPUT_HINT: &str = "当前输入不是有效数字，尚未记入草稿；保存时会沿用上一次的有效值。";

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
