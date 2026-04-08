use dioxus::prelude::*;
use rssr_domain::{
    EntryQuery, EntrySummary, FeedSummary, ReadFilter, StarredFilter, StartupView, UserSettings,
    is_entry_archived,
};
use std::collections::BTreeSet;
use time::OffsetDateTime;

use super::entries_page_cards::render_entry_card;
use super::entries_page_controls::{
    EntryControlsProps, EntryGroupingMode, entry_grouping_mode_from_preference,
    grouping_mode_preference, initial_entry_controls_hidden, map_selected_feed_urls_to_ids,
    render_entry_controls, render_entry_directory,
};
use super::entries_page_groups::{
    build_directory_months, build_directory_sources, build_group_nav_items, build_month_nav_items,
    group_anchor_id, group_entries_by_source_tree, group_entries_by_time_tree,
};
use crate::{
    app::{AppNav, AppUiState},
    bootstrap::AppServices,
    components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation,
    router::AppRoute,
    status::{set_status_error, set_status_info},
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
    let mut entries = use_signal(Vec::<EntrySummary>::new);
    let mut feeds = use_signal(Vec::<FeedSummary>::new);
    let mut read_filter = use_signal(ReadFilter::default);
    let mut starred_filter = use_signal(StarredFilter::default);
    let mut selected_feed_urls = use_signal(Vec::<String>::new);
    let mut show_archived = use_signal(|| UserSettings::default().show_archived_entries);
    let mut grouping_mode = use_signal(|| {
        entry_grouping_mode_from_preference(UserSettings::default().entry_grouping_mode)
    });
    let mut archive_after_months = use_signal(|| UserSettings::default().archive_after_months);
    let expanded_directory_sources = use_signal(BTreeSet::<String>::new);
    let controls_hidden = use_signal(initial_entry_controls_hidden);
    let reload_tick = use_signal(|| 0_u64);
    let status = use_signal(|| "正在加载文章列表…".to_string());
    let status_tone = use_signal(|| "info".to_string());
    let mut preferences_loaded = use_signal(|| false);

    use_resource(move || async move {
        if let Some(feed_id) = feed_id
            && let Ok(services) = AppServices::shared().await
        {
            let _ = services.remember_last_opened_feed_id(feed_id).await;
        }
    });

    use_resource(move || async move {
        match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(settings) => {
                    archive_after_months.set(settings.archive_after_months);
                    read_filter.set(settings.entry_read_filter);
                    starred_filter.set(settings.entry_starred_filter);
                    selected_feed_urls.set(settings.entry_filtered_feed_urls);
                    show_archived.set(settings.show_archived_entries);
                    grouping_mode
                        .set(entry_grouping_mode_from_preference(settings.entry_grouping_mode));
                    preferences_loaded.set(true);
                }
                Err(err) => set_status_error(status, status_tone, format!("读取设置失败：{err}")),
            },
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });

    use_resource(move || async move {
        let _ = reload_tick();
        match AppServices::shared().await {
            Ok(services) => match services.list_feeds().await {
                Ok(items) => feeds.set(items),
                Err(err) => set_status_error(status, status_tone, format!("读取订阅失败：{err}")),
            },
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });

    use_resource(move || async move {
        let _ = reload_tick();
        match AppServices::shared().await {
            Ok(services) => match services
                .list_entries(&EntryQuery {
                    feed_id,
                    read_filter: read_filter(),
                    starred_filter: starred_filter(),
                    feed_ids: if feed_id.is_some() {
                        Vec::new()
                    } else {
                        map_selected_feed_urls_to_ids(&feeds(), &selected_feed_urls())
                    },
                    search_title: (!(ui.entry_search)().trim().is_empty())
                        .then(|| (ui.entry_search)()),
                    limit: None,
                })
                .await
            {
                Ok(items) => {
                    set_status_info(status, status_tone, format!("共 {} 篇文章。", items.len()));
                    entries.set(items);
                }
                Err(err) => set_status_error(status, status_tone, format!("读取文章失败：{err}")),
            },
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });

    let now = current_time_utc();
    let archived_count = entries()
        .iter()
        .filter(|entry| is_entry_archived(entry.published_at, archive_after_months(), now))
        .count();
    let visible_entries = entries()
        .into_iter()
        .filter(|entry| {
            show_archived() || !is_entry_archived(entry.published_at, archive_after_months(), now)
        })
        .collect::<Vec<_>>();
    let source_filter_options = if feed_id.is_some() {
        Vec::new()
    } else {
        feeds().into_iter().map(|feed| (feed.id, feed.title, feed.url)).collect::<Vec<_>>()
    };
    let source_grouped_entries = group_entries_by_source_tree(&visible_entries);
    let time_grouped_entries = group_entries_by_time_tree(&visible_entries);
    let directory_months = build_directory_months(&time_grouped_entries);
    let directory_sources = build_directory_sources(&source_grouped_entries);
    let group_nav_items = match grouping_mode() {
        EntryGroupingMode::Time => build_month_nav_items(&time_grouped_entries),
        EntryGroupingMode::Source => build_group_nav_items(&source_grouped_entries),
    };

    use_effect(move || {
        if !preferences_loaded() {
            return;
        }

        let next_grouping = grouping_mode_preference(grouping_mode());
        let next_show_archived = show_archived();
        let next_read_filter = read_filter();
        let next_starred_filter = starred_filter();
        let next_feed_urls = selected_feed_urls();

        spawn(async move {
            match AppServices::shared().await {
                Ok(services) => match services.load_settings().await {
                    Ok(mut settings) => {
                        let changed = settings.entry_grouping_mode != next_grouping
                            || settings.show_archived_entries != next_show_archived
                            || settings.entry_read_filter != next_read_filter
                            || settings.entry_starred_filter != next_starred_filter
                            || settings.entry_filtered_feed_urls != next_feed_urls;

                        if !changed {
                            return;
                        }

                        settings.entry_grouping_mode = next_grouping;
                        settings.show_archived_entries = next_show_archived;
                        settings.entry_read_filter = next_read_filter;
                        settings.entry_starred_filter = next_starred_filter;
                        settings.entry_filtered_feed_urls = next_feed_urls;
                        if let Err(err) = services.save_settings(&settings).await {
                            tracing::warn!(error = %err, "保存文章页偏好失败");
                        }
                    }
                    Err(err) => tracing::warn!(error = %err, "读取文章页偏好失败"),
                },
                Err(err) => tracing::warn!(error = %err, "初始化应用失败，无法保存文章页偏好"),
            }
        });
    });

    rsx! {
        section { class: "page page-entries", "data-page": "entries",
            AppNav {}
            div { class: "entries-layout",
                div { class: "entries-main",
                    div { class: "reading-header reading-header--entries",
                        div { class: "reading-header__row",
                            h2 { if feed_id.is_some() { "订阅文章" } else { "文章" } }
                        }
                    }
                    if feed_id.is_some() {
                        div { class: "entries-back-link",
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
                        controls_hidden,
                        grouping_mode,
                        show_archived,
                        archive_after_months,
                        visible_entries: &visible_entries,
                        archived_count,
                        source_filter_options: &source_filter_options,
                        read_filter,
                        starred_filter,
                        selected_feed_urls,
                        group_nav_items: &group_nav_items,
                        status,
                        status_tone,
                    }) }
                    if entries().is_empty() {
                        StatusBanner {
                            message: if feed_id.is_some() {
                                "这个订阅下还没有可显示的文章，先尝试刷新该 feed。".to_string()
                            } else {
                                "没有可显示的文章，先去订阅页添加并刷新 feed。".to_string()
                            },
                            tone: "info".to_string()
                        }
                    } else if visible_entries.is_empty() {
                        StatusBanner {
                            message: "当前结果中的文章都已被自动归档，勾选“显示已归档文章”即可查看。".to_string(),
                            tone: "info".to_string()
                        }
                    } else {
                        div { class: "entry-groups",
                            if grouping_mode() == EntryGroupingMode::Time {
                                for month in time_grouped_entries {
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
                                                                { render_entry_card(entry, reload_tick, status, status_tone) }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                for group in source_grouped_entries {
                                    section { class: "entry-group", key: "{group.title}", id: "{group_anchor_id(&group.title)}",
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
                                                        { render_entry_card(entry, reload_tick, status, status_tone) }
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
                if !group_nav_items.is_empty() {
                    { render_entry_directory(
                        grouping_mode(),
                        &directory_months,
                        &directory_sources,
                        expanded_directory_sources,
                    ) }
                }
            }
        }
    }
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
