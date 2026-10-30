use dioxus::prelude::*;
use rssr_domain::{
    DEFAULT_ENTRIES_PAGE_SIZE, MAX_ARCHIVE_AFTER_MONTHS, MAX_ENTRIES_PAGE_SIZE,
    MAX_REFRESH_INTERVAL_MINUTES, StartupView, ThemeMode,
    validation::{MAX_READER_FONT_SCALE, MIN_READER_FONT_SCALE},
};

use super::facade::SettingsPageFacade;

#[component]
pub(crate) fn ReadingPreferencesSection(facade: SettingsPageFacade) -> Element {
    let draft = facade.draft();
    let theme_facade = facade.clone();
    let startup_facade = facade.clone();
    let refresh_facade = facade.clone();
    let archive_facade = facade.clone();
    let entries_page_size_facade = facade.clone();
    let font_scale_facade = facade.clone();
    let mut invalid = use_signal(InvalidNumericFields::default);
    let [refresh_revision, archive_revision, page_size_revision, font_scale_revision] =
        facade.numeric_revisions();

    rsx! {
        div { "data-layout": "settings-card-section", "data-section": "reading-preferences",
            div { "data-slot": "settings-card-section-header",
                h3 { "data-slot": "settings-card-section-title", "阅读节奏" }
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
                                Err(_) => {
                                    refresh_facade.note_draft_edit();
                                    invalid.write().refresh_interval =
                                        Some(refresh_facade.numeric_revisions()[0]);
                                }
                            }
                        }
                    }
                    if invalid().refresh_interval == Some(refresh_revision) {
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
                                Err(_) => {
                                    archive_facade.note_draft_edit();
                                    invalid.write().archive_after_months =
                                        Some(archive_facade.numeric_revisions()[1]);
                                }
                            }
                        }
                    }
                    if invalid().archive_after_months == Some(archive_revision) {
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
                                    entries_page_size_facade.note_draft_edit();
                                    invalid.write().entries_page_size =
                                        Some(entries_page_size_facade.numeric_revisions()[2]);
                                }
                            }
                        }
                    }
                    if invalid().entries_page_size == Some(page_size_revision) {
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
                        min: format!("{MIN_READER_FONT_SCALE}"),
                        max: format!("{MAX_READER_FONT_SCALE}"),
                        step: "0.05",
                        "data-field": "reader-font-scale",
                        value: format!("{}", draft.reader_font_scale),
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
                                    font_scale_facade.note_draft_edit();
                                    invalid.write().reader_font_scale =
                                        Some(font_scale_facade.numeric_revisions()[3]);
                                }
                            }
                        }
                    }
                    if invalid().reader_font_scale == Some(font_scale_revision) {
                        p { "data-slot": "page-intro", "data-state": "invalid", "{NUMERIC_INPUT_HINT}" }
                    }
                }
            }
        }
    }
}

/// Each parse error belongs to a field's projection. Stored settings retire it
/// only when that field has no pending/submitted edit, even for equal values.
/// An A -> B -> A projection cannot revive it. Parse failures retain the last
/// parsed value and do not create a SettingsPatch, matching the main form contract.
/// Rendering only compares revisions; it neither writes signals nor emits input.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct InvalidNumericFields {
    refresh_interval: Option<u64>,
    archive_after_months: Option<u64>,
    entries_page_size: Option<u64>,
    reader_font_scale: Option<u64>,
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
