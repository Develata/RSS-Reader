use super::facade::EntriesPageFacade;
use super::groups::{EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem};
use super::state::EntryGroupingMode;
use crate::components::{entry_filters::EntryFilters, status_banner::StatusBanner};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
struct DirectorySectionViewState {
    is_open_base: bool,
    is_open: bool,
    can_toggle: bool,
}

fn directory_section_view_state(
    default_open: bool,
    toggled: bool,
    is_active: bool,
) -> DirectorySectionViewState {
    let is_open_base = if default_open { !toggled } else { toggled };
    DirectorySectionViewState {
        is_open_base,
        is_open: is_active || is_open_base,
        can_toggle: !is_active,
    }
}

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
    let archived_count = facade.archived_entry_count();
    let source_filter_options = facade.source_filter_options();
    let group_nav_items: &[EntryGroupNavItem] = facade.group_nav_items();

    rsx! {
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
                    div { "data-layout": "entry-overview-metric",
                        span { "data-slot": "entry-overview-label", "当前结果" }
                        strong { "data-slot": "entry-overview-value", "{visible_entries_len}" }
                    }
                    div { "data-layout": "entry-overview-metric",
                        span { "data-slot": "entry-overview-label", "每页数量" }
                        strong { "data-slot": "entry-overview-value", "{facade.page_size()}" }
                    }
                    div { "data-layout": "entry-overview-metric",
                        span { "data-slot": "entry-overview-label", "归档文章" }
                        strong { "data-slot": "entry-overview-value", "{archived_count}" }
                    }
                    div { "data-layout": "entry-overview-metric", "data-tone": "summary",
                        span { "data-slot": "entry-overview-label", "当前组织" }
                        strong {
                            "data-slot": "entry-overview-value",
                            if facade.grouping_mode() == EntryGroupingMode::Time { "按时间" } else { "按来源" }
                        }
                    }
                }
                if !group_nav_items.is_empty() {
                    nav { "data-layout": "entry-top-directory", "aria-label": "文章目录",
                        for item in group_nav_items {
                            button {
                                "data-layout": "entry-top-directory-chip",
                                r#type: "button",
                                "data-directory-kind": "group",
                                "data-active": if item.is_active { "true" } else { "false" },
                                "data-directory-anchor": "{item.anchor_id}",
                                onclick: {
                                    let anchor_id = item.anchor_id.clone();
                                    let target_page = item.target_page;
                                    let facade = facade.clone();
                                    move |_| {
                                        facade.navigate_to_directory_target(target_page, anchor_id.clone())
                                    }
                                },
                                span { "data-slot": "entry-directory-title", "{item.title}" }
                                span { "data-slot": "entry-directory-meta", "{item.subtitle}" }
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
                if facade.has_status_message() {
                    StatusBanner {
                        message: facade.status_message().to_string(),
                        tone: facade.status_tone().to_string(),
                    }
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
    let toggled_directory_sections = facade.expanded_directory_sections();
    let default_expanded_directory_sections = facade.default_expanded_directory_sections();

    rsx! {
        aside { "data-layout": "entry-directory-rail",
            h3 { "data-slot": "entry-directory-heading", "目录" }
            if grouping_mode == EntryGroupingMode::Time {
                nav { "data-layout": "entry-directory-nav", "aria-label": "文章目录导航",
                    for month in directory_months {
                        {
                            let anchor_id = month.anchor_id.clone();
                            let view_state = directory_section_view_state(
                                default_expanded_directory_sections.contains(&anchor_id),
                                toggled_directory_sections.contains(&anchor_id),
                                month.is_active,
                            );
                            let toggle_anchor = anchor_id.clone();
                            let toggle_facade = facade.clone();
                            rsx! {
                                div { "data-layout": "entry-directory-section", key: "{month.anchor_id}",
                                    button {
                                        "data-layout": "entry-directory-toggle",
                                        "data-directory-kind": "group",
                                        "data-directory-level": "month",
                                        "data-nav": "entry-directory-month",
                                        "data-active": if month.is_active { "true" } else { "false" },
                                        "data-can-toggle": if view_state.can_toggle { "true" } else { "false" },
                                        "data-open-base": if view_state.is_open_base { "true" } else { "false" },
                                        "data-open": if view_state.is_open { "true" } else { "false" },
                                        "data-directory-anchor": "{month.anchor_id}",
                                        aria_disabled: if view_state.can_toggle { "false" } else { "true" },
                                        aria_expanded: if view_state.is_open { "true" } else { "false" },
                                        r#type: "button",
                                        onclick: move |_| {
                                            toggle_facade.toggle_directory_section(toggle_anchor.clone());
                                        },
                                        span { "data-slot": "entry-directory-title", "{month.title}" }
                                        span { "data-slot": "entry-directory-meta", "{month.subtitle}" }
                                    }
                                    div {
                                        "data-layout": "entry-directory-children",
                                        "data-directory-section-body": "true",
                                        "data-open-base": if view_state.is_open_base { "true" } else { "false" },
                                        "data-open": if view_state.is_open { "true" } else { "false" },
                                        for date in &month.dates {
                                            button {
                                                "data-layout": "entry-directory-link",
                                                "data-directory-kind": "item",
                                                "data-directory-group-anchor": "{month.anchor_id}",
                                                "data-directory-level": "date",
                                                "data-nav": "entry-directory-date",
                                                "data-active": if date.is_active { "true" } else { "false" },
                                                "data-directory-anchor": "{date.anchor_id}",
                                                r#type: "button",
                                                onclick: {
                                                    let anchor_id = date.anchor_id.clone();
                                                    let target_page = date.target_page;
                                                    let facade = facade.clone();
                                                    move |_| {
                                                        facade.navigate_to_directory_target(target_page, anchor_id.clone())
                                                    }
                                                },
                                                span { "data-slot": "entry-directory-title", "{date.title}" }
                                                span { "data-slot": "entry-directory-meta", "{date.subtitle}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                nav { "data-layout": "entry-directory-nav", "aria-label": "文章目录导航",
                    for source in directory_sources {
                        {
                            let anchor_id = source.anchor_id.clone();
                            let view_state = directory_section_view_state(
                                default_expanded_directory_sections.contains(&anchor_id),
                                toggled_directory_sections.contains(&anchor_id),
                                source.is_active,
                            );
                            let toggle_anchor = anchor_id.clone();
                            let toggle_facade = facade.clone();
                            rsx! {
                                div { "data-layout": "entry-directory-section", key: "{anchor_id}",
                                    button {
                                        "data-layout": "entry-directory-toggle",
                                        "data-directory-kind": "group",
                                        "data-active": if source.is_active { "true" } else { "false" },
                                        "data-can-toggle": if view_state.can_toggle { "true" } else { "false" },
                                        "data-open-base": if view_state.is_open_base { "true" } else { "false" },
                                        "data-open": if view_state.is_open { "true" } else { "false" },
                                        "data-directory-anchor": "{source.anchor_id}",
                                        aria_disabled: if view_state.can_toggle { "false" } else { "true" },
                                        aria_expanded: if view_state.is_open { "true" } else { "false" },
                                        "data-action": if view_state.is_open { "collapse-directory-source" } else { "expand-directory-source" },
                                        onclick: move |_| {
                                            toggle_facade.toggle_directory_section(toggle_anchor.clone());
                                        },
                                        span { "data-slot": "entry-directory-title", "{source.title}" }
                                        span { "data-slot": "entry-directory-meta", "{source.subtitle}" }
                                    }
                                    div {
                                        "data-layout": "entry-directory-grandchildren",
                                        "data-directory-section-body": "true",
                                        "data-open-base": if view_state.is_open_base { "true" } else { "false" },
                                        "data-open": if view_state.is_open { "true" } else { "false" },
                                        for month in &source.months {
                                            button {
                                                "data-layout": "entry-directory-link",
                                                "data-directory-kind": "item",
                                                "data-directory-group-anchor": "{source.anchor_id}",
                                                "data-directory-level": "month",
                                                "data-nav": "entry-directory-month",
                                                "data-active": if month.is_active { "true" } else { "false" },
                                                "data-directory-anchor": "{month.anchor_id}",
                                                r#type: "button",
                                                onclick: {
                                                    let anchor_id = month.anchor_id.clone();
                                                    let target_page = month.target_page;
                                                    let facade = facade.clone();
                                                    move |_| {
                                                        facade.navigate_to_directory_target(target_page, anchor_id.clone())
                                                    }
                                                },
                                                span { "data-slot": "entry-directory-title", "{month.title}" }
                                                span { "data-slot": "entry-directory-meta", "{month.subtitle}" }
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

#[cfg(test)]
mod tests {
    use super::directory_section_view_state;

    #[test]
    fn active_directory_section_stays_open_and_cannot_toggle() {
        let state = directory_section_view_state(true, true, true);
        assert!(!state.is_open_base);
        assert!(state.is_open);
        assert!(!state.can_toggle);
    }

    #[test]
    fn inactive_current_page_section_can_be_collapsed() {
        let state = directory_section_view_state(true, true, false);
        assert!(!state.is_open_base);
        assert!(!state.is_open);
        assert!(state.can_toggle);
    }

    #[test]
    fn off_page_section_can_be_manually_opened() {
        let state = directory_section_view_state(false, true, false);
        assert!(state.is_open_base);
        assert!(state.is_open);
        assert!(state.can_toggle);
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
            div { "data-layout": "entry-pagination-summary",
                "第 {facade.page_start()}-{facade.page_end()} 篇，共 {facade.visible_entries_len()} 篇"
            }
            div { "data-layout": "entry-pagination-actions",
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-action": "entry-page-previous",
                    disabled: !facade.can_go_previous_page(),
                    onclick: move |_| previous_facade.go_to_previous_page(),
                    "上一页"
                }
                span { "data-slot": "entry-pagination-status",
                    "第 {facade.current_page()} / {facade.total_pages()} 页"
                }
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-action": "entry-page-next",
                    disabled: !facade.can_go_next_page(),
                    onclick: move |_| next_facade.go_to_next_page(),
                    "下一页"
                }
            }
        }
    }
}
