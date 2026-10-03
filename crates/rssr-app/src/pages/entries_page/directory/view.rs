use super::{DirectoryModel, DirectoryState, DirectoryStore};
use crate::pages::entries_page::state::EntryGroupingMode;
use dioxus::prelude::*;

#[component]
pub(crate) fn DirectoryTop(model: DirectoryModel) -> Element {
    let DirectoryStore(state) = use_context();
    let state = state.read();
    rsx! {
        nav { "data-layout": "entry-top-directory", "aria-label": "文章目录",
            "data-directory-consumer": "top", "data-directory-epoch": "{state.epoch}",
            "data-directory-revision": "{state.revision}", "data-directory-mode": if state.manual { "manual" } else { "follow" },
            "data-directory-current-group": state.active().group.as_deref().unwrap_or(""),
            for item in &model.group_nav_items {
                button { key: "{item.anchor_id}", "data-layout": "entry-top-directory-chip", r#type: "button",
                    "data-directory-kind": "group", "data-directory-anchor": "{item.anchor_id}",
                    "data-directory-page": "{item.target_page}",
                    "data-active": if state.active().group.as_deref() == Some(&item.anchor_id) { "true" } else { "false" },
                    span { "data-slot": "entry-directory-title", "{item.title}" }
                    span { "data-slot": "entry-directory-meta", "{item.subtitle}" }
                }
            }
        }
    }
}

#[component]
pub(crate) fn DirectoryRail(model: DirectoryModel, grouping_mode: EntryGroupingMode) -> Element {
    let DirectoryStore(state) = use_context();
    let state = state.read();
    rsx! {
        aside { "data-layout": "entry-directory-rail", "data-directory-consumer": "rail",
            "data-directory-epoch": "{state.epoch}", "data-directory-revision": "{state.revision}",
            "data-directory-mode": if state.manual { "manual" } else { "follow" },
            "data-directory-current-group": state.active().group.as_deref().unwrap_or(""),
            h2 { "data-slot": "entry-directory-heading", "目录" }
            nav { "data-layout": "entry-directory-nav", "aria-label": "文章目录导航",
                if grouping_mode == EntryGroupingMode::Time {
                    for month in &model.directory_months {
                        div { "data-layout": "entry-directory-section", key: "{month.anchor_id}",
                            { toggle(&state, &month.anchor_id, &month.title, &month.subtitle, true) }
                            div { "data-layout": "entry-directory-children", "data-directory-section-body": "true",
                                "data-open-base": boolean(state.is_open(&month.anchor_id)), "data-open": boolean(state.is_open(&month.anchor_id)),
                                for date in &month.dates {
                                    { link(&state, &month.anchor_id, &date.anchor_id, &date.title, &date.subtitle, date.target_page, true) }
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
                                for month in &source.months {
                                    { link(&state, &source.anchor_id, &month.anchor_id, &month.title, &month.subtitle, month.target_page, false) }
                                }
                            }
                        }
                    }
                }
            }
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
            "data-active": boolean(state.active().group.as_deref() == Some(anchor)),
            "data-can-toggle": "true", "data-open-base": boolean(open), "data-open": boolean(open),
            aria_disabled: "false", aria_expanded: boolean(open),
            span { "data-slot": "entry-directory-title", "{title}" }
            span { "data-slot": "entry-directory-meta", "{subtitle}" }
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
            "data-active": boolean(state.active().item.as_deref() == Some(anchor)),
            span { "data-slot": "entry-directory-title", "{title}" }
            span { "data-slot": "entry-directory-meta", "{subtitle}" }
        }
    }
}
