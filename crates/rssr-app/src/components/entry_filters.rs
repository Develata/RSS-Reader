use super::{dialog::Dialog, icon::Icon};
use dioxus::prelude::*;
use rssr_domain::{ReadFilter, StarredFilter};
use std::collections::BTreeSet;

#[component]
pub fn EntryFilters(
    source_filter_open: Signal<bool>,
    read_filter: ReadFilter,
    starred_filter: StarredFilter,
    available_sources: Vec<(i64, String, String, u32)>,
    selected_feed_urls: Vec<String>,
    on_change_read_filter: EventHandler<ReadFilter>,
    on_change_starred_filter: EventHandler<StarredFilter>,
    on_change_selected_feed_urls: EventHandler<Vec<String>>,
) -> Element {
    rsx! {
        div { "data-layout": "entry-filters",
            if !available_sources.is_empty() {
                SourceChooser { open: source_filter_open, available_sources, selected_feed_urls, on_apply: on_change_selected_feed_urls }
            }
            div { class: "entry-status-filters", role: "group", aria_label: "文章状态筛选",
                FilterButton { icon: "check", label: "仅已读", active: read_filter == ReadFilter::ReadOnly,
                    onclick: move |_| on_change_read_filter.call(if read_filter == ReadFilter::ReadOnly { ReadFilter::All } else { ReadFilter::ReadOnly }) }
                FilterButton { icon: "circle", label: "仅未读", active: read_filter == ReadFilter::UnreadOnly,
                    onclick: move |_| on_change_read_filter.call(if read_filter == ReadFilter::UnreadOnly { ReadFilter::All } else { ReadFilter::UnreadOnly }) }
                FilterButton { icon: "star", label: "仅未收藏", active: starred_filter == StarredFilter::UnstarredOnly,
                    onclick: move |_| on_change_starred_filter.call(if starred_filter == StarredFilter::UnstarredOnly { StarredFilter::All } else { StarredFilter::UnstarredOnly }) }
                FilterButton { icon: "star", filled: true, label: "仅收藏", active: starred_filter == StarredFilter::StarredOnly,
                    onclick: move |_| on_change_starred_filter.call(if starred_filter == StarredFilter::StarredOnly { StarredFilter::All } else { StarredFilter::StarredOnly }) }
            }
        }
    }
}

#[component]
fn FilterButton(
    icon: &'static str,
    label: &'static str,
    active: bool,
    #[props(default)] filled: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! { button { r#type: "button", class: "workspace-icon-button filter-icon", title: label, aria_label: label, aria_pressed: active,
    onclick, Icon { name: icon, filled } } }
}

#[component]
fn SourceChooser(
    mut open: Signal<bool>,
    available_sources: Vec<(i64, String, String, u32)>,
    selected_feed_urls: Vec<String>,
    on_apply: EventHandler<Vec<String>>,
) -> Element {
    let mut draft = use_signal(BTreeSet::<String>::new);
    let mut search = use_signal(String::new);
    let all_urls =
        available_sources.iter().map(|(_, _, url, _)| url.clone()).collect::<BTreeSet<_>>();
    let initial = if selected_feed_urls.is_empty() {
        all_urls.clone()
    } else {
        selected_feed_urls.iter().filter(|url| all_urls.contains(*url)).cloned().collect()
    };
    let all_selected = *draft.read() == all_urls;
    let all_for_toggle = all_urls.clone();
    let apply_urls = all_urls.clone();
    let selected_count = draft.read().len();
    crate::ui::use_reactive_side_effect(
        (open(), all_selected, selected_count),
        move |(open, all, count)| {
            if open {
                // `indeterminate` is a DOM property, not an HTML attribute. Rust
                // owns the selection; this adapter only projects its mixed state.
                let mixed = !all && count > 0;
                let _ = document::eval(&format!(
                    "const checkbox = document.getElementById('source-filter-all'); if (checkbox) checkbox.indeterminate = {mixed};"
                ));
            }
        },
    );
    let query = search().trim().to_lowercase();
    rsx! {
        button { id: "source-filter-trigger", r#type: "button", class: "source-filter-trigger", aria_haspopup: "dialog", aria_expanded: open(),
            aria_label: if selected_feed_urls.is_empty() { "筛选来源：全部来源".to_string() } else { format!("筛选来源：已选 {} 个", selected_feed_urls.len()) },
            title: "筛选来源", "data-action": "open-source-filter",
            onclick: move |_| { draft.set(initial.clone()); search.set(String::new()); open.set(true); },
            span { if selected_feed_urls.is_empty() { "全部来源" } else { "…" } }
            Icon { name: "chevron-down" }
        }
        if open() {
            Dialog { id: "source-filter-dialog", label: "筛选来源", anchor: Some("source-filter-trigger"), on_close: move |_| open.set(false),
                div { class: "workspace-dialog-heading",
                    h2 { "筛选来源" }
                    button { class: "workspace-icon-button", aria_label: "关闭来源筛选", onclick: move |_| open.set(false), Icon { name: "close" } }
                }
                input { class: "text-input", r#type: "search", aria_label: "搜索来源", placeholder: "搜索来源", value: search(), oninput: move |event| search.set(event.value()) }
                label { class: "source-choice source-choice-all",
                    input { id: "source-filter-all", r#type: "checkbox", checked: all_selected,
                        aria_checked: if all_selected { "true" } else if selected_count > 0 { "mixed" } else { "false" },
                        aria_label: "全部来源", onchange: move |_| draft.set(if all_selected { BTreeSet::new() } else { all_for_toggle.clone() }) }
                    span { "全部来源" } small { "已选 {selected_count}" }
                }
                div { class: "source-choices",
                    for (id, title, url, _) in available_sources.iter().filter(|(_, title, url, _)| query.is_empty() || title.to_lowercase().contains(&query) || url.to_lowercase().contains(&query)) {
                        { let url = url.clone(); let checked = draft.read().contains(&url);
                          rsx! { label { key: "{id}", class: "source-choice",
                            input { r#type: "checkbox", checked, aria_label: "{title}", onchange: move |event| { if event.checked() { draft.write().insert(url.clone()); } else { draft.write().remove(&url); } } }
                            span { "{title}" }
                          } }
                        }
                    }
                }
                div { class: "workspace-dialog-actions",
                    button { class: "button", "data-variant": "secondary", onclick: move |_| open.set(false), "取消" }
                    button { class: "button", disabled: selected_count == 0, "data-action": "apply-source-filter", onclick: move |_| {
                        let value = if *draft.read() == apply_urls { Vec::new() } else { draft.read().iter().cloned().collect() };
                        on_apply.call(value); open.set(false);
                    }, "应用" }
                }
            }
        }
    }
}
