use dioxus::prelude::*;
use rssr_domain::{
    EntryQuery, EntrySummary, FeedSummary, ReadFilter, StarredFilter, StartupView, UserSettings,
    is_entry_archived,
};
use std::collections::{BTreeMap, BTreeSet};
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::components::entry_filters::EntryFilters;
use crate::{
    app::{AppNav, AppUiState},
    bootstrap::AppServices,
    components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation,
    router::AppRoute,
    status::{set_status_error, set_status_info},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryGroupingMode {
    Time,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryMonthGroup {
    anchor_id: String,
    title: String,
    subtitle: String,
    dates: Vec<EntryDateGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntrySourceGroup {
    anchor_id: String,
    title: String,
    subtitle: String,
    months: Vec<EntrySourceMonthGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntrySourceMonthGroup {
    anchor_id: String,
    title: String,
    subtitle: String,
    entries: Vec<EntrySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryDateGroup {
    anchor_id: String,
    title: String,
    subtitle: String,
    sources: Vec<EntryDateSourceGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryDateSourceGroup {
    anchor_id: String,
    title: String,
    subtitle: String,
    entries: Vec<EntrySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryGroupNavItem {
    anchor_id: String,
    title: String,
    subtitle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryDirectoryMonth {
    anchor_id: String,
    title: String,
    subtitle: String,
    dates: Vec<EntryDirectoryDate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryDirectorySource {
    anchor_id: String,
    title: String,
    subtitle: String,
    months: Vec<EntryDirectoryMonth>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryDirectoryDate {
    anchor_id: String,
    title: String,
    subtitle: String,
}

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

    let mut ui = use_context::<AppUiState>();
    let mut entries = use_signal(Vec::<EntrySummary>::new);
    let mut feeds = use_signal(Vec::<FeedSummary>::new);
    let mut read_filter = use_signal(ReadFilter::default);
    let mut starred_filter = use_signal(StarredFilter::default);
    let mut selected_feed_ids = use_signal(Vec::<i64>::new);
    let mut show_archived = use_signal(|| false);
    let mut grouping_mode = use_signal(|| EntryGroupingMode::Time);
    let mut archive_after_months = use_signal(|| UserSettings::default().archive_after_months);
    let mut mobile_directory_open = use_signal(|| false);
    let mut expanded_directory_sources = use_signal(BTreeSet::<String>::new);
    let mut controls_hidden = use_signal(initial_entry_controls_hidden);
    let reload_tick = use_signal(|| 0_u64);
    let status = use_signal(|| "正在加载文章列表…".to_string());
    let status_tone = use_signal(|| "info".to_string());

    use_resource(move || async move {
        if let Some(feed_id) = feed_id
            && let Ok(services) = AppServices::shared().await
        {
            let _ = services.remember_last_opened_feed_id(feed_id).await;
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
                    feed_ids: if feed_id.is_some() { Vec::new() } else { selected_feed_ids() },
                    search_title: (!(ui.entry_search)().trim().is_empty())
                        .then(|| (ui.entry_search)()),
                    limit: None,
                })
                .await
            {
                Ok(items) => {
                    if let Ok(feed_items) = services.list_feeds().await {
                        feeds.set(feed_items);
                    }
                    if let Ok(settings) = services.load_settings().await {
                        archive_after_months.set(settings.archive_after_months);
                    }
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
        feeds().into_iter().map(|feed| (feed.id, feed.title)).collect::<Vec<_>>()
    };
    let source_grouped_entries = group_entries_by_source_tree(&visible_entries);
    let time_grouped_entries = group_entries_by_time_tree(&visible_entries);
    let directory_months = build_directory_months(&time_grouped_entries);
    let directory_sources = build_directory_sources(&source_grouped_entries);
    let group_nav_items = match grouping_mode() {
        EntryGroupingMode::Time => build_month_nav_items(&time_grouped_entries),
        EntryGroupingMode::Source => build_group_nav_items(&source_grouped_entries),
    };

    rsx! {
        section { class: "page page-entries", "data-page": "entries",
            AppNav {}
            div { class: "entries-layout",
                div { class: "entries-main",
                    div { class: "reading-header reading-header--entries",
                        div { class: "reading-header__row",
                            h2 { if feed_id.is_some() { "订阅文章" } else { "文章" } }
                            button {
                                class: "button secondary entry-controls-toggle",
                                "data-action": if controls_hidden() { "show-entry-controls" } else { "hide-entry-controls" },
                                onclick: move |_| {
                                    let next = !controls_hidden();
                                    remember_entry_controls_hidden(next);
                                    controls_hidden.set(next);
                                },
                                if controls_hidden() { "显示筛选与组织" } else { "收起筛选与组织" }
                            }
                        }
                        p {
                            class: "page-intro",
                            if feed_id.is_some() {
                                "当前只显示所选订阅的文章。你仍然可以按时间或按来源组织当前结果，然后继续进入阅读页。"
                            } else {
                                "文章默认按时间组织展示。你也可以切换为按来源浏览，然后继续进入阅读页。"
                            }
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
                    if controls_hidden() {
                        div { class: "entry-controls-reveal",
                            span { class: "entry-controls-reveal__label", "阅读控制已收起" }
                            button {
                                class: "button secondary",
                                "data-action": "show-entry-controls",
                                onclick: move |_| {
                                    remember_entry_controls_hidden(false);
                                    controls_hidden.set(false);
                                },
                                "显示筛选与组织"
                            }
                        }
                    } else {
                        div { class: "entry-controls-panel",
                            div { class: "entry-organize-bar entry-organize-bar--airy",
                                label { class: "field-label", r#for: "entry-grouping-mode", "组织方式" }
                                select {
                                    id: "entry-grouping-mode",
                                    class: "select-input",
                                    "data-action": if grouping_mode() == EntryGroupingMode::Time { "group-by-time" } else { "group-by-source" },
                                    value: match grouping_mode() {
                                        EntryGroupingMode::Time => "time",
                                        EntryGroupingMode::Source => "source",
                                    },
                                    onchange: move |event| {
                                        grouping_mode.set(match event.value().as_str() {
                                            "source" => EntryGroupingMode::Source,
                                            _ => EntryGroupingMode::Time,
                                        });
                                    },
                                    option { value: "time", "按时间" }
                                    option { value: "source", "按来源" }
                                }
                                label { class: "entry-filters__toggle",
                                    input {
                                        r#type: "checkbox",
                                        "data-action": "toggle-archived",
                                        checked: show_archived(),
                                        onchange: move |event| show_archived.set(event.checked())
                                    }
                                    span { "显示已归档文章" }
                                }
                                p { class: "page-intro",
                                    if show_archived() {
                                        "当前同时显示归档文章。"
                                    } else {
                                        "默认隐藏超过 {archive_after_months()} 个月的归档文章。"
                                    }
                                }
                            }
                            div { class: "entry-overview entry-overview--airy",
                                div { class: "entry-overview__metric",
                                    span { class: "entry-overview__label", "当前结果" }
                                    strong { class: "entry-overview__value", "{visible_entries.len()}" }
                                }
                                div { class: "entry-overview__metric",
                                    span { class: "entry-overview__label", "归档文章" }
                                    strong { class: "entry-overview__value", "{archived_count}" }
                                }
                                div { class: "entry-overview__metric entry-overview__metric--hint",
                                    span { class: "entry-overview__label", "当前组织" }
                                    strong {
                                        class: "entry-overview__value",
                                        if grouping_mode() == EntryGroupingMode::Time { "按时间" } else { "按来源" }
                                    }
                                }
                            }
                            if !group_nav_items.is_empty() {
                                button {
                                    class: "button secondary entry-mobile-directory-toggle",
                                    "data-action": if mobile_directory_open() { "close-entry-directory" } else { "open-entry-directory" },
                                    onclick: move |_| mobile_directory_open.set(!mobile_directory_open()),
                                    if mobile_directory_open() { "收起目录" } else { "目录" }
                                }
                                nav {
                                    class: if mobile_directory_open() {
                                        "entry-top-directory is-open"
                                    } else {
                                        "entry-top-directory"
                                    },
                                    "aria-label": "文章目录",
                                    for item in &group_nav_items {
                                        a {
                                            class: "entry-top-directory__chip",
                                            href: format!("#{}", item.anchor_id),
                                            onclick: move |_| mobile_directory_open.set(false),
                                            span { class: "entry-top-directory__title", "{item.title}" }
                                            span { class: "entry-top-directory__meta", "{item.subtitle}" }
                                        }
                                    }
                                }
                            }
                            EntryFilters {
                                search: (ui.entry_search)(),
                                read_filter: read_filter(),
                                starred_filter: starred_filter(),
                                available_sources: source_filter_options.clone(),
                                selected_feed_ids: selected_feed_ids(),
                                on_search: move |value| ui.entry_search.set(value),
                                on_change_read_filter: move |value| read_filter.set(value),
                                on_change_starred_filter: move |value| starred_filter.set(value),
                                on_change_selected_feed_ids: move |value| selected_feed_ids.set(value),
                            }
                            StatusBanner { message: status(), tone: status_tone() }
                            if archived_count > 0 && !show_archived() {
                                StatusBanner {
                                    message: format!("当前已自动归档 {} 篇较旧文章，可勾选“显示已归档文章”查看。", archived_count),
                                    tone: "info".to_string()
                                }
                            }
                        }
                    }
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
                    aside { class: "entry-directory-rail",
                        h3 { class: "entry-directory-rail__title", "目录" }
                        if grouping_mode() == EntryGroupingMode::Time {
                            nav { class: "entry-directory-rail__nav", "aria-label": "文章目录导航",
                                for month in directory_months {
                                    div { class: "entry-directory-rail__section", key: "{month.anchor_id}",
                                        a {
                                            class: "entry-directory-rail__link entry-directory-rail__link--month",
                                            href: format!("#{}", month.anchor_id),
                                            span { class: "entry-directory-rail__link-title", "{month.title}" }
                                            span { class: "entry-directory-rail__link-meta", "{month.subtitle}" }
                                        }
                                        div { class: "entry-directory-rail__children",
                                            for date in month.dates {
                                                a {
                                                    class: "entry-directory-rail__link entry-directory-rail__link--date",
                                                    href: format!("#{}", date.anchor_id),
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
                                        let is_open = expanded_directory_sources().contains(&anchor_id);
                                        let toggle_anchor = anchor_id.clone();
                                        rsx! {
                                            div { class: "entry-directory-rail__subsection", key: "{anchor_id}",
                                                button {
                                                    class: if is_open {
                                                        "entry-directory-rail__toggle is-open"
                                                    } else {
                                                        "entry-directory-rail__toggle"
                                                    },
                                                    "data-action": if is_open { "collapse-directory-source" } else { "expand-directory-source" },
                                                    onclick: move |_| {
                                                        let mut next = expanded_directory_sources();
                                                        if !next.insert(toggle_anchor.clone()) {
                                                            next.remove(&toggle_anchor);
                                                        }
                                                        expanded_directory_sources.set(next);
                                                    },
                                                    span { class: "entry-directory-rail__toggle-text", "{source.title}" }
                                                    span { class: "entry-directory-rail__toggle-meta", "{source.subtitle}" }
                                                }
                                                if is_open {
                                                    div { class: "entry-directory-rail__grandchildren",
                                                        for month in source.months {
                                                            a {
                                                                class: "entry-directory-rail__link",
                                                                href: format!("#{}", month.anchor_id),
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
        }
    }
}

fn format_entry_date_utc(published_at: Option<OffsetDateTime>) -> Option<String> {
    const ENTRY_DATE_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day]");

    published_at.and_then(|value| value.to_offset(UtcOffset::UTC).format(ENTRY_DATE_FORMAT).ok())
}

fn render_entry_card(
    entry: EntrySummary,
    reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
    let read_title = entry.title.clone();
    let starred_title = entry.title.clone();

    rsx! {
        li { class: "entry-card entry-card--reading", key: "{entry.id}",
            Link {
                class: "entry-card__title",
                to: AppRoute::ReaderPage { entry_id: entry.id },
                "{entry.title}"
            }
            div { class: "entry-card__meta",
                "{entry.feed_title}"
                if let Some(date) = format_entry_date_utc(entry.published_at) { " · {date}" }
                if entry.is_read { " · 已读" } else { " · 未读" }
                if entry.is_starred { " · 已收藏" }
            }
            div { class: "entry-card__actions",
                button {
                    class: "button secondary",
                    "data-action": "mark-read",
                    onclick: move |_| {
                        let mut reload_tick = reload_tick;
                        let title = read_title.clone();
                        spawn(async move {
                            match AppServices::shared().await {
                                Ok(services) => match services.set_read(entry.id, !entry.is_read).await {
                                    Ok(()) => {
                                        set_status_info(
                                            status,
                                            status_tone,
                                            format!(
                                                "已将《{}》{}。",
                                                title,
                                                if entry.is_read { "标记为未读" } else { "标记为已读" }
                                            ),
                                        );
                                        reload_tick += 1;
                                    }
                                    Err(err) => set_status_error(
                                        status,
                                        status_tone,
                                        format!("更新已读状态失败：{err}"),
                                    ),
                                },
                                Err(err) => set_status_error(
                                    status,
                                    status_tone,
                                    format!("初始化应用失败：{err}"),
                                ),
                            }
                        });
                    },
                    if entry.is_read { "标未读" } else { "标已读" }
                }
                button {
                    class: "button secondary",
                    "data-action": "toggle-starred",
                    onclick: move |_| {
                        let mut reload_tick = reload_tick;
                        let title = starred_title.clone();
                        spawn(async move {
                            match AppServices::shared().await {
                                Ok(services) => match services.set_starred(entry.id, !entry.is_starred).await {
                                    Ok(()) => {
                                        set_status_info(
                                            status,
                                            status_tone,
                                            format!(
                                                "已{}《{}》。",
                                                if entry.is_starred { "取消收藏" } else { "收藏" },
                                                title
                                            ),
                                        );
                                        reload_tick += 1;
                                    }
                                    Err(err) => set_status_error(
                                        status,
                                        status_tone,
                                        format!("更新收藏状态失败：{err}"),
                                    ),
                                },
                                Err(err) => set_status_error(
                                    status,
                                    status_tone,
                                    format!("初始化应用失败：{err}"),
                                ),
                            }
                        });
                    },
                    if entry.is_starred { "取消收藏" } else { "收藏" }
                }
            }
        }
    }
}

fn group_entries_by_time_tree(entries: &[EntrySummary]) -> Vec<EntryMonthGroup> {
    let mut groups: BTreeMap<(i32, u8), Vec<EntrySummary>> = BTreeMap::new();
    let mut undated_entries = Vec::new();

    for entry in entries {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push(entry.clone());
        } else {
            undated_entries.push(entry.clone());
        }
    }

    let mut grouped = groups
        .into_iter()
        .rev()
        .map(|((year, month), mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            let title = format!("{year} 年 {month:02} 月");
            EntryMonthGroup {
                anchor_id: group_anchor_id(&title),
                title,
                subtitle: format!("{} 篇文章", items.len()),
                dates: group_date_buckets(&items),
            }
        })
        .collect::<Vec<_>>();

    if !undated_entries.is_empty() {
        undated_entries.sort_by_key(|entry| {
            std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
        });
        let title = "未标注日期".to_string();
        grouped.push(EntryMonthGroup {
            anchor_id: group_anchor_id(&title),
            title,
            subtitle: format!("{} 篇文章", undated_entries.len()),
            dates: group_date_buckets(&undated_entries),
        });
    }

    grouped
}

fn group_entries_by_source_tree(entries: &[EntrySummary]) -> Vec<EntrySourceGroup> {
    let mut groups: BTreeMap<String, Vec<EntrySummary>> = BTreeMap::new();
    let mut latest_seen: BTreeMap<String, Option<OffsetDateTime>> = BTreeMap::new();

    for entry in entries {
        groups.entry(entry.feed_title.clone()).or_default().push(entry.clone());
        let latest = latest_seen.entry(entry.feed_title.clone()).or_insert(None);
        if latest.is_none() || entry.published_at > *latest {
            *latest = entry.published_at;
        }
    }

    let mut grouped = groups
        .into_iter()
        .map(|(feed_title, mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            let latest = latest_seen.get(&feed_title).and_then(|value| *value);
            (
                latest,
                EntrySourceGroup {
                    anchor_id: group_anchor_id(&feed_title),
                    title: feed_title,
                    subtitle: format!("{} 篇文章", items.len()),
                    months: group_source_months(&items),
                },
            )
        })
        .collect::<Vec<_>>();

    grouped.sort_by(|(left_latest, left_group), (right_latest, right_group)| {
        right_latest.cmp(left_latest).then_with(|| left_group.title.cmp(&right_group.title))
    });

    grouped.into_iter().map(|(_, group)| group).collect()
}

fn group_date_buckets(entries: &[EntrySummary]) -> Vec<EntryDateGroup> {
    let mut groups: BTreeMap<String, Vec<EntrySummary>> = BTreeMap::new();

    for entry in entries {
        let key =
            format_entry_date_utc(entry.published_at).unwrap_or_else(|| "未标注日期".to_string());
        groups.entry(key).or_default().push(entry.clone());
    }

    groups
        .into_iter()
        .rev()
        .map(|(date, mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            let anchor_id = group_anchor_id(&format!("{}-{}", date, items[0].id));
            EntryDateGroup {
                anchor_id,
                title: date,
                subtitle: format!("{} 篇文章", items.len()),
                sources: group_date_sources(&items),
            }
        })
        .collect()
}

fn group_date_sources(entries: &[EntrySummary]) -> Vec<EntryDateSourceGroup> {
    let mut groups: BTreeMap<String, Vec<EntrySummary>> = BTreeMap::new();

    for entry in entries {
        groups.entry(entry.feed_title.clone()).or_default().push(entry.clone());
    }

    groups
        .into_iter()
        .map(|(feed_title, mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            let anchor_id = group_anchor_id(&format!("{}-{}", feed_title, items[0].id));
            EntryDateSourceGroup {
                anchor_id,
                title: feed_title,
                subtitle: format!("{} 篇文章", items.len()),
                entries: items,
            }
        })
        .collect()
}

fn group_source_months(entries: &[EntrySummary]) -> Vec<EntrySourceMonthGroup> {
    let mut groups: BTreeMap<(i32, u8), Vec<EntrySummary>> = BTreeMap::new();
    let mut undated_entries = Vec::new();

    for entry in entries {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push(entry.clone());
        } else {
            undated_entries.push(entry.clone());
        }
    }

    let mut months = groups
        .into_iter()
        .rev()
        .map(|((year, month), mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            let title = format!("{year} 年 {month:02} 月");
            let anchor_id = group_anchor_id(&format!("{}-{}", title, items[0].id));
            EntrySourceMonthGroup {
                anchor_id,
                title,
                subtitle: format!("{} 篇文章", items.len()),
                entries: items,
            }
        })
        .collect::<Vec<_>>();

    if !undated_entries.is_empty() {
        undated_entries.sort_by_key(|entry| {
            std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
        });
        let title = "未标注日期".to_string();
        let anchor_id = group_anchor_id(&format!("{}-{}", title, undated_entries[0].id));
        months.push(EntrySourceMonthGroup {
            anchor_id,
            title,
            subtitle: format!("{} 篇文章", undated_entries.len()),
            entries: undated_entries,
        });
    }

    months
}

fn build_month_nav_items(groups: &[EntryMonthGroup]) -> Vec<EntryGroupNavItem> {
    groups
        .iter()
        .map(|group| EntryGroupNavItem {
            anchor_id: group.anchor_id.clone(),
            title: group.title.clone(),
            subtitle: group.subtitle.clone(),
        })
        .collect()
}

fn build_directory_months(groups: &[EntryMonthGroup]) -> Vec<EntryDirectoryMonth> {
    groups
        .iter()
        .map(|month| EntryDirectoryMonth {
            anchor_id: month.anchor_id.clone(),
            title: month.title.clone(),
            subtitle: month.subtitle.clone(),
            dates: month
                .dates
                .iter()
                .map(|date| EntryDirectoryDate {
                    anchor_id: date.anchor_id.clone(),
                    title: date.title.clone(),
                    subtitle: date.subtitle.clone(),
                })
                .collect(),
        })
        .collect()
}

fn build_group_nav_items(groups: &[EntrySourceGroup]) -> Vec<EntryGroupNavItem> {
    groups
        .iter()
        .map(|group| EntryGroupNavItem {
            anchor_id: group.anchor_id.clone(),
            title: group.title.clone(),
            subtitle: group.subtitle.clone(),
        })
        .collect()
}

fn build_directory_sources(groups: &[EntrySourceGroup]) -> Vec<EntryDirectorySource> {
    groups
        .iter()
        .map(|group| EntryDirectorySource {
            anchor_id: group.anchor_id.clone(),
            title: group.title.clone(),
            subtitle: group.subtitle.clone(),
            months: group
                .months
                .iter()
                .map(|month| EntryDirectoryMonth {
                    anchor_id: month.anchor_id.clone(),
                    title: month.title.clone(),
                    subtitle: month.subtitle.clone(),
                    dates: Vec::new(),
                })
                .collect(),
        })
        .collect()
}

fn group_anchor_id(title: &str) -> String {
    let slug = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else if ch.is_whitespace() || matches!(ch, '-' | '_' | '/' | '.') {
                '-'
            } else {
                ch
            }
        })
        .collect::<String>();
    format!("entry-group-{}", slug.trim_matches('-'))
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

fn initial_entry_controls_hidden() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
            && let Ok(Some(value)) = storage.get_item("rssr-entry-controls-hidden")
        {
            return value == "1";
        }
    }

    false
}

fn remember_entry_controls_hidden(hidden: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
        {
            let _ = storage.set_item("rssr-entry-controls-hidden", if hidden { "1" } else { "0" });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{group_entries_by_source_tree, group_entries_by_time_tree};
    use rssr_domain::EntrySummary;
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    fn entry(id: i64, feed_title: &str, title: &str, published_at: Option<&str>) -> EntrySummary {
        EntrySummary {
            id,
            feed_id: id,
            title: title.to_string(),
            feed_title: feed_title.to_string(),
            published_at: published_at
                .map(|value| OffsetDateTime::parse(value, &Rfc3339).expect("parse published_at")),
            is_read: false,
            is_starred: false,
        }
    }

    #[test]
    fn groups_entries_by_time_in_descending_month_order() {
        let entries = vec![
            entry(1, "Alpha", "March one", Some("2026-03-21T08:00:00Z")),
            entry(2, "Beta", "April one", Some("2026-04-02T08:00:00Z")),
            entry(4, "Beta", "April two", Some("2026-04-02T09:00:00Z")),
            entry(3, "Beta", "No date", None),
        ];

        let groups = group_entries_by_time_tree(&entries);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].title, "2026 年 04 月");
        assert_eq!(groups[0].dates[0].title, "2026-04-02");
        assert_eq!(groups[0].dates[0].sources[0].title, "Beta");
        assert_eq!(groups[0].dates[0].sources[0].entries[0].title, "April two");
        assert_eq!(groups[1].title, "2026 年 03 月");
        assert_eq!(groups[2].title, "未标注日期");
    }

    #[test]
    fn groups_entries_by_source_using_latest_entry_order() {
        let entries = vec![
            entry(1, "Alpha", "Older alpha", Some("2026-03-21T08:00:00Z")),
            entry(2, "Beta", "Newest beta", Some("2026-04-02T08:00:00Z")),
            entry(3, "Alpha", "Newest alpha", Some("2026-04-01T08:00:00Z")),
        ];

        let groups = group_entries_by_source_tree(&entries);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].title, "Beta");
        assert_eq!(groups[1].title, "Alpha");
        assert_eq!(groups[1].months[0].entries[0].title, "Newest alpha");
    }
}
