use dioxus::prelude::*;
use rssr_domain::{EntryGroupingPreference, EntrySummary, FeedSummary, ReadFilter, StarredFilter};
use std::collections::BTreeSet;

use super::entries_page_groups::{EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem};
use crate::{
    app::AppUiState,
    components::{entry_filters::EntryFilters, status_banner::StatusBanner},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntryGroupingMode {
    Time,
    Source,
}

#[derive(Clone, Copy)]
pub(super) struct EntryControlsProps<'a> {
    pub ui: AppUiState,
    pub controls_hidden: Signal<bool>,
    pub grouping_mode: Signal<EntryGroupingMode>,
    pub show_archived: Signal<bool>,
    pub archive_after_months: Signal<u32>,
    pub visible_entries: &'a [EntrySummary],
    pub archived_count: usize,
    pub source_filter_options: &'a [(i64, String, String)],
    pub read_filter: Signal<ReadFilter>,
    pub starred_filter: Signal<StarredFilter>,
    pub selected_feed_urls: Signal<Vec<String>>,
    pub group_nav_items: &'a [EntryGroupNavItem],
    pub status: Signal<String>,
    pub status_tone: Signal<String>,
}

pub(super) fn render_entry_controls(props: EntryControlsProps<'_>) -> Element {
    let EntryControlsProps {
        mut ui,
        mut controls_hidden,
        mut grouping_mode,
        mut show_archived,
        archive_after_months,
        visible_entries,
        archived_count,
        source_filter_options,
        mut read_filter,
        mut starred_filter,
        mut selected_feed_urls,
        group_nav_items,
        status,
        status_tone,
    } = props;

    rsx! {
        if controls_hidden() {
            div { class: "entry-controls-reveal",
                button {
                    class: "entry-controls-toggle entry-controls-toggle--flat",
                    "data-action": "show-entry-controls",
                    title: "显示筛选与组织",
                    "aria-label": "显示筛选与组织",
                    onclick: move |_| {
                        remember_entry_controls_hidden(false);
                        controls_hidden.set(false);
                    },
                    span { class: "entry-controls-toggle__chevron entry-controls-toggle__chevron--down", aria_hidden: "true" }
                }
            }
        } else {
            div { class: "entry-controls-panel",
                div { class: "entry-organize-bar",
                    label { class: "field-label", r#for: "entry-grouping-mode", "组织方式" }
                    select {
                        id: "entry-grouping-mode",
                        class: "select-input",
                        "data-action": if grouping_mode() == EntryGroupingMode::Time { "group-by-time" } else { "group-by-source" },
                        value: match grouping_mode() {
                            EntryGroupingMode::Time => "time",
                            EntryGroupingMode::Source => "source",
                        },
                        onchange: move |event| {
                            grouping_mode.set(match event.value().as_str() {
                                "source" => EntryGroupingMode::Source,
                                _ => EntryGroupingMode::Time,
                            });
                        },
                        option { value: "time", "按时间" }
                        option { value: "source", "按来源" }
                    }
                    label { class: "entry-filters__toggle",
                        input {
                            r#type: "checkbox",
                            "data-action": "toggle-archived",
                            checked: show_archived(),
                            onchange: move |event| show_archived.set(event.checked())
                        }
                        span { "显示已归档文章" }
                    }
                    p { class: "page-intro",
                        if show_archived() {
                            "当前同时显示归档文章。"
                        } else {
                            "默认隐藏超过 {archive_after_months()} 个月的归档文章。"
                        }
                    }
                }
                div { class: "entry-overview",
                    div { class: "entry-overview__metric",
                        span { class: "entry-overview__label", "当前结果" }
                        strong { class: "entry-overview__value", "{visible_entries.len()}" }
                    }
                    div { class: "entry-overview__metric",
                        span { class: "entry-overview__label", "归档文章" }
                        strong { class: "entry-overview__value", "{archived_count}" }
                    }
                    div { class: "entry-overview__metric", "data-tone": "summary",
                        span { class: "entry-overview__label", "当前组织" }
                        strong {
                            class: "entry-overview__value",
                            if grouping_mode() == EntryGroupingMode::Time { "按时间" } else { "按来源" }
                        }
                    }
                }
                if !group_nav_items.is_empty() {
                    nav { class: "entry-top-directory", "aria-label": "文章目录",
                        for item in group_nav_items {
                            button {
                                class: "entry-top-directory__chip",
                                r#type: "button",
                                onclick: {
                                    let anchor_id = item.anchor_id.clone();
                                    move |_| scroll_to_entry_group(&anchor_id)
                                },
                                span { class: "entry-top-directory__title", "{item.title}" }
                                span { class: "entry-top-directory__meta", "{item.subtitle}" }
                            }
                        }
                    }
                }
                EntryFilters {
                    search: (ui.entry_search)(),
                    read_filter: read_filter(),
                    starred_filter: starred_filter(),
                    available_sources: source_filter_options.to_vec(),
                    selected_feed_urls: selected_feed_urls(),
                    on_search: move |value| ui.entry_search.set(value),
                    on_change_read_filter: move |value| read_filter.set(value),
                    on_change_starred_filter: move |value| starred_filter.set(value),
                    on_change_selected_feed_urls: move |value| selected_feed_urls.set(value),
                }
                StatusBanner { message: status(), tone: status_tone() }
                if archived_count > 0 && !show_archived() {
                    StatusBanner {
                        message: format!("当前已自动归档 {} 篇较旧文章，可勾选“显示已归档文章”查看。", archived_count),
                        tone: "info".to_string()
                    }
                }
                div { class: "entry-controls-reveal",
                    button {
                        class: "entry-controls-toggle entry-controls-toggle--flat",
                        "data-action": "hide-entry-controls",
                        title: "收起筛选与组织",
                        "aria-label": "收起筛选与组织",
                        onclick: move |_| {
                            remember_entry_controls_hidden(true);
                            controls_hidden.set(true);
                        },
                        span { class: "entry-controls-toggle__chevron entry-controls-toggle__chevron--up", aria_hidden: "true" }
                    }
                }
            }
        }
    }
}

pub(super) fn render_entry_directory(
    grouping_mode: EntryGroupingMode,
    directory_months: &[EntryDirectoryMonth],
    directory_sources: &[EntryDirectorySource],
    mut expanded_directory_sources: Signal<BTreeSet<String>>,
) -> Element {
    rsx! {
        aside { class: "entry-directory-rail",
            h3 { class: "entry-directory-rail__title", "目录" }
            if grouping_mode == EntryGroupingMode::Time {
                nav { class: "entry-directory-rail__nav", "aria-label": "文章目录导航",
                    for month in directory_months {
                        div { class: "entry-directory-rail__section", key: "{month.anchor_id}",
                            button {
                                class: "entry-directory-rail__link entry-directory-rail__link--month",
                                r#type: "button",
                                onclick: {
                                    let anchor_id = month.anchor_id.clone();
                                    move |_| scroll_to_entry_group(&anchor_id)
                                },
                                span { class: "entry-directory-rail__link-title", "{month.title}" }
                                span { class: "entry-directory-rail__link-meta", "{month.subtitle}" }
                            }
                            div { class: "entry-directory-rail__children",
                                for date in &month.dates {
                                    button {
                                        class: "entry-directory-rail__link entry-directory-rail__link--date",
                                        r#type: "button",
                                        onclick: {
                                            let anchor_id = date.anchor_id.clone();
                                            move |_| scroll_to_entry_group(&anchor_id)
                                        },
                                        span { class: "entry-directory-rail__link-title", "{date.title}" }
                                        span { class: "entry-directory-rail__link-meta", "{date.subtitle}" }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                nav { class: "entry-directory-rail__nav", "aria-label": "文章目录导航",
                    for source in directory_sources {
                        {
                            let anchor_id = source.anchor_id.clone();
                            let is_open = expanded_directory_sources().contains(&anchor_id);
                            let toggle_anchor = anchor_id.clone();
                            rsx! {
                                div { class: "entry-directory-rail__subsection", key: "{anchor_id}",
                                    button {
                                        class: "entry-directory-rail__toggle",
                                        aria_expanded: if is_open { "true" } else { "false" },
                                        "data-action": if is_open { "collapse-directory-source" } else { "expand-directory-source" },
                                        onclick: move |_| {
                                            let mut next = expanded_directory_sources();
                                            if !next.insert(toggle_anchor.clone()) {
                                                next.remove(&toggle_anchor);
                                            }
                                            expanded_directory_sources.set(next);
                                        },
                                        span { class: "entry-directory-rail__toggle-text", "{source.title}" }
                                        span { class: "entry-directory-rail__toggle-meta", "{source.subtitle}" }
                                    }
                                    if is_open {
                                        div { class: "entry-directory-rail__grandchildren",
                                            for month in &source.months {
                                                button {
                                                    class: "entry-directory-rail__link",
                                                    r#type: "button",
                                                    onclick: {
                                                        let anchor_id = month.anchor_id.clone();
                                                        move |_| scroll_to_entry_group(&anchor_id)
                                                    },
                                                    span { class: "entry-directory-rail__link-title", "{month.title}" }
                                                    span { class: "entry-directory-rail__link-meta", "{month.subtitle}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn entry_grouping_mode_from_preference(
    preference: EntryGroupingPreference,
) -> EntryGroupingMode {
    match preference {
        EntryGroupingPreference::Time => EntryGroupingMode::Time,
        EntryGroupingPreference::Source => EntryGroupingMode::Source,
    }
}

pub(super) fn grouping_mode_preference(mode: EntryGroupingMode) -> EntryGroupingPreference {
    match mode {
        EntryGroupingMode::Time => EntryGroupingPreference::Time,
        EntryGroupingMode::Source => EntryGroupingPreference::Source,
    }
}

pub(super) fn map_selected_feed_urls_to_ids(
    feeds: &[FeedSummary],
    selected_feed_urls: &[String],
) -> Vec<i64> {
    if selected_feed_urls.is_empty() {
        return Vec::new();
    }

    let selected = selected_feed_urls.iter().map(String::as_str).collect::<BTreeSet<_>>();
    feeds
        .iter()
        .filter(|feed| selected.contains(feed.url.as_str()))
        .map(|feed| feed.id)
        .collect::<Vec<_>>()
}

#[cfg(target_arch = "wasm32")]
fn initial_entry_controls_hidden_impl() -> Option<bool> {
    if let Some(window) = web_sys::window()
        && let Ok(Some(storage)) = window.local_storage()
        && let Ok(Some(value)) = storage.get_item("rssr-entry-controls-hidden")
    {
        return Some(value == "1");
    }

    None
}

pub(super) fn initial_entry_controls_hidden() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        return initial_entry_controls_hidden_impl().unwrap_or(true);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}

pub(super) fn remember_entry_controls_hidden(hidden: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
        {
            let _ = storage.set_item("rssr-entry-controls-hidden", if hidden { "1" } else { "0" });
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    let _ = hidden;
}

pub(super) fn scroll_to_entry_group(anchor_id: &str) {
    let Ok(anchor_id_json) = serde_json::to_string(anchor_id) else {
        return;
    };

    document::eval(&format!(
        r#"
        const targetId = {anchor_id_json};
        const scrollToTarget = () => {{
            const element = document.getElementById(targetId);
            if (!element) {{
                return false;
            }}

            if (window.location.hash !== `#${{targetId}}`) {{
                window.location.hash = targetId;
            }}

            element.scrollIntoView({{ behavior: "smooth", block: "start", inline: "nearest" }});
            return true;
        }};

        if (!scrollToTarget()) {{
            requestAnimationFrame(scrollToTarget);
        }} else {{
            requestAnimationFrame(scrollToTarget);
        }}
        "#
    ));
}
