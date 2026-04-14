mod browser_interactions;
mod cards;
mod clock;
mod controls;
mod facade;
mod groups;
pub(crate) mod intent;
mod presenter;
mod reducer;
mod session;
mod state;

use dioxus::prelude::*;

use self::browser_interactions::initial_entry_controls_hidden;
use self::cards::render_entry_card;
use self::clock::current_time_utc;
use self::controls::{render_entry_controls, render_entry_directory};
use self::{facade::EntriesPageFacade, session::EntriesPageSession, state::EntriesPageState};
use crate::{
    app::AppNav,
    components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation,
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
            h2 { "data-slot": "page-title", "文章" }
            StatusBanner { message: status(), tone: status_tone() }
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
    use_mobile_back_navigation(feed_id.map(|_| AppRoute::FeedsPage {}));

    let ui = use_context::<AppShellState>();
    let facade = use_entries_page_workspace(feed_id, ui);
    let controls = render_entry_controls(&facade);
    let load_more_facade = facade.clone();

    rsx! {
        section {
            "data-page": "entries",
            "data-entry-scope": if feed_id.is_some() { "feed" } else { "all" },
            AppNav {}
            div { "data-layout": "entries-layout",
                div { "data-layout": "entries-main",
                    div { "data-layout": "page-header", "data-slot": "page-section-header", "data-section": "entries",
                        div { "data-slot": "page-section-row",
                            h2 { "data-slot": "page-title", "{entries_page_title(feed_id)}" }
                        }
                    }
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
                    { controls }
                    if facade.entries_is_empty() {
                        div { "data-layout": "entries-page-state", "data-state": "empty",
                            StatusBanner {
                                message: facade.empty_entries_message(),
                                tone: "info".to_string()
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
                                            h3 { "data-slot": "entry-group-title", "{month.title}" }
                                            p { "data-slot": "entry-group-meta", "{month.subtitle}" }
                                        }
                                        for date_group in &month.dates {
                                            section { key: "{date_group.anchor_id}", id: "{date_group.anchor_id}", "data-layout": "entry-date-group", "data-grouping-mode": "time",
                                                div { "data-layout": "entry-group-header", "data-group-level": "date",
                                                    h4 { "data-slot": "entry-group-title", "{date_group.title}" }
                                                    p { "data-slot": "entry-group-meta", "{date_group.subtitle}" }
                                                }
                                                for source in &date_group.sources {
                                                    section { key: "{source.anchor_id}", id: "{source.anchor_id}", "data-layout": "entry-source-group", "data-grouping-mode": "time",
                                                        div { "data-layout": "entry-group-header", "data-group-level": "source",
                                                            h5 { "data-slot": "entry-group-title", "{source.title}" }
                                                            p { "data-slot": "entry-group-meta", "{source.subtitle}" }
                                                        }
                                                        ul { "data-layout": "entry-list", "data-state": "populated",
                                                            for (index , entry) in source.entries.iter().enumerate() {
                                                                { render_entry_card(entry.clone(), facade.clone(), list_edge_state(index, source.entries.len())) }
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
                                    section { key: "{group.title}", id: "{group.anchor_id}", "data-layout": "entry-group", "data-grouping-mode": "source", "data-group-level": "source",
                                        div { "data-layout": "entry-group-header", "data-group-level": "primary",
                                            h3 { "data-slot": "entry-group-title", "{group.title}" }
                                            p { "data-slot": "entry-group-meta", "{group.subtitle}" }
                                        }
                                        for month in &group.months {
                                            section { key: "{month.anchor_id}", id: "{month.anchor_id}", "data-layout": "entry-date-group", "data-grouping-mode": "source",
                                                div { "data-layout": "entry-group-header", "data-group-level": "date",
                                                    h4 { "data-slot": "entry-group-title", "{month.title}" }
                                                    p { "data-slot": "entry-group-meta", "{month.subtitle}" }
                                                }
                                                ul { "data-layout": "entry-list", "data-state": "populated",
                                                    for (index , entry) in month.entries.iter().enumerate() {
                                                        { render_entry_card(entry.clone(), facade.clone(), list_edge_state(index, month.entries.len())) }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if facade.has_more_entries() {
                            div { "data-layout": "entries-page-load-more",
                                button {
                                    class: "button",
                                    "data-variant": "secondary",
                                    "data-action": "show-more-entries",
                                    onclick: move |_| load_more_facade.show_more_entries(),
                                    "继续加载更多文章（剩余 {facade.remaining_entries_count()} 篇）"
                                }
                            }
                        }
                    }
                }
                if !facade.group_nav_items().is_empty() {
                    { render_entry_directory(
                        &facade,
                        facade.grouping_mode(),
                        facade.directory_months(),
                        facade.directory_sources(),
                    ) }
                }
            }
        }
    }
}

fn use_entries_page_workspace(feed_id: Option<i64>, ui: AppShellState) -> EntriesPageFacade {
    let state = use_signal(|| EntriesPageState::new(initial_entry_controls_hidden()));
    let session = EntriesPageSession::new(feed_id, state);
    let state_snapshot = session.snapshot();
    let entry_search = ui.entry_search();
    let query_search = (!entry_search.trim().is_empty()).then_some(entry_search);
    let entry_query = state_snapshot.entry_query(feed_id, query_search.clone());
    let preferences_loaded = state_snapshot.preferences_loaded;
    let grouping_mode = state::grouping_mode_preference(state_snapshot.grouping_mode);
    let show_archived = state_snapshot.show_archived;
    let read_filter = state_snapshot.read_filter;
    let starred_filter = state_snapshot.starred_filter;
    let selected_feed_urls = state_snapshot.selected_feed_urls.clone();

    use_reactive_task(
        (feed_id, preferences_loaded),
        move |(_, preferences_loaded)| {
            session.bootstrap(!preferences_loaded, true);
        },
    );

    use_reactive_task(
        (feed_id, entry_query.clone()),
        move |(_, entry_query)| {
            session.load_entries_query(entry_query);
        },
    );

    use_reactive_side_effect(
        (
            preferences_loaded,
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        ),
        move |(
            preferences_loaded,
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        )| {
            session.save_browsing_preferences_with(
                preferences_loaded,
                grouping_mode,
                show_archived,
                read_filter,
                starred_filter,
                selected_feed_urls,
            );
        },
    );

    EntriesPageFacade::new(ui, session, state_snapshot, current_time_utc())
}

fn entries_page_title(feed_id: Option<i64>) -> &'static str {
    if feed_id.is_some() { "订阅文章" } else { "文章" }
}

fn list_edge_state(index: usize, len: usize) -> &'static str {
    match (index, len) {
        (_, 0) => "single",
        (0, 1) => "single",
        (0, _) => "start",
        (i, l) if i + 1 == l => "end",
        _ => "middle",
    }
}
