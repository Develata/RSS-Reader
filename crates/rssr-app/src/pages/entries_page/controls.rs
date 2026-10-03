use super::directory::DirectoryTop;
use super::facade::EntriesPageFacade;
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
    let clear_filters_facade = facade.clone();
    let visible_entries_len = facade.visible_entries_len();
    let archived_count = facade.archived_entry_count();
    let source_filter_options = facade.source_filter_options();
    let model = facade.directory_model();

    rsx! {
        if let Some(summary) = facade.active_filter_summary() {
            div { "data-layout": "entry-filter-summary", role: "group", aria_label: "当前筛选",
                p { "data-slot": "entry-filter-summary-text", "筛选：{summary}" }
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-action": "clear-entry-filters",
                    onclick: move |_| clear_filters_facade.clear_filters(),
                    "清除筛选"
                }
            }
        }
        if facade.controls_hidden() {
            div { "data-layout": "entry-controls-reveal",
                button {
                    "data-layout": "entry-controls-toggle",
                    "data-action": "show-entry-controls",
                    title: "显示筛选与组织",
                    "aria-label": "显示筛选与组织",
                    onclick: move |_| show_controls_facade.set_controls_hidden(false),
                    span {
                        "data-slot": "entry-controls-toggle-chevron",
                        "data-direction": "down",
                        aria_hidden: "true"
                    }
                    span { "data-slot": "entry-controls-toggle-label", "筛选与组织" }
                }
            }
        } else {
            div { "data-layout": "entry-controls-panel",
                div { "data-layout": "entry-organize-bar",
                    label { class: "field-label", r#for: "entry-grouping-mode", "组织方式" }
                    select {
                        id: "entry-grouping-mode",
                        class: "select-input",
                        "data-field": "entry-grouping-mode",
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
                    label {
                        input {
                            r#type: "checkbox",
                            "data-field": "show-archived",
                            checked: facade.show_archived(),
                            onchange: move |event| archived_facade.set_show_archived(event.checked())
                        }
                        span { "显示已归档文章" }
                    }
                    p { "data-slot": "page-intro",
                        if facade.show_archived() {
                            "当前同时显示归档文章。"
                        } else {
                            "默认隐藏超过 {facade.archive_after_months()} 个月的归档文章。"
                        }
                    }
                }
                div { "data-layout": "entry-overview",
                    div { "data-layout": "entry-overview-metric", "data-tone": "primary",
                        span { "data-slot": "entry-overview-label", "当前结果" }
                        strong { "data-slot": "entry-overview-value", "{visible_entries_len}" }
                    }
                    div { "data-layout": "entry-overview-metric", "data-tone": "secondary",
                        span { "data-slot": "entry-overview-label", "每页数量" }
                        strong { "data-slot": "entry-overview-value", "{facade.page_size()}" }
                    }
                    div { "data-layout": "entry-overview-metric", "data-tone": "secondary",
                        span { "data-slot": "entry-overview-label", "归档文章" }
                        strong { "data-slot": "entry-overview-value", "{archived_count}" }
                    }
                    div { "data-layout": "entry-overview-metric", "data-tone": "secondary",
                        span { "data-slot": "entry-overview-label", "当前组织" }
                        strong {
                            "data-slot": "entry-overview-value",
                            if facade.grouping_mode() == EntryGroupingMode::Time { "按时间" } else { "按来源" }
                        }
                    }
                }
                if !model.group_nav_items.is_empty() {
                    DirectoryTop { model: model.clone() }
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
                if archived_count > 0 && !facade.show_archived() {
                    StatusBanner {
                        message: facade.archived_entries_message(),
                        tone: "info".to_string()
                    }
                }
                div { "data-layout": "entry-controls-reveal",
                    button {
                        "data-layout": "entry-controls-toggle",
                        "data-action": "hide-entry-controls",
                        title: "收起筛选与组织",
                        "aria-label": "收起筛选与组织",
                        onclick: move |_| hide_controls_facade.set_controls_hidden(true),
                        span {
                            "data-slot": "entry-controls-toggle-chevron",
                            "data-direction": "up",
                            aria_hidden: "true"
                        }
                        span { "data-slot": "entry-controls-toggle-label", "收起筛选" }
                    }
                }
            }
        }
    }
}

pub(super) fn render_entry_pagination_controls(facade: &EntriesPageFacade) -> Element {
    if facade.total_pages() <= 1 {
        return rsx! {};
    }

    let previous_facade = facade.clone();
    let next_facade = facade.clone();

    rsx! {
        nav { "data-layout": "entry-pagination", "aria-label": "文章分页",
            div { class: "sr-only", "data-layout": "entry-pagination-summary",
                "第 {facade.page_start()}-{facade.page_end()} 篇，共 {facade.visible_entries_len()} 篇"
            }
            div { "data-layout": "entry-pagination-actions",
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-action": "entry-page-previous",
                    aria_label: "上一页", title: "上一页",
                    disabled: !facade.can_go_previous_page(),
                    onclick: move |_| previous_facade.go_to_previous_page(),
                    "‹"
                }
                span { "data-slot": "entry-pagination-status",
                    "{facade.current_page()} / {facade.total_pages()}"
                }
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-action": "entry-page-next",
                    aria_label: "下一页", title: "下一页",
                    disabled: !facade.can_go_next_page(),
                    onclick: move |_| next_facade.go_to_next_page(),
                    "›"
                }
            }
        }
    }
}
