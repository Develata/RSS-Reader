use std::sync::Arc;

mod browser_interactions;
mod bulk;
mod cards;
mod clock;
mod controls;
mod directory;
mod facade;
mod groups;
pub(crate) mod intent;
mod preferences_policy;
mod presenter;
mod pull_refresh;
pub(crate) mod reducer;
mod request_lifecycle;
mod session;
pub(crate) mod state;
mod write_feedback;
mod write_lifecycle;

use dioxus::prelude::*;

use self::cards::render_entry_card_at;
use self::clock::{current_time_utc, use_sampled_clock};
use self::controls::{render_entry_controls, render_entry_pagination_controls};
use self::{
    facade::EntriesPageFacade,
    presenter::{EntriesPagePresenter, EntriesPresenterInput},
    session::use_entries_page_session,
};
use crate::{
    app::AppNav,
    components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation_with_dismiss,
    router::AppRoute,
    ui::{AppShellState, use_reactive_side_effect, use_reactive_task, use_startup_route_bus},
};

#[component]
pub fn StartupPage() -> Element {
    let navigator = use_navigator();
    let status = use_signal(|| "正在准备你的阅读入口…".to_string());
    let status_tone = use_signal(|| "info".to_string());

    use_startup_route_bus(navigator, status, status_tone);

    rsx! {
        section { "data-page": "entries",
            AppNav {}
            h1 { "data-slot": "page-title", "文章" }
            StatusBanner { message: status(), tone: status_tone(), announce: true }
        }
    }
}

#[component]
pub fn EntriesPage() -> Element {
    entries_page_content(None)
}

#[component]
pub fn FeedEntriesPage(feed_id: i64) -> Element {
    entries_page_content(Some(feed_id))
}

