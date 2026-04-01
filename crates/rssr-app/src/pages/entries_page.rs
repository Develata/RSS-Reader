use dioxus::prelude::*;
use rssr_domain::{EntryQuery, EntrySummary, StartupView, UserSettings, is_entry_archived};
use std::collections::BTreeMap;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::components::entry_filters::EntryFilters;
use crate::{
    app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner, router::AppRoute,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryGroupingMode {
    Time,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryGroup {
    title: String,
    subtitle: String,
    entries: Vec<EntrySummary>,
}

#[component]
pub fn StartupPage() -> Element {
    let navigator = use_navigator();
    let status = use_signal(|| "正在准备你的阅读入口…".to_string());
    let status_tone = use_signal(|| "info".to_string());

    let _ = use_resource(move || async move {
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
    let mut entries = use_signal(Vec::<EntrySummary>::new);
    let mut search = use_signal(String::new);
    let mut unread_only = use_signal(|| false);
    let mut starred_only = use_signal(|| false);
    let mut show_archived = use_signal(|| false);
    let mut grouping_mode = use_signal(|| EntryGroupingMode::Time);
    let mut archive_after_months = use_signal(|| UserSettings::default().archive_after_months);
    let reload_tick = use_signal(|| 0_u64);
    let status = use_signal(|| "正在加载文章列表…".to_string());
    let status_tone = use_signal(|| "info".to_string());

    let _ = use_resource(move || async move {
        if let Some(feed_id) = feed_id
            && let Ok(services) = AppServices::shared().await
        {
            let _ = services.remember_last_opened_feed_id(feed_id).await;
        }
    });

    let _ = use_resource(move || async move {
        let _ = reload_tick();
        match AppServices::shared().await {
            Ok(services) => match services
                .list_entries(&EntryQuery {
                    feed_id,
                    unread_only: unread_only(),
                    starred_only: starred_only(),
                    search_title: (!search().trim().is_empty()).then(|| search()),
                    limit: None,
                })
                .await
            {
                Ok(items) => {
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
    let grouped_entries = group_entries(&visible_entries, grouping_mode());

    rsx! {
        section { class: "page page-entries", "data-page": "entries",
            AppNav {}
            div { class: "reading-header",
                h2 { if feed_id.is_some() { "订阅文章" } else { "文章" } }
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
                Link {
                    class: "button secondary",
                    "data-nav": "entries",
                    to: AppRoute::EntriesPage {},
                    "返回全部文章"
                }
            }
            div { class: "entry-organize-bar",
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
            EntryFilters {
                search: search(),
                unread_only: unread_only(),
                starred_only: starred_only(),
                on_search: move |value| search.set(value),
                on_toggle_unread: move |value| unread_only.set(value),
                on_toggle_starred: move |value| starred_only.set(value),
            }
            StatusBanner { message: status(), tone: status_tone() }
            if archived_count > 0 && !show_archived() {
                StatusBanner {
                    message: format!("当前已自动归档 {} 篇较旧文章，可勾选“显示已归档文章”查看。", archived_count),
                    tone: "info".to_string()
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
                    for group in grouped_entries {
                        section { class: "entry-group", key: "{group.title}",
                            div { class: "entry-group__header",
                                h3 { class: "entry-group__title", "{group.title}" }
                                p { class: "entry-group__meta", "{group.subtitle}" }
                            }
                            ul { class: "entry-list entry-list--grouped",
                                for entry in group.entries {
                                    {
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

fn group_entries(entries: &[EntrySummary], mode: EntryGroupingMode) -> Vec<EntryGroup> {
    match mode {
        EntryGroupingMode::Time => group_entries_by_time(entries),
        EntryGroupingMode::Source => group_entries_by_source(entries),
    }
}

fn group_entries_by_time(entries: &[EntrySummary]) -> Vec<EntryGroup> {
    let mut groups: BTreeMap<(i32, u8), Vec<EntrySummary>> = BTreeMap::new();
    let mut undated = Vec::new();

    for entry in entries {
        if let Some(published_at) = entry.published_at {
            let published_at = published_at.to_offset(UtcOffset::UTC);
            groups
                .entry((published_at.year(), published_at.month() as u8))
                .or_default()
                .push(entry.clone());
        } else {
            undated.push(entry.clone());
        }
    }

    let mut grouped = groups
        .into_iter()
        .rev()
        .map(|((year, month), mut items)| {
            items.sort_by_key(|entry| {
                std::cmp::Reverse(entry.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH))
            });
            EntryGroup {
                title: format!("{year} 年 {month:02} 月"),
                subtitle: format!("{} 篇文章", items.len()),
                entries: items,
            }
        })
        .collect::<Vec<_>>();

    if !undated.is_empty() {
        grouped.push(EntryGroup {
            title: "未标注日期".to_string(),
            subtitle: format!("{} 篇文章", undated.len()),
            entries: undated,
        });
    }

    grouped
}

fn group_entries_by_source(entries: &[EntrySummary]) -> Vec<EntryGroup> {
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
                EntryGroup {
                    title: feed_title,
                    subtitle: format!("{} 篇文章", items.len()),
                    entries: items,
                },
            )
        })
        .collect::<Vec<_>>();

    grouped.sort_by(|(left_latest, left_group), (right_latest, right_group)| {
        right_latest.cmp(left_latest).then_with(|| left_group.title.cmp(&right_group.title))
    });

    grouped.into_iter().map(|(_, group)| group).collect()
}

fn set_status_info(mut status: Signal<String>, mut status_tone: Signal<String>, message: String) {
    status.set(message);
    status_tone.set("info".to_string());
}

fn set_status_error(mut status: Signal<String>, mut status_tone: Signal<String>, message: String) {
    status.set(message);
    status_tone.set("error".to_string());
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

#[cfg(test)]
mod tests {
    use super::{EntryGroupingMode, group_entries};
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
            entry(3, "Beta", "No date", None),
        ];

        let groups = group_entries(&entries, EntryGroupingMode::Time);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].title, "2026 年 04 月");
        assert_eq!(groups[0].entries[0].title, "April one");
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

        let groups = group_entries(&entries, EntryGroupingMode::Source);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].title, "Beta");
        assert_eq!(groups[1].title, "Alpha");
        assert_eq!(groups[1].entries[0].title, "Newest alpha");
    }
}
