use super::entries_page_groups::{EntryDirectoryMonth, EntryDirectorySource, EntryGroupNavItem};
use super::entries_page_intent::EntriesPageIntent;
use super::entries_page_reducer::dispatch_entries_page_intent;
use super::entries_page_state::{EntriesPageState, EntryGroupingMode};
use crate::{
    app::AppUiState,
    components::{entry_filters::EntryFilters, status_banner::StatusBanner},
};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub(super) struct EntryControlsProps<'a> {
    pub ui: AppUiState,
    pub state: Signal<EntriesPageState>,
    pub visible_entries_len: usize,
    pub archived_count: usize,
    pub source_filter_options: &'a [(i64, String, String)],
    pub group_nav_items: &'a [EntryGroupNavItem],
}

pub(super) fn render_entry_controls(props: EntryControlsProps<'_>) -> Element {
    let EntryControlsProps {
        mut ui,
        state,
        visible_entries_len,
        archived_count,
        source_filter_options,
        group_nav_items,
    } = props;
    let snapshot = state();

    rsx! {
        if snapshot.controls_hidden {
            div { class: "entry-controls-reveal",
                button {
                    class: "entry-controls-toggle entry-controls-toggle--flat",
                    "data-action": "show-entry-controls",
                    title: "显示筛选与组织",
                    "aria-label": "显示筛选与组织",
                    onclick: move |_| {
                        remember_entry_controls_hidden(false);
                        dispatch_entries_page_intent(state, EntriesPageIntent::SetControlsHidden(false));
                    },
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
                        "data-action": if snapshot.grouping_mode == EntryGroupingMode::Time { "group-by-time" } else { "group-by-source" },
                        value: match snapshot.grouping_mode {
                            EntryGroupingMode::Time => "time",
                            EntryGroupingMode::Source => "source",
                        },
                        onchange: move |event| {
                            dispatch_entries_page_intent(
                                state,
                                EntriesPageIntent::SetGroupingMode(match event.value().as_str() {
                                    "source" => EntryGroupingMode::Source,
                                    _ => EntryGroupingMode::Time,
                                }),
                            );
                        },
                        option { value: "time", "按时间" }
                        option { value: "source", "按来源" }
                    }
                    label { class: "entry-filters__toggle",
                        input {
                            r#type: "checkbox",
                            "data-action": "toggle-archived",
                            checked: snapshot.show_archived,
                            onchange: move |event| {
                                dispatch_entries_page_intent(
                                    state,
                                    EntriesPageIntent::SetShowArchived(event.checked()),
                                )
                            }
                        }
                        span { "显示已归档文章" }
                    }
                    p { class: "page-intro",
                        if snapshot.show_archived {
                            "当前同时显示归档文章。"
                        } else {
                            "默认隐藏超过 {snapshot.archive_after_months} 个月的归档文章。"
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
                            if snapshot.grouping_mode == EntryGroupingMode::Time { "按时间" } else { "按来源" }
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
                    search: (ui.entry_search)(),
                    read_filter: snapshot.read_filter,
                    starred_filter: snapshot.starred_filter,
                    available_sources: source_filter_options.to_vec(),
                    selected_feed_urls: snapshot.selected_feed_urls.clone(),
                    on_search: move |value| ui.entry_search.set(value),
                    on_change_read_filter: move |value| {
                        dispatch_entries_page_intent(state, EntriesPageIntent::SetReadFilter(value))
                    },
                    on_change_starred_filter: move |value| {
                        dispatch_entries_page_intent(
                            state,
                            EntriesPageIntent::SetStarredFilter(value),
                        )
                    },
                    on_change_selected_feed_urls: move |value| {
                        dispatch_entries_page_intent(
                            state,
                            EntriesPageIntent::SetSelectedFeedUrls(value),
                        )
                    },
                }
                StatusBanner { message: snapshot.status, tone: snapshot.status_tone }
                if archived_count > 0 && !snapshot.show_archived {
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
                        onclick: move |_| {
                            remember_entry_controls_hidden(true);
                            dispatch_entries_page_intent(state, EntriesPageIntent::SetControlsHidden(true));
                        },
                        span { class: "entry-controls-toggle__chevron entry-controls-toggle__chevron--up", aria_hidden: "true" }
                    }
                }
            }
        }
    }
}

pub(super) fn render_entry_directory(
    grouping_mode: EntryGroupingMode,
    directory_months: &[EntryDirectoryMonth],
    directory_sources: &[EntryDirectorySource],
    state: Signal<EntriesPageState>,
) -> Element {
    let expanded_directory_sources = state().expanded_directory_sources;

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
                            rsx! {
                                div { class: "entry-directory-rail__subsection", key: "{anchor_id}",
                                    button {
                                        class: "entry-directory-rail__toggle",
                                        aria_expanded: if is_open { "true" } else { "false" },
                                        "data-action": if is_open { "collapse-directory-source" } else { "expand-directory-source" },
                                        onclick: move |_| {
                                            dispatch_entries_page_intent(
                                                state,
                                                EntriesPageIntent::ToggleDirectorySource(
                                                    toggle_anchor.clone(),
                                                ),
                                            );
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