fn entries_page_content(feed_id: Option<i64>) -> Element {
    let ui = use_context::<AppShellState>();
    let facade = use_entries_page_workspace(feed_id, ui);
    let directory_state = use_signal(directory::DirectoryState::default);
    let mut directory_collapsed = use_signal(|| false);
    let mut mobile_directory_open = use_signal(|| false);
    let source_filter_open = use_signal(|| false);
    let exit_hint = use_mobile_back_navigation_with_dismiss(
        feed_id.map(|_| AppRoute::FeedsPage {}),
        move || {
            let mut source_filter_open = source_filter_open;
            let mut mobile_directory_open = mobile_directory_open;
            if *source_filter_open.peek() {
                source_filter_open.set(false);
                true
            } else if *mobile_directory_open.peek() {
                mobile_directory_open.set(false);
                true
            } else {
                false
            }
        },
    );
    let mut directory_was_open = use_signal(|| false);
    use_effect(move || {
        let open = mobile_directory_open();
        if open == *directory_was_open.peek() {
            return;
        }
        directory_was_open.set(open);
        let target = if open { "close-directory" } else { "open-directory" };
        // DOM adapter only moves focus; Rust continues to own open/close and directory state.
        let _ = document::eval(&format!(
            "document.querySelector('[data-action={target}]')?.focus({{preventScroll:true}})"
        ));
    });
    use_context_provider(|| directory::DirectoryStore(directory_state));
    let controls = render_entry_controls(
        &facade,
        source_filter_open,
        mobile_directory_open(),
        EventHandler::new(move |_| {
            directory_collapsed.set(false);
            mobile_directory_open.set(true);
        }),
    );
    let pagination = render_entry_pagination_controls(&facade);

    rsx! {
        section {
            "data-page": "entries",
            "data-entry-scope": if feed_id.is_some() { "feed" } else { "all" },
            "data-position-key": facade.position_key(),
            "data-position-page": "{facade.current_page()}",
            "data-position-ready": if facade.positions_ready() { "true" } else { "false" },
            "data-has-selection": facade.selected_count() > 0,
            "data-directory-collapsed": directory_collapsed(),
            "data-mobile-directory-open": mobile_directory_open(),
            directory::DirectoryBridge {
                model: facade.directory_model(),
                context_key: facade.position_key(),
                session: facade.session(),
            }
            AppNav {}
            if feed_id.is_none() { pull_refresh::PullRefresh {} }
            div { "data-layout": "entries-layout",
                div { "data-layout": "entries-main",
                    { controls }
                    if feed_id.is_some() {
                        div { "data-layout": "entries-page-backlink",
                            Link {
                                class: "button",
                                "data-variant": "secondary",
                                "data-nav": "entries",
                                to: AppRoute::EntriesPage {},
                                "返回全部文章"
                            }
                        }
                    }
                    StatusBanner {
                        message: facade.status_message().to_string(),
                        tone: facade.status_tone().to_string(),
                        announce: true,
                        visually_hidden: facade.entries_loaded() && facade.status_tone() == "info",
                    }
                    if facade.entries_is_empty() {
                        if facade.entries_loaded() {
                            div { "data-layout": "entries-page-state", "data-state": "empty",
                                StatusBanner {
                                    message: facade.empty_entries_message(),
                                    tone: "info".to_string()
                                }
                            }
                        }
                    } else if facade.visible_entries_is_empty() {
                        div { "data-layout": "entries-page-state", "data-state": "archived",
                            StatusBanner {
                                message: facade.archived_entries_state_message().to_string(),
                                tone: "info".to_string()
                            }
                        }
                    } else {
                        div {
                            "data-layout": "entry-groups",
                            "data-state": "populated",
                            "data-grouping-mode": if facade.grouping_mode() == state::EntryGroupingMode::Time { "time" } else { "source" },
                            if facade.grouping_mode() == state::EntryGroupingMode::Time {
                                for month in facade.time_grouped_entries() {
                                    section { key: "{month.anchor_id}", id: "{month.anchor_id}", "data-layout": "entry-group", "data-grouping-mode": "time", "data-group-level": "month",
                                        div { "data-layout": "entry-group-header", "data-group-level": "primary",
                                            h2 { "data-slot": "entry-group-title", "{month.title}" }
                                            p { "data-slot": "entry-group-meta", "{month.subtitle}" }
                                        }
                                        for date_group in &month.dates {
                                            section { key: "{date_group.anchor_id}", id: "{date_group.anchor_id}", "data-layout": "entry-date-group", "data-grouping-mode": "time",
                                                "data-entry-scroll-anchor": "{date_group.anchor_id}",
                                                "data-entry-scroll-group-anchor": "{month.anchor_id}",
                                                div { "data-layout": "entry-group-header", "data-group-level": "date",
                                                    h3 { "data-slot": "entry-group-title", "{date_group.title}" }
                                                    p { "data-slot": "entry-group-meta", "{date_group.subtitle}" }
                                                }
                                                for source in &date_group.sources {
                                                    section { key: "{source.anchor_id}", id: "{source.anchor_id}", "data-layout": "entry-source-group", "data-grouping-mode": "time",
                                                        div { "data-layout": "entry-group-header", "data-group-level": "source",
                                                            h4 { "data-slot": "entry-group-title", "{source.title}" }
                                                            p { "data-slot": "entry-group-meta", "{source.subtitle}" }
                                                        }
                                                        ul { "data-layout": "entry-list", "data-state": "populated",
                                                            for (position , card) in source.entry_cards.iter().copied().enumerate() {
                                                                { render_entry_card_at(&facade, card, position, source.entry_cards.len()) }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                for group in facade.source_grouped_entries() {
                                    section { key: "{group.anchor_id}", id: "{group.anchor_id}", "data-layout": "entry-group", "data-grouping-mode": "source", "data-group-level": "source",
                                        div { "data-layout": "entry-group-header", "data-group-level": "primary",
                                            h2 { "data-slot": "entry-group-title", "{group.title}" }
                                            p { "data-slot": "entry-group-meta", "{group.subtitle}" }
                                        }
                                        for month in &group.months {
                                            section { key: "{month.anchor_id}", id: "{month.anchor_id}", "data-layout": "entry-date-group", "data-grouping-mode": "source",
                                                "data-entry-scroll-anchor": "{month.anchor_id}",
                                                "data-entry-scroll-group-anchor": "{group.anchor_id}",
                                                div { "data-layout": "entry-group-header", "data-group-level": "date",
                                                    h3 { "data-slot": "entry-group-title", "{month.title}" }
                                                    p { "data-slot": "entry-group-meta", "{month.subtitle}" }
                                                }
                                                ul { "data-layout": "entry-list", "data-state": "populated",
                                                    for (position , card) in month.entry_cards.iter().copied().enumerate() {
                                                        { render_entry_card_at(&facade, card, position, month.entry_cards.len()) }
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
                if !facade.group_nav_items().is_empty() {
                    if mobile_directory_open() {
                        button { class: "directory-backdrop", aria_label: "关闭目录", onclick: move |_| mobile_directory_open.set(false) }
                    }
                    directory::DirectoryRail { model: facade.directory_model(), grouping_mode: facade.grouping_mode(),
                        on_close: move |_| { directory_collapsed.set(true); mobile_directory_open.set(false); } }
                }
            }
        }
        { pagination }
        // Android 首页按一次返回后的提示（use_mobile_back_navigation）；与翻页条一样放在页面
        // 面板之外。
        if exit_hint() {
            p { "data-layout": "exit-hint", role: "status", "再按一次返回键退出" }
        }
    }
}

fn use_entries_page_workspace(feed_id: Option<i64>, ui: AppShellState) -> EntriesPageFacade {
    let session = use_entries_page_session(feed_id);
    let state_snapshot = Arc::new(session.snapshot());
    // memo 链：先把状态收窄成 presenter 真正依赖的投影，再由它推导 presenter。
    //
    // 信号订阅粒度是「整个信号」，所以第一个 memo 每次状态变化都会重算——但它很便宜
    // （只做 Arc 指针拷贝与少量标量）。关键在于它的值没变时，第二个 memo 不会重算，
    // 于是 `SetStatus`（每次切换已读/收藏都会发一条）、`ToggleSelected` 这类与分组
    // 无关的 intent 不再重建两棵分组树。
    let presenter_input = use_memo(move || {
        session.with_state(|state| EntriesPresenterInput::from_state(state, feed_id))
    });
    let presenter = use_memo(move || {
        // `presenter_input()` 会克隆整份投影；`with` 直接借用（`from_input` 是纯函数）。
        presenter_input.with(|input| Arc::new(EntriesPagePresenter::from_input(input)))
    });
    // The page's read-only refresh follows the shell's (C4b item 2).
    let library_revision = ui.library_revision();
    let entry_search = ui.entry_search();
    let query_search = (!entry_search.trim().is_empty()).then_some(entry_search);
    let preferences_loaded = state_snapshot.preferences_load.is_loaded();
    let entries_ready = state_snapshot.preferences_load.can_load_entries();
    let reload_revision = state_snapshot.reload_revision;
    let requery_revision = state_snapshot.requery_revision;
    let workspace = state_snapshot.workspace();
    // 归档分界的“现在”只在挂载、手动刷新完成与查询条件变化时采样；状态提示、查询结果这类
    // 普通重渲染不读时钟，否则 `EntryQuery` 每跨一秒就变化并重发查询（审计 A4）。
    let query_now = use_sampled_clock(
        state_snapshot.query_clock_trigger(feed_id, query_search.clone(), library_revision),
        current_time_utc,
    );
    let facade =
        EntriesPageFacade::new(ui, session, Arc::clone(&state_snapshot), presenter.cloned());
    let entry_query = state_snapshot.entry_query(feed_id, query_search.clone(), query_now);

    use_reactive_side_effect(
        (
            feed_id,
            query_search.clone(),
            workspace.show_archived,
            workspace.read_filter,
            workspace.starred_filter,
            workspace.selected_feed_urls.clone(),
        ),
        move |_| {
            session.dispatch(intent::EntriesPageIntent::CancelBulk);
        },
    );
    let mut previous_search = use_signal(|| (feed_id, query_search.clone()));
    use_reactive_side_effect((feed_id, query_search.clone()), move |next| {
        // 首次挂载不是筛选切换；异步加载可能先完成，不能随后把恢复页码重置为 1。
        if *previous_search.peek() != next {
            previous_search.set(next);
            session.dispatch(intent::EntriesPageIntent::ClearSelection);
            session.dispatch(intent::EntriesPageIntent::SetCurrentPage(state::FIRST_PAGE_NUMBER));
        }
    });

    use_reactive_task((feed_id, library_revision), move |_| session.bootstrap_or_reread());
    // Once per opened subscription, not per refresh.
    use_reactive_task(feed_id, move |feed_id| {
        if let Some(feed_id) = feed_id {
            session.remember_opened_feed(feed_id);
        }
    });

    use_reactive_task(
        (
            feed_id,
            entries_ready,
            entry_query.clone(),
            library_revision,
            reload_revision,
            requery_revision,
        ),
        move |(_, entries_ready, entry_query, _, _, _)| {
            if entries_ready {
                session.load_entries_query(entry_query);
            }
        },
    );

    use_reactive_side_effect((preferences_loaded, workspace), move |(loaded, workspace)| {
        session.save_browsing_preferences(loaded, workspace);
    });

    facade
}
