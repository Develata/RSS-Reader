use super::{facade::EntriesPageFacade, intent::EntriesPageIntent, state::EntryGroupingMode};
use crate::components::status_banner::StatusBanner;
use crate::components::{entry_filters::EntryFilters, icon::Icon};
use dioxus::prelude::*;

pub(super) fn render_entry_controls(
    facade: &EntriesPageFacade,
    source_filter_open: Signal<bool>,
    directory_open: bool,
    on_open_directory: EventHandler,
) -> Element {
    let session = facade.session();
    let read = facade.clone();
    let starred = facade.clone();
    let sources = facade.clone();
    let source_mode = facade.clone();
    let time_mode = facade.clone();
    let archive = facade.clone();
    let clear = facade.clone();
    let count_label = if facade.selected_count() > 0 {
        format!("已选 {}", facade.selected_count())
    } else {
        format!("{} 篇", facade.visible_entries_len())
    };
    rsx! {
        div { class: "entries-toolbar",
            div { class: "entries-toolbar-title",
                button { class: "entry-select-hit entry-select-all", role: "checkbox", aria_checked: facade.page_selection_state(), aria_label: "选择当前页", title: "选择当前页", disabled: facade.visible_entries_is_empty() || facade.bulk_busy(),
                    "data-action": "select-current-page", onclick: move |_| session.dispatch(EntriesPageIntent::TogglePageSelection),
                    span { class: "entry-select-box", "data-checked": facade.page_selection_state(), aria_hidden: "true" }
                }
                h1 { "data-slot": "page-title", "文章" }
                span { class: "entries-count", aria_live: "polite", title: "{count_label}", "{count_label}" }
            }
            EntryFilters {
                source_filter_open,
                read_filter: facade.read_filter(), starred_filter: facade.starred_filter(),
                available_sources: facade.source_filter_options().to_vec(), selected_feed_urls: facade.selected_feed_urls().to_vec(),
                on_change_read_filter: move |value| read.set_read_filter(value),
                on_change_starred_filter: move |value| starred.set_starred_filter(value),
                on_change_selected_feed_urls: move |value| sources.set_selected_feed_urls(value),
            }
            div { class: "entry-group-toggle", role: "group", aria_label: "文章分组方式",
                button { aria_pressed: facade.grouping_mode() == EntryGroupingMode::Source, "data-action": "group-by-source", onclick: move |_| source_mode.set_grouping_mode(EntryGroupingMode::Source), "来源" }
                button { aria_pressed: facade.grouping_mode() == EntryGroupingMode::Time, "data-action": "group-by-time", onclick: move |_| time_mode.set_grouping_mode(EntryGroupingMode::Time), "时间" }
            }
            div { class: "entries-toolbar-actions",
                { super::bulk::render_bulk_controls(facade) }
                details { class: "entries-view-options",
                    summary { class: "workspace-icon-button", title: "列表选项", aria_label: "列表选项", Icon { name: "more" } }
                    div { class: "entries-view-options-body",
                        label { input { r#type: "checkbox", checked: facade.show_archived(), onchange: move |event| archive.set_show_archived(event.checked()) } "显示已归档文章" }
                        button { class: "button", "data-variant": "secondary", "data-action": "clear-entry-filters", onclick: move |_| clear.clear_filters(), "清除筛选" }
                    }
                }
                button { class: "directory-open-button workspace-icon-button", "data-action": "open-directory", title: "打开目录", aria_label: "打开目录", aria_controls: "reading-directory", aria_expanded: directory_open, disabled: facade.group_nav_items().is_empty(),
                    onclick: move |_| on_open_directory.call(()), Icon { name: "directory" }
                }
            }
        }
        if facade.entries_truncated() {
            StatusBanner { message: facade.truncated_entries_message(), tone: "info".to_string() }
        }
        if facade.archived_entry_count() > 0 && !facade.show_archived() {
            p { class: "entry-archive-summary", "{facade.archived_entries_message()}" }
        }
        if let Some(summary) = facade.active_filter_summary() {
            p { class: "entry-filter-summary", "data-layout": "entry-filter-summary", span { "data-slot": "entry-filter-summary-text", "{summary}" } }
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
