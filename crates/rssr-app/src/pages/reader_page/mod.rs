mod facade;
pub(crate) mod intent;
mod reducer;
mod session;
pub(crate) mod state;
pub(crate) mod support;

use dioxus::prelude::*;

use crate::{
    app::AppNav, components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation,
    hooks::use_reader_shortcuts::use_reader_shortcuts, router::AppRoute, ui::use_reactive_task,
};

pub(crate) use self::session::ReaderPageSession;

use self::facade::ReaderPageFacade;

#[component]
pub fn ReaderPage(entry_id: i64) -> Element {
    use_mobile_back_navigation(Some(AppRoute::EntriesPage {}));

    let navigator = use_navigator();
    let facade = use_reader_page_workspace(entry_id);
    let shortcuts = facade.shortcuts();
    let previous_action_target = facade.previous_action_target();
    let next_action_target = facade.next_action_target();
    let previous_facade = facade.clone();
    let read_facade = facade.clone();
    let starred_facade = facade.clone();
    let next_facade = facade.clone();

    rsx! {
        article {
            class: "reader-page",
            "data-page": "reader",
            tabindex: 0,
            onkeydown: move |event| shortcuts.call(event),
            AppNav {}
            header { class: "reader-header",
                h2 { class: "reader-title", "{facade.title()}" }
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
                p { class: "reader-meta", "来源：{facade.source()}" }
                p { class: "reader-meta", "发布时间：{facade.published_at()}" }
            }
            if let Some(message) = facade.error() {
                StatusBanner { message: message.to_string(), tone: "error".to_string() }
            } else {
                if !facade.status().is_empty() {
                    StatusBanner { message: facade.status().to_string(), tone: facade.status_tone().to_string() }
                }
                div { class: "reader-body",
                    if let Some(html) = facade.body_html() {
                        div { class: "reader-html", dangerous_inner_html: "{html}" }
                    } else {
                        pre { "{facade.body_text()}" }
                    }
                }
                div { class: "reader-pagination reader-pagination--context inline-actions",
                    if let Some(previous_feed_entry_id) = facade.navigation_state().previous_feed_entry_id {
                        button {
                            class: "button secondary",
                            "data-nav": "previous-feed-entry",
                            onclick: move |_| { navigator.push(AppRoute::ReaderPage { entry_id: previous_feed_entry_id }); },
                            "上一篇同订阅文章"
                        }
                    }
                    if let Some(next_feed_entry_id) = facade.navigation_state().next_feed_entry_id {
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
                        class: if previous_action_target.is_some() {
                            "reader-bottom-bar__button"
                        } else {
                            "reader-bottom-bar__button is-disabled"
                        },
                        disabled: previous_action_target.is_none(),
                        "data-nav": "previous-unread-entry",
                        onclick: move |_| {
                            if let Some(target) = previous_facade.previous_action_target() {
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
                            read_facade.toggle_read(false);
                        },
                        span { class: "reader-bottom-bar__icon", if facade.is_read() { "○" } else { "✓" } }
                        span { class: "reader-bottom-bar__label", if facade.is_read() { "未读（M）" } else { "已读（M）" } }
                    }
                    button {
                        class: if facade.is_starred() {
                            "reader-bottom-bar__button is-active"
                        } else {
                            "reader-bottom-bar__button"
                        },
                        "data-action": "toggle-starred",
                        onclick: move |_| {
                            starred_facade.toggle_starred(false);
                        },
                        span { class: "reader-bottom-bar__icon", if facade.is_starred() { "★" } else { "☆" } }
                        span { class: "reader-bottom-bar__label", "收藏（F）" }
                    }
                    button {
                        class: if next_action_target.is_some() {
                            "reader-bottom-bar__button"
                        } else {
                            "reader-bottom-bar__button is-disabled"
                        },
                        disabled: next_action_target.is_none(),
                        "data-nav": "next-unread-entry",
                        onclick: move |_| {
                            if let Some(target) = next_facade.next_action_target() {
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

fn use_reader_page_workspace(entry_id: i64) -> ReaderPageFacade {
    let state = use_signal(state::ReaderPageState::new);
    let session = ReaderPageSession::new(entry_id, state);
    let shortcuts = use_reader_shortcuts(session);
    let reload_version = session.reload_tick();

    use_reactive_task((entry_id, reload_version), move |_| {
        session.load();
    });

    ReaderPageFacade::new(session, session.snapshot(), shortcuts)
}
