mod bindings;
mod cards;
mod controls;
mod effect;
mod groups;
mod intent;
mod presenter;
mod queries;
mod reducer;
mod runtime;
mod session;
mod state;

use dioxus::prelude::*;
use rssr_domain::StartupView;
use time::OffsetDateTime;

use self::cards::render_entry_card;
use self::controls::{
    EntryControlsProps, initial_entry_controls_hidden, render_entry_controls,
    render_entry_directory,
};
use self::{session::EntriesPageSession, state::EntriesPageState};
use crate::{
    app::{AppNav, AppUiState},
    bootstrap::AppServices,
    components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation,
    router::AppRoute,
    status::set_status_error,
};

#[component]
pub fn StartupPage() -> Element {
    let navigator = use_navigator();
    let status = use_signal(|| "正在准备你的阅读入口…".to_string());
    let status_tone = use_signal(|| "info".to_string());

    use_resource(move || async move {
        match AppServices::shared().await {
            Ok(services) => {
                let settings = match services.load_settings().await {
                    Ok(settings) => settings,
                    Err(err) => {
                        set_status_error(status, status_tone, format!("读取设置失败：{err}"));
                        navigator.replace(AppRoute::EntriesPage {});
                        return;
                    }
                };

                let target = match settings.startup_view {
                    StartupView::All => AppRoute::EntriesPage {},
                    StartupView::LastFeed => {
                        let last_feed_id = services.load_last_opened_feed_id().await.ok().flatten();
                        let feed_exists = match last_feed_id {
                            Some(feed_id) => services
                                .list_feeds()
                                .await
                                .map(|feeds| feeds.iter().any(|feed| feed.id == feed_id))
                                .unwrap_or(false),
                            None => false,
                        };

                        if let Some(feed_id) = last_feed_id.filter(|_| feed_exists) {
                            AppRoute::FeedEntriesPage { feed_id }
                        } else {
                            AppRoute::EntriesPage {}
                        }
                    }
                };

                navigator.replace(target);
            }
            Err(err) => {
                set_status_error(status, status_tone, format!("初始化应用失败：{err}"));
                navigator.replace(AppRoute::EntriesPage {});
            }
        }
    });

    rsx! {
        section { class: "page page-entries", "data-page": "entries",
            AppNav {}
            h2 { "文章" }
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

    let ui = use_context::<AppUiState>();
    let (session, state_snapshot, presenter) = use_entries_page_workspace(feed_id, ui);

    rsx! {
        section {
            class: "page page-entries",
            "data-page": "entries",
            "data-entry-scope": if feed_id.is_some() { "feed" } else { "all" },
            AppNav {}
            div { class: "entries-layout",
                div { class: "entries-main",
                    div { class: "reading-header",
                        div { class: "reading-header__row",
                            h2 { "{entries_page_title(feed_id)}" }
                        }
                    }
                    if feed_id.is_some() {
                        div { class: "entries-page__backlink",
                            Link {
                                class: "button secondary",
                                "data-nav": "entries",
                                to: AppRoute::EntriesPage {},
                                "返回全部文章"
                            }
                        }
                    }
                    { render_entry_controls(EntryControlsProps {
                        ui,
                        session,
                        visible_entries_len: presenter.visible_entries.len(),
                        archived_count: presenter.archived_count,
                        source_filter_options: &presenter.source_filter_options,
                        group_nav_items: &presenter.group_nav_items,
                    }) }
                    if state_snapshot.entries.is_empty() {
                        div { class: "entries-page__state", "data-state": "empty",
                            StatusBanner {
                                message: empty_entries_message(feed_id),
                                tone: "info".to_string()
                            }
                        }
                    } else if presenter.visible_entries.is_empty() {
                        div { class: "entries-page__state", "data-state": "archived",
                            StatusBanner {
                                message: "当前结果中的文章都已被自动归档，勾选“显示已归档文章”即可查看。".to_string(),
                                tone: "info".to_string()
                            }
                        }
                    } else {
                        div { class: "entry-groups",
                            if state_snapshot.grouping_mode == state::EntryGroupingMode::Time {
                                for month in presenter.time_grouped_entries {
                                    section { class: "entry-group entry-group--time", key: "{month.anchor_id}", id: "{month.anchor_id}",
                                        div { class: "entry-group__header",
                                            h3 { class: "entry-group__title", "{month.title}" }
                                            p { class: "entry-group__meta", "{month.subtitle}" }
                                        }
                                        for date_group in month.dates {
                                            section { class: "entry-date-group", key: "{date_group.anchor_id}", id: "{date_group.anchor_id}",
                                                div { class: "entry-date-group__header",
                                                    h4 { class: "entry-date-group__title", "{date_group.title}" }
                                                    p { class: "entry-date-group__meta", "{date_group.subtitle}" }
                                                }
                                                for source in date_group.sources {
                                                    section { class: "entry-source-group", key: "{source.anchor_id}", id: "{source.anchor_id}",
                                                        div { class: "entry-source-group__header",
                                                            h5 { class: "entry-source-group__title", "{source.title}" }
                                                            p { class: "entry-source-group__meta", "{source.subtitle}" }
                                                        }
                                                        ul { class: "entry-list entry-list--grouped",
                                                            for entry in source.entries {
                                                                { render_entry_card(entry, session) }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                for group in presenter.source_grouped_entries {
                                    section { class: "entry-group", key: "{group.title}", id: "{group.anchor_id}",
                                        div { class: "entry-group__header",
                                            h3 { class: "entry-group__title", "{group.title}" }
                                            p { class: "entry-group__meta", "{group.subtitle}" }
                                        }
                                        for month in group.months {
                                            section { class: "entry-date-group", key: "{month.anchor_id}", id: "{month.anchor_id}",
                                                div { class: "entry-date-group__header",
                                                    h4 { class: "entry-date-group__title", "{month.title}" }
                                                    p { class: "entry-date-group__meta", "{month.subtitle}" }
                                                }
                                                ul { class: "entry-list entry-list--grouped",
                                                    for entry in month.entries {
                                                        { render_entry_card(entry, session) }
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
                if !presenter.group_nav_items.is_empty() {
                    { render_entry_directory(
                        state_snapshot.grouping_mode,
                        &presenter.directory_months,
                        &presenter.directory_sources,
                        session,
                    ) }
                }
            }
        }
    }
}

fn use_entries_page_workspace(
    feed_id: Option<i64>,
    ui: AppUiState,
) -> (EntriesPageSession, EntriesPageState, presenter::EntriesPagePresenter) {
    let state = use_signal(|| EntriesPageState::new(initial_entry_controls_hidden()));
    let session = EntriesPageSession::new(feed_id, state);
    let reload_version = session.reload_tick();
    let query_search = (!(ui.entry_search)().trim().is_empty()).then(|| (ui.entry_search)());

    use_resource(move || async move {
        session.remember_last_opened_feed();
    });

    use_resource(move || async move {
        session.load_preferences();
    });

    use_resource(use_reactive!(|(reload_version)| async move {
        let _ = reload_version;
        session.load_feeds();
    }));

    use_resource(use_reactive!(|(feed_id, reload_version, query_search)| async move {
        let _ = reload_version;
        let _ = feed_id;
        session.load_entries(query_search.clone());
    }));

    use_effect(move || {
        session.save_browsing_preferences();
    });

    let state_snapshot = session.snapshot();
    let presenter = session.presenter(current_time_utc());
    (session, state_snapshot, presenter)
}

#[cfg(target_arch = "wasm32")]
fn current_time_utc() -> OffsetDateTime {
    let millis = js_sys::Date::now();
    let seconds = (millis / 1_000.0).floor() as i64;
    let nanos = ((millis % 1_000.0) * 1_000_000.0).round() as i64;
    OffsetDateTime::from_unix_timestamp(seconds).expect("valid unix timestamp")
        + time::Duration::nanoseconds(nanos)
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn entries_page_title(feed_id: Option<i64>) -> &'static str {
    if feed_id.is_some() { "订阅文章" } else { "文章" }
}

fn empty_entries_message(feed_id: Option<i64>) -> String {
    if feed_id.is_some() {
        "这个订阅下还没有可显示的文章，先尝试刷新该 feed。".to_string()
    } else {
        "没有可显示的文章，先去订阅页添加并刷新 feed。".to_string()
    }
}
