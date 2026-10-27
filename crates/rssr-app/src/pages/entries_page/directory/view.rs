use super::{DirectoryModel, DirectoryState, DirectoryStore};
use crate::pages::entries_page::state::EntryGroupingMode;
use dioxus::prelude::*;

#[component]
pub(crate) fn DirectoryRail(
    model: DirectoryModel,
    grouping_mode: EntryGroupingMode,
    on_close: EventHandler,
) -> Element {
    let DirectoryStore(state) = use_context();
    use_context_provider(super::virtual_articles::Cache::default);
    let state = state.read();
    let epoch = state.epoch();
    let revision = state.revision();
    rsx! {
        aside { id: "reading-directory", "data-layout": "entry-directory-rail", "data-directory-consumer": "rail",
            onkeydown: move |event| { if event.key() == Key::Escape { on_close.call(()); } },
            "data-directory-epoch": "{epoch}", "data-directory-revision": "{revision}",
            "data-directory-mode": if state.is_manual() { "manual" } else { "follow" },
            "data-directory-rendered": state.is_visible(),
            "data-directory-current-group": state.active_group().unwrap_or(""),
            "data-directory-current-item": state.active_item().unwrap_or(""),
            "data-directory-current-leaf": state.active_leaf().unwrap_or(""),
            div { class: "directory-heading-row",
                h2 { "data-slot": "entry-directory-heading", "目录" }
                button { class: "workspace-icon-button", title: "收起目录", aria_label: "收起目录", "data-action": "close-directory", onclick: move |_| on_close.call(()), crate::components::icon::Icon { name: "chevron-right" } }
            }
            if state.is_visible() {
              nav { "data-layout": "entry-directory-nav", "aria-label": "文章目录导航",
                if grouping_mode == EntryGroupingMode::Time {
                    for month in &model.directory_months {
                        div { "data-layout": "entry-directory-section", key: "{month.anchor_id}",
                            { toggle(&state, &month.anchor_id, &month.title, &month.subtitle, true) }
                            div { "data-layout": "entry-directory-children", "data-directory-section-body": "true",
                                "data-open-base": boolean(state.is_open(&month.anchor_id)), "data-open": boolean(state.is_open(&month.anchor_id)),
                              if state.is_open(&month.anchor_id) {
                                for date in &month.dates {
                                    div { key: "{date.anchor_id}", "data-layout": "entry-directory-date-branch",
                                        div { class: "directory-month-heading",
                                            { link(&state, &month.anchor_id, &date.anchor_id, &date.title, &date.subtitle, date.target_page, true) }
                                            { articles_toggle(&state, &date.anchor_id, &date.title) }
                                        }
                                        { articles(&state, &model, &date.anchor_id, &date.entry_cards) }
                                    }
                                }
                              }
                            }
                        }
                    }
                } else {
                    for source in &model.directory_sources {
                        div { "data-layout": "entry-directory-section", key: "{source.anchor_id}",
                            { toggle(&state, &source.anchor_id, &source.title, &source.subtitle, false) }
                            div { "data-layout": "entry-directory-grandchildren", "data-directory-section-body": "true",
                                "data-open-base": boolean(state.is_open(&source.anchor_id)), "data-open": boolean(state.is_open(&source.anchor_id)),
                              if state.is_open(&source.anchor_id) {
                                for month in &source.months {
                                    div { key: "{month.anchor_id}", "data-layout": "entry-directory-month-branch",
                                        div { class: "directory-month-heading",
                                            { link(&state, &source.anchor_id, &month.anchor_id, &month.title, &month.subtitle, month.target_page, false) }
                                            { articles_toggle(&state, &month.anchor_id, &month.title) }
                                        }
                                        { articles(&state, &model, &month.anchor_id, &month.entry_cards) }
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

fn articles(
    state: &DirectoryState,
    model: &DirectoryModel,
    anchor: &str,
    cards: &[super::super::groups::EntryCardRef],
) -> Element {
    if state.is_open(anchor) && cards.len() > 80 {
        return rsx! { super::virtual_articles::VirtualArticles {
            key: "{state.epoch()}:{anchor}", model: model.clone(), anchor: anchor.to_owned(),
            epoch: state.epoch(), active: state.active_leaf().map(str::to_owned),
        }};
    }
    rsx! {
        div { id: "directory-articles-{anchor}", "data-layout": "entry-directory-articles",
            // Both grouping modes share identity checks and global page mapping.
            // Only expanded paths mount article nodes; the projection keeps refs.
            if state.is_open(anchor) {
                for (card, entry) in cards.iter().filter_map(|card| model.1.get(card.index).filter(|entry| entry.id == card.id).map(|entry| (card, entry))) {
                    DirectoryArticle { key: "{entry.id}", entry: std::sync::Arc::clone(entry),
                        active: state.active_leaf() == Some(&super::super::groups::entry_anchor_id(entry.id)),
                        page: (card.index / model.page_size.max(1)) as u32 + 1 }
                }
            }
        }
    }
}

fn articles_toggle(state: &DirectoryState, anchor: &str, title: &str) -> Element {
    let open = state.is_open(anchor);
    let icon = if open { "chevron-down" } else { "chevron-right" };
    let verb = if open { "收起" } else { "展开" };
    rsx! {
        button {
            r#type: "button", class: "workspace-icon-button directory-month-toggle",
            "data-layout": "entry-directory-branch-toggle", "data-directory-anchor": anchor,
            "data-can-toggle": "true", "data-open": boolean(open),
            aria_expanded: boolean(open), aria_controls: "directory-articles-{anchor}",
            aria_label: "{verb}{title}的文章",
            crate::components::icon::Icon { name: icon }
        }
    }
}

#[component]
pub(super) fn DirectoryArticle(
    entry: std::sync::Arc<rssr_domain::EntrySummary>,
    active: bool,
    page: u32,
    #[props(default)] window_index: Option<usize>,
    #[props(default)] window_count: usize,
) -> Element {
    let anchor = super::super::groups::entry_anchor_id(entry.id);
    let title = &entry.title;
    rsx! {
        button { r#type: "button", "data-layout": "entry-directory-link",
            "data-directory-kind": "entry", "data-directory-level": "article",
            "data-directory-anchor": "{anchor}", "data-directory-page": "{page}",
            "data-virtual-index": window_index,
            "data-virtual-prev": window_index.and_then(|index| index.checked_sub(1)),
            "data-virtual-next": window_index.filter(|index| index + 1 < window_count).map(|index| index + 1),
            "data-active": boolean(active), aria_current: if active { "location" } else { "false" },
            aria_label: "定位文章：{title}", title,
            span { "data-slot": "entry-directory-title", "{title}" }
        }
    }
}

fn boolean(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn toggle(
    state: &DirectoryState,
    anchor: &str,
    title: &str,
    subtitle: &str,
    time: bool,
) -> Element {
    let open = state.is_open(anchor);
    rsx! {
        button { "data-layout": "entry-directory-toggle", r#type: "button",
            "data-directory-kind": "group", "data-directory-anchor": anchor,
            "data-directory-level": if time { "month" } else { "source" },
            "data-nav": if time { Some("entry-directory-month") } else { None },
            "data-action": if time { None } else if open { Some("collapse-directory-source") } else { Some("expand-directory-source") },
            "data-active": boolean(state.active_group() == Some(anchor)),
            "data-can-toggle": "true", "data-open-base": boolean(open), "data-open": boolean(open),
            aria_disabled: "false", aria_expanded: boolean(open),
            span { "data-slot": "entry-directory-title", "{title}" }
            span { "data-slot": "entry-directory-meta", title: subtitle, "{compact_count(subtitle)}" }
        }
    }
}

fn link(
    state: &DirectoryState,
    group: &str,
    anchor: &str,
    title: &str,
    subtitle: &str,
    page: u32,
    date: bool,
) -> Element {
    rsx! {
        button { key: "{anchor}", "data-layout": "entry-directory-link", r#type: "button",
            "data-directory-kind": "item", "data-directory-group-anchor": group,
            "data-directory-level": if date { "date" } else { "month" },
            "data-nav": if date { "entry-directory-date" } else { "entry-directory-month" },
            "data-directory-anchor": anchor, "data-directory-page": "{page}",
            "data-active": boolean(state.active_item() == Some(anchor)),
            span { "data-slot": "entry-directory-title", "{title}" }
            span { "data-slot": "entry-directory-meta", title: subtitle, "{compact_count(subtitle)}" }
        }
    }
}

fn compact_count(subtitle: &str) -> &str {
    subtitle.strip_suffix("篇文章").unwrap_or(subtitle).trim()
}
