use super::facade::EntriesPageFacade;
use super::groups::{EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem};
use super::state::EntryGroupingMode;
use crate::components::{entry_filters::EntryFilters, status_banner::StatusBanner};
use dioxus::prelude::*;

pub(super) fn render_entry_controls(facade: &EntriesPageFacade) -> Element {
    let show_controls_facade = facade.clone();
    let grouping_facade = facade.clone();
    let archived_facade = facade.clone();
    let read_filter_facade = facade.clone();
    let starred_filter_facade = facade.clone();
    let selected_sources_facade = facade.clone();
    let hide_controls_facade = facade.clone();
    let search_facade = facade.clone();
    let visible_entries_len = facade.visible_entries_len();
    let archived_count = facade.archived_count();
    let source_filter_options = facade.source_filter_options();
    let group_nav_items: &[EntryGroupNavItem] = facade.group_nav_items();

    rsx! {
        if facade.controls_hidden() {
            div { class: "entry-controls-reveal",
                button {
                    class: "entry-controls-toggle entry-controls-toggle--flat",
                    "data-action": "show-entry-controls",
                    title: "显示筛选与组织",
                    "aria-label": "显示筛选与组织",
                    onclick: move |_| show_controls_facade.set_controls_hidden(false),
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
                        "data-action": if facade.grouping_mode() == EntryGroupingMode::Time { "group-by-time" } else { "group-by-source" },
                        value: match facade.grouping_mode() {
                            EntryGroupingMode::Time => "time",
                            EntryGroupingMode::Source => "source",
                        },
                        onchange: move |event| {
                            grouping_facade.set_grouping_mode(match event.value().as_str() {
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
                            checked: facade.show_archived(),
                            onchange: move |event| archived_facade.set_show_archived(event.checked())
                        }
                        span { "显示已归档文章" }
                    }
                    p { class: "page-intro",
                        if facade.show_archived() {
                            "当前同时显示归档文章。"
                        } else {
                            "默认隐藏超过 {facade.archive_after_months()} 个月的归档文章。"
                        }
                    }
                }
                div { class: "entry-overview",
                    div { class: "entry-overview__metric",
                        span { class: "entry-overview__label", "当前结果" }
                        strong { class: "entry-overview__value", "{visible_entries_len}" }
                    }
                    div { class: "entry-overview__metric",
                        span { class: "entry-overview__label", "归档文章" }
                        strong { class: "entry-overview__value", "{archived_count}" }
                    }
                    div { class: "entry-overview__metric", "data-tone": "summary",
                        span { class: "entry-overview__label", "当前组织" }
                        strong {
                            class: "entry-overview__value",
                            if facade.grouping_mode() == EntryGroupingMode::Time { "按时间" } else { "按来源" }
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
                    search: facade.entry_search(),
                    read_filter: facade.read_filter(),
                    starred_filter: facade.starred_filter(),
                    available_sources: source_filter_options.to_vec(),
                    selected_feed_urls: facade.selected_feed_urls().to_vec(),
                    on_search: move |value| search_facade.set_entry_search(value),
                    on_change_read_filter: move |value| read_filter_facade.set_read_filter(value),
                    on_change_starred_filter: move |value| starred_filter_facade.set_starred_filter(value),
                    on_change_selected_feed_urls: move |value| selected_sources_facade.set_selected_feed_urls(value),
                }
                StatusBanner {
                    message: facade.status().to_string(),
                    tone: facade.status_tone().to_string(),
                }
                if archived_count > 0 && !facade.show_archived() {
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
                        onclick: move |_| hide_controls_facade.set_controls_hidden(true),
                        span { class: "entry-controls-toggle__chevron entry-controls-toggle__chevron--up", aria_hidden: "true" }
                    }
                }
            }
        }
    }
}

pub(super) fn render_entry_directory(
    facade: &EntriesPageFacade,
    grouping_mode: EntryGroupingMode,
    directory_months: &[EntryDirectoryMonth],
    directory_sources: &[EntryDirectorySource],
) -> Element {
    let expanded_directory_sources = facade.expanded_directory_sources();

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
                            let is_open = expanded_directory_sources.contains(&anchor_id);
                            let toggle_anchor = anchor_id.clone();
                            let toggle_facade = facade.clone();
                            rsx! {
                                div { class: "entry-directory-rail__subsection", key: "{anchor_id}",
                                    button {
                                        class: "entry-directory-rail__toggle",
                                        aria_expanded: if is_open { "true" } else { "false" },
                                        "data-action": if is_open { "collapse-directory-source" } else { "expand-directory-source" },
                                        onclick: move |_| {
                                            toggle_facade.toggle_directory_source(toggle_anchor.clone());
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
