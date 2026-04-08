use dioxus::prelude::*;

pub(crate) use super::{
    reader_page_bindings::ReaderPageBindings, reader_page_effect::ReaderPageEffect,
    reader_page_runtime::execute_reader_page_effect, reader_page_state::ReaderPageState,
};
use crate::{
    app::AppNav, bootstrap::ReaderNavigation, components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation,
    hooks::use_reader_shortcuts::use_reader_shortcuts, router::AppRoute,
};

#[component]
pub fn ReaderPage(entry_id: i64) -> Element {
    use_mobile_back_navigation(Some(AppRoute::EntriesPage {}));

    let navigator = use_navigator();
    let state = use_signal(ReaderPageState::new);
    let bindings = ReaderPageBindings::new(state);
    let shortcuts = use_reader_shortcuts(entry_id, state, bindings);
    let snapshot = state();
    let reload_version = snapshot.reload_tick;

    use_resource(use_reactive!(|(entry_id, reload_version)| async move {
        let _ = reload_version;
        let outcome = execute_reader_page_effect(ReaderPageEffect::LoadEntry(entry_id)).await;
        bindings.apply_runtime_outcome(outcome);
    }));

    rsx! {
        article {
            class: "reader-page",
            "data-page": "reader",
            tabindex: 0,
            onkeydown: move |event| shortcuts.call(event),
            AppNav {}
            header { class: "reader-header",
                h2 { class: "reader-title", "{snapshot.title}" }
            }
            div { class: "reader-toolbar inline-actions",
                button {
                    class: "button secondary",
                    "data-nav": "back",
                    onclick: move |_| navigator.go_back(),
                    "返回上一页"
                }
            }
            div { class: "reader-meta-block",
                p { class: "reader-meta", "来源：{snapshot.source}" }
                p { class: "reader-meta", "发布时间：{snapshot.published_at}" }
            }
            if let Some(message) = snapshot.error.clone() {
                StatusBanner { message, tone: "error".to_string() }
            } else {
                if !snapshot.status.is_empty() {
                    StatusBanner { message: snapshot.status.clone(), tone: snapshot.status_tone.clone() }
                }
                div { class: "reader-body",
                    if let Some(html) = snapshot.body_html.clone() {
                        div { class: "reader-html", dangerous_inner_html: "{html}" }
                    } else {
                        pre { "{snapshot.body_text}" }
                    }
                }
                div { class: "reader-pagination reader-pagination--context inline-actions",
                    if let Some(previous_feed_entry_id) = snapshot.navigation_state.previous_feed_entry_id {
                        button {
                            class: "button secondary",
                            "data-nav": "previous-feed-entry",
                            onclick: move |_| { navigator.push(AppRoute::ReaderPage { entry_id: previous_feed_entry_id }); },
                            "上一篇同订阅文章"
                        }
                    }
                    if let Some(next_feed_entry_id) = snapshot.navigation_state.next_feed_entry_id {
                        button {
                            class: "button secondary",
                            "data-nav": "next-feed-entry",
                            onclick: move |_| { navigator.push(AppRoute::ReaderPage { entry_id: next_feed_entry_id }); },
                            "下一篇同订阅文章"
                        }
                    }
                }
                nav { class: "reader-bottom-bar", "aria-label": "阅读快捷操作",
                    button {
                        class: if previous_action_target(snapshot.navigation_state).is_some() {
                            "reader-bottom-bar__button"
                        } else {
                            "reader-bottom-bar__button is-disabled"
                        },
                        disabled: previous_action_target(snapshot.navigation_state).is_none(),
                        "data-nav": "previous-entry",
                        onclick: move |_| {
                            if let Some(target) = previous_action_target(state().navigation_state) {
                                navigator.push(AppRoute::ReaderPage { entry_id: target });
                            }
                        },
                        span { class: "reader-bottom-bar__icon", "‹" }
                        span { class: "reader-bottom-bar__label", "上一未读" }
                    }
                    button {
                        class: "reader-bottom-bar__button",
                        "data-action": "mark-read",
                        onclick: move |_| {
                            spawn(async move {
                                let outcome =
                                    execute_reader_page_effect(ReaderPageEffect::ToggleRead {
                                        entry_id,
                                        currently_read: state().is_read,
                                        via_shortcut: false,
                                    })
                                    .await;
                                bindings.apply_runtime_outcome(outcome);
                            });
                        },
                        span { class: "reader-bottom-bar__icon", if snapshot.is_read { "○" } else { "✓" } }
                        span { class: "reader-bottom-bar__label", if snapshot.is_read { "未读（M）" } else { "已读（M）" } }
                    }
                    button {
                        class: if snapshot.is_starred {
                            "reader-bottom-bar__button is-active"
                        } else {
                            "reader-bottom-bar__button"
                        },
                        "data-action": "toggle-starred",
                        onclick: move |_| {
                            spawn(async move {
                                let outcome =
                                    execute_reader_page_effect(ReaderPageEffect::ToggleStarred {
                                        entry_id,
                                        currently_starred: state().is_starred,
                                        via_shortcut: false,
                                    })
                                    .await;
                                bindings.apply_runtime_outcome(outcome);
                            });
                        },
                        span { class: "reader-bottom-bar__icon", if snapshot.is_starred { "★" } else { "☆" } }
                        span { class: "reader-bottom-bar__label", "收藏（F）" }
                    }
                    button {
                        class: if next_action_target(snapshot.navigation_state).is_some() {
                            "reader-bottom-bar__button"
                        } else {
                            "reader-bottom-bar__button is-disabled"
                        },
                        disabled: next_action_target(snapshot.navigation_state).is_none(),
                        "data-nav": "next-entry",
                        onclick: move |_| {
                            if let Some(target) = next_action_target(state().navigation_state) {
                                navigator.push(AppRoute::ReaderPage { entry_id: target });
                            }
                        },
                        span { class: "reader-bottom-bar__icon", "›" }
                        span { class: "reader-bottom-bar__label", "下一未读" }
                    }
                }
            }
        }
    }
}

fn previous_action_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.previous_unread_entry_id.or(navigation.previous_feed_entry_id)
}

fn next_action_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.next_unread_entry_id.or(navigation.next_feed_entry_id)
}
