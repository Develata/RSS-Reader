use dioxus::prelude::*;
use rssr_domain::{ReadFilter, StarredFilter};

#[component]
pub fn EntryFilters(
    search: String,
    read_filter: ReadFilter,
    starred_filter: StarredFilter,
    available_sources: Vec<(i64, String)>,
    selected_feed_ids: Vec<i64>,
    on_search: EventHandler<String>,
    on_change_read_filter: EventHandler<ReadFilter>,
    on_change_starred_filter: EventHandler<StarredFilter>,
    on_change_selected_feed_ids: EventHandler<Vec<i64>>,
) -> Element {
    rsx! {
        div { class: "entry-filters",
            label {
                class: "sr-only",
                r#for: "entry-search-title",
                "按标题搜索"
            }
            input {
                id: "entry-search-title",
                name: "search_title",
                class: "text-input",
                "data-action": "search-title",
                value: "{search}",
                placeholder: "按标题搜索",
                oninput: move |event| on_search.call(event.value())
            }
            label { class: "entry-filters__toggle",
                input {
                    name: "filter_unread",
                    r#type: "checkbox",
                    "data-action": "filter-unread",
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
            label { class: "entry-filters__toggle",
                input {
                    name: "filter_read",
                    r#type: "checkbox",
                    "data-action": "filter-read",
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
            label { class: "entry-filters__toggle",
                input {
                    name: "filter_starred",
                    r#type: "checkbox",
                    "data-action": "filter-starred",
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
            label { class: "entry-filters__toggle",
                input {
                    name: "filter_unstarred",
                    r#type: "checkbox",
                    "data-action": "filter-unstarred",
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
                div { class: "entry-filters__sources",
                    p { class: "entry-filters__sources-label", "按来源筛选" }
                    div { class: "entry-filters__source-grid",
                        for (feed_id, title) in available_sources {
                            {
                                let is_selected = selected_feed_ids.contains(&feed_id);
                                let next_selected_feed_ids = if is_selected {
                                    selected_feed_ids.iter().copied().filter(|id| *id != feed_id).collect::<Vec<_>>()
                                } else {
                                    let mut ids = selected_feed_ids.clone();
                                    ids.push(feed_id);
                                    ids.sort_unstable();
                                    ids.dedup();
                                    ids
                                };
                                rsx! {
                                    label {
                                        class: if is_selected {
                                            "entry-filters__source-chip is-selected"
                                        } else {
                                            "entry-filters__source-chip"
                                        },
                                        input {
                                            class: "sr-only",
                                            r#type: "checkbox",
                                            checked: is_selected,
                                            onchange: move |_| on_change_selected_feed_ids.call(next_selected_feed_ids.clone())
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
