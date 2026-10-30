pub(crate) mod facade;
mod image_viewer;
mod image_viewer_state;
pub(crate) mod intent;
pub(crate) mod reducer;
mod result_lifecycle;
mod session;
pub(crate) mod state;
pub(crate) mod support;
pub(crate) mod webview;

use dioxus::prelude::*;

use crate::{
    app::AppNav,
    components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::{navigate_back, use_mobile_back_navigation_with_dismiss},
    hooks::use_reader_shortcuts::use_reader_shortcuts,
    router::AppRoute,
    ui::{AppShellState, use_reactive_side_effect, use_reactive_task},
};

pub(crate) use self::session::ReaderPageSession;

use self::facade::ReaderPageFacade;
use self::session::dispatch_reader_page_intent;

#[component]
pub fn ReaderPage(entry_id: i64) -> Element {
    let mut viewer = image_viewer::use_reader_image_viewer(entry_id);
    use_mobile_back_navigation_with_dismiss(Some(AppRoute::EntriesPage {}), move || {
        let mut viewer = viewer;
        viewer.write().close()
    });

    let navigator = use_navigator();
    let facade = use_reader_page_workspace(entry_id);
    let previous_facade = facade.clone();
    let read_facade = facade.clone();
    let starred_facade = facade.clone();
    let next_facade = facade.clone();

    rsx! {
        article {
            class: "reader-page",
            "data-page": "reader",
            "data-position-key": "reader:{entry_id}",
            "data-position-ready": if facade.positions_ready(entry_id) { "true" } else { "false" },
            "data-layout": "reader-page",
            "data-state": facade.load_state(entry_id),
            aria_busy: facade.load_state(entry_id) == "pending",
            AppNav { on_back: move |_| {
                if !viewer.write().close() { navigate_back(navigator, Some(AppRoute::EntriesPage {})); }
            } }
            // 快捷键监听整个页面（hooks/use_reader_shortcuts.rs），输入框里的按键不算；阅读区
            // 仍可用 Tab 进入，读屏在这里读出快捷键说明。
            div {
                "data-layout": "reader-shortcut-scope",
                role: "region",
                aria_label: "文章阅读区",
                aria_describedby: "reader-shortcuts-help",
                tabindex: 0,
            p { id: "reader-shortcuts-help", class: "sr-only",
                "阅读页可用 J、K 或左右方向键切换未读文章，M 切换已读，F 切换收藏；在输入框里不生效。"
            }
            header { class: "reader-header", "data-layout": "reader-header",
                h1 { class: "reader-title", "data-slot": "reader-title", "{facade.title()}" }
            }
            div { class: "reader-meta-block", "data-layout": "reader-meta-block",
                p { class: "reader-meta", "data-slot": "reader-meta", "来源：{facade.source()}" }
                if let Some(author) = facade.author() {
                    p { class: "reader-meta", "data-slot": "reader-meta", "作者：{author}" }
                }
                p { class: "reader-meta", "data-slot": "reader-meta", "发布时间：{facade.published_at()}" }
                if let Some(url) = facade.original_url() {
                    a {
                        "data-action": "open-original",
                        href: "{url}",
                        target: crate::bootstrap::external_link::TARGET,
                        rel: "noopener noreferrer",
                        "打开原文"
                    }
                }
            }
            StatusBanner {
                message: facade.error().unwrap_or_else(|| facade.status_message()).to_string(),
                tone: (if facade.error().is_some() { facade.error_tone() } else { facade.status_tone() }).to_string(),
                announce: true,
                visually_hidden: !facade.status_shown(),
            }
            if facade.error().is_none() {
                div { class: "reader-body", "data-layout": "reader-body", "data-state": "{facade.body_state()}",
                    if let Some(html) = facade.body_html() {
                        div { class: "reader-html", "data-slot": "reader-body-html", dangerous_inner_html: "{html}" }
                    } else {
                        pre { "data-slot": "reader-body-text", "{facade.body_text()}" }
                    }
                }
                div { class: "reader-pagination reader-pagination--context inline-actions", "data-layout": "reader-pagination", "data-context": "feed",
                    if let Some(previous_feed_entry_id) = facade.navigation_state().previous_feed_entry_id {
                        button {
                            class: "button inline-actions__item",
                            "data-variant": "secondary",
                            "data-nav": "previous-feed-entry",
                            onclick: move |_| { navigator.push(AppRoute::ReaderPage { entry_id: previous_feed_entry_id }); },
                            "上一篇同订阅文章"
                        }
                    }
                    if let Some(next_feed_entry_id) = facade.navigation_state().next_feed_entry_id {
                        button {
                            class: "button inline-actions__item",
                            "data-variant": "secondary",
                            "data-nav": "next-feed-entry",
                            onclick: move |_| { navigator.push(AppRoute::ReaderPage { entry_id: next_feed_entry_id }); },
                            "下一篇同订阅文章"
                        }
                    }
                }
            }
            }
        }
        // 底栏固定在视口底部，位置不随文章长短变化；放在 article 之外：有的主题给阅读页加了
        // backdrop-filter，会让里面的 fixed 定位相对于阅读页而不是视口。
        if facade.error().is_none() {
            nav { class: "reader-bottom-bar", "data-layout": "reader-bottom-bar", "aria-label": "阅读快捷操作",
                button {
                    class: "reader-bottom-bar__button",
                    disabled: !facade.has_previous_entry_target(),
                    title: if facade.has_previous_entry_target() { "上一篇未读文章（K 或 ←）" } else { "此方向没有未读文章" },
                    "data-state": "{facade.previous_entry_state()}",
                    "data-nav": "previous-unread-entry",
                    onclick: move |_| {
                        if let Some(target) = previous_facade.previous_entry_target() {
                            navigator.push(AppRoute::ReaderPage { entry_id: target });
                        }
                    },
                    span { class: "reader-bottom-bar__icon", "data-slot": "reader-bottom-bar-icon", aria_hidden: "true", "‹" }
                    span { class: "reader-bottom-bar__label", "data-slot": "reader-bottom-bar-label",
                        "上一未读"
                        span { class: "reader-bottom-bar__shortcut", "data-slot": "reader-bottom-bar-shortcut", "（K）" }
                    }
                }
                button {
                    class: "reader-bottom-bar__button",
                    disabled: !facade.flags_known(entry_id),
                    "data-state": "{facade.read_state()}",
                    "data-action": "mark-read",
                    onclick: move |_| {
                        read_facade.toggle_read(false);
                    },
                    span { class: "reader-bottom-bar__icon", "data-slot": "reader-bottom-bar-icon", aria_hidden: "true", "{facade.read_toggle_icon()}" }
                    span { class: "reader-bottom-bar__label", "data-slot": "reader-bottom-bar-label",
                        "{facade.read_toggle_text()}"
                        span { class: "reader-bottom-bar__shortcut", "data-slot": "reader-bottom-bar-shortcut", "（M）" }
                    }
                }
                button {
                    class: "reader-bottom-bar__button",
                    disabled: !facade.flags_known(entry_id),
                    "data-state": "{facade.starred_state()}",
                    "data-action": "toggle-starred",
                    aria_pressed: facade.is_starred(),
                    onclick: move |_| {
                        starred_facade.toggle_starred(false);
                    },
                    span { class: "reader-bottom-bar__icon", "data-slot": "reader-bottom-bar-icon", aria_hidden: "true", "{facade.starred_toggle_icon()}" }
                    span { class: "reader-bottom-bar__label", "data-slot": "reader-bottom-bar-label",
                        "收藏"
                        span { class: "reader-bottom-bar__shortcut", "data-slot": "reader-bottom-bar-shortcut", "（F）" }
                    }
                }
                button {
                    class: "reader-bottom-bar__button",
                    disabled: !facade.has_next_entry_target(),
                    title: if facade.has_next_entry_target() { "下一篇未读文章（J 或 →）" } else { "此方向没有未读文章" },
                    "data-state": "{facade.next_entry_state()}",
                    "data-nav": "next-unread-entry",
                    onclick: move |_| {
                        if let Some(target) = next_facade.next_entry_target() {
                            navigator.push(AppRoute::ReaderPage { entry_id: target });
                        }
                    },
                    span { class: "reader-bottom-bar__icon", "data-slot": "reader-bottom-bar-icon", aria_hidden: "true", "›" }
                    span { class: "reader-bottom-bar__label", "data-slot": "reader-bottom-bar-label",
                        "下一未读"
                        span { class: "reader-bottom-bar__shortcut", "data-slot": "reader-bottom-bar-shortcut", "（J）" }
                    }
                }
            }
        }
        if let image_viewer_state::ImageViewerState::Open(image) = viewer() {
            image_viewer::ReaderImageViewer { image, on_close: move |_| { viewer.write().close(); } }
        }
    }
}

fn use_reader_page_workspace(entry_id: i64) -> ReaderPageFacade {
    let state = use_signal(state::ReaderPageState::new);
    let session = ReaderPageSession::new(entry_id, state);
    use_reader_shortcuts(session);
    use_reactive_task(entry_id, move |_| {
        session.load();
    });
    // The read-only refresh, on every bump after the page opened (C4b item 2).
    let library_revision = use_context::<AppShellState>().library_revision();
    let opened_at = use_hook(|| library_revision);
    use_reactive_task(library_revision, move |revision| {
        if revision != opened_at {
            session.reread();
        }
    });

    let snapshot = session.snapshot();
    let should_localize_assets = snapshot.body_html.is_some()
        && snapshot.error.is_none()
        && !snapshot.asset_localization_requested;

    use_reactive_side_effect(
        (entry_id, should_localize_assets),
        move |(_, should_localize_assets)| {
            if should_localize_assets {
                dispatch_reader_page_intent(
                    state,
                    intent::ReaderPageIntent::SetAssetLocalizationRequested,
                );
                session.localize_entry_assets();
            }
        },
    );

    ReaderPageFacade::new(session, snapshot)
}
