use dioxus::prelude::*;
use rssr_domain::{ReadFilter, StarredFilter};

#[component]
pub fn EntryFilters(
    search: String,
    read_filter: ReadFilter,
    starred_filter: StarredFilter,
    available_sources: Vec<(i64, String, String)>,
    selected_feed_urls: Vec<String>,
    on_search: EventHandler<String>,
    on_change_read_filter: EventHandler<ReadFilter>,
    on_change_starred_filter: EventHandler<StarredFilter>,
    on_change_selected_feed_urls: EventHandler<Vec<String>>,
) -> Element {
    rsx! {
        div { class: "entry-filters", "data-layout": "entry-filters",
            label {
                class: "sr-only",
                r#for: "entry-search-title",
                "按标题搜索"
            }
            input {
                id: "entry-search-title",
                name: "search_title",
                class: "text-input",
                "data-field": "search-title",
                value: "{search}",
                placeholder: "按标题搜索",
                oninput: move |event| on_search.call(event.value())
            }
            label { class: "entry-filters__toggle", "data-layout": "entry-filters-toggle",
                input {
                    name: "filter_unread",
                    r#type: "checkbox",
                    "data-field": "read-filter-unread",
                    checked: matches!(read_filter, ReadFilter::UnreadOnly),
                    onchange: move |event| {
                        on_change_read_filter.call(if event.checked() {
                            ReadFilter::UnreadOnly
                        } else {
                            ReadFilter::All
                        })
                    }
                }
                span { "仅未读" }
            }
            label { class: "entry-filters__toggle", "data-layout": "entry-filters-toggle",
                input {
                    name: "filter_read",
                    r#type: "checkbox",
                    "data-field": "read-filter-read",
                    checked: matches!(read_filter, ReadFilter::ReadOnly),
                    onchange: move |event| {
                        on_change_read_filter.call(if event.checked() {
                            ReadFilter::ReadOnly
                        } else {
                            ReadFilter::All
                        })
                    }
                }
                span { "仅已读" }
            }
            label { class: "entry-filters__toggle", "data-layout": "entry-filters-toggle",
                input {
                    name: "filter_starred",
                    r#type: "checkbox",
                    "data-field": "starred-filter-starred",
                    checked: matches!(starred_filter, StarredFilter::StarredOnly),
                    onchange: move |event| {
                        on_change_starred_filter.call(if event.checked() {
                            StarredFilter::StarredOnly
                        } else {
                            StarredFilter::All
                        })
                    }
                }
                span { "仅收藏" }
            }
            label { class: "entry-filters__toggle", "data-layout": "entry-filters-toggle",
                input {
                    name: "filter_unstarred",
                    r#type: "checkbox",
                    "data-field": "starred-filter-unstarred",
                    checked: matches!(starred_filter, StarredFilter::UnstarredOnly),
                    onchange: move |event| {
                        on_change_starred_filter.call(if event.checked() {
                            StarredFilter::UnstarredOnly
                        } else {
                            StarredFilter::All
                        })
                    }
                }
                span { "仅未收藏" }
            }
            if !available_sources.is_empty() {
                div { class: "entry-filters__sources", "data-layout": "entry-filters-sources",
                    p { class: "entry-filters__sources-label", "data-slot": "entry-filters-sources-label", "按来源筛选" }
                    div { class: "entry-filters__source-grid", "data-layout": "entry-filters-source-grid",
                        for (_feed_id, title, url) in available_sources {
                            {
                                let is_selected = selected_feed_urls.contains(&url);
                                let next_selected_feed_urls = if is_selected {
                                    selected_feed_urls
                                        .iter()
                                        .filter(|current| **current != url)
                                        .cloned()
                                        .collect::<Vec<_>>()
                                } else {
                                    let mut urls = selected_feed_urls.clone();
                                    urls.push(url.clone());
                                    urls.sort();
                                    urls.dedup();
                                    urls
                                };
                                rsx! {
                                    label {
                                        class: "entry-filters__source-chip",
                                        "data-layout": "entry-filters-source-chip",
                                        "data-state": if is_selected { "selected" } else { "unselected" },
                                        input {
                                            class: "sr-only",
                                            r#type: "checkbox",
                                            "data-field": "entry-source-filter",
                                            checked: is_selected,
                                            onchange: move |_| on_change_selected_feed_urls.call(next_selected_feed_urls.clone())
                                        }
                                        span { "{title}" }
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
