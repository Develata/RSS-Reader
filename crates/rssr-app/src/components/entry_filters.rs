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
        div { "data-layout": "entry-filters",
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
            label { "data-layout": "entry-filters-toggle",
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
            label { "data-layout": "entry-filters-toggle",
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
            label { "data-layout": "entry-filters-toggle",
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
            label { "data-layout": "entry-filters-toggle",
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
                div { "data-layout": "entry-filters-sources",
                    p { "data-slot": "entry-filters-sources-label", "按来源筛选" }
                    div { "data-layout": "entry-filters-source-grid",
                        for (_feed_id, title, url) in available_sources {
                            {
                                let is_selected = selected_feed_urls.contains(&url);
                                // 只在真的被点击时才算出新的选中集合：此前对每个 chip 都提前
                                // 构造一份候选 Vec 并做 push + sort + dedup，而其中只有被点的
                                // 那一个会被用到。
                                // 注意仍然为每个 chip 克隆了一份当前选中集合（闭包要拥有它），
                                // 省掉的是那次多余的排序去重，不是这份克隆本身。
                                let current_selection = selected_feed_urls.clone();
                                rsx! {
                                    label {
                                        "data-layout": "entry-filters-source-chip",
                                        "data-state": if is_selected { "selected" } else { "unselected" },
                                        input {
                                            class: "sr-only",
                                            r#type: "checkbox",
                                            "data-field": "entry-source-filter",
                                            checked: is_selected,
                                            onchange: move |_| on_change_selected_feed_urls.call(toggle_source_selection(&current_selection, &url))
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

/// 在已选来源集合里切换一个 URL，返回排序去重后的新集合。
fn toggle_source_selection(selected: &[String], url: &str) -> Vec<String> {
    if selected.iter().any(|current| current == url) {
        return selected.iter().filter(|current| current.as_str() != url).cloned().collect();
    }

    let mut next = selected.to_vec();
    next.push(url.to_string());
    next.sort();
    next.dedup();
    next
}

#[cfg(test)]
mod tests {
    use super::toggle_source_selection;

    #[test]
    fn adds_then_removes_a_source_keeping_the_set_sorted_and_deduped() {
        let selected = vec!["https://b.example/feed".to_string()];

        let added = toggle_source_selection(&selected, "https://a.example/feed");
        assert_eq!(
            added,
            vec!["https://a.example/feed".to_string(), "https://b.example/feed".to_string(),]
        );

        let removed = toggle_source_selection(&added, "https://a.example/feed");
        assert_eq!(removed, selected);
    }

    #[test]
    fn toggling_an_already_selected_source_does_not_duplicate_it() {
        let selected = vec!["https://a.example/feed".to_string()];

        assert!(toggle_source_selection(&selected, "https://a.example/feed").is_empty());
    }
}
