use dioxus::prelude::*;
use rssr_domain::{EntryQuery, EntrySummary};
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::components::entry_filters::EntryFilters;
use crate::{
    app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner, router::AppRoute,
};

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
    let reload_tick = use_signal(|| 0_u64);
    let status = use_signal(|| "正在加载文章列表…".to_string());
    let status_tone = use_signal(|| "info".to_string());

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
                    set_status_info(status, status_tone, format!("共 {} 篇文章。", items.len()));
                    entries.set(items);
                }
                Err(err) => set_status_error(status, status_tone, format!("读取文章失败：{err}")),
            },
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });

    rsx! {
        section { class: "page page-entries", "data-page": "entries",
            AppNav {}
            h2 { if feed_id.is_some() { "订阅文章" } else { "文章" } }
            p {
                class: "page-intro",
                if feed_id.is_some() {
                    "当前只显示所选订阅的文章。选择一篇即可进入阅读页。"
                } else {
                    "文章按发布时间倒序展示。选择一篇即可进入阅读页。"
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
            EntryFilters {
                search: search(),
                unread_only: unread_only(),
                starred_only: starred_only(),
                on_search: move |value| search.set(value),
                on_toggle_unread: move |value| unread_only.set(value),
                on_toggle_starred: move |value| starred_only.set(value),
            }
            StatusBanner { message: status(), tone: status_tone() }
            if entries().is_empty() {
                StatusBanner {
                    message: if feed_id.is_some() {
                        "这个订阅下还没有可显示的文章，先尝试刷新该 feed。".to_string()
                    } else {
                        "没有可显示的文章，先去订阅页添加并刷新 feed。".to_string()
                    },
                    tone: "info".to_string()
                }
            } else {
                ul { class: "entry-list",
                    for entry in entries() {
                        {
                            let read_title = entry.title.clone();
                            let starred_title = entry.title.clone();
                            rsx! {
                                li { class: "entry-card", key: "{entry.id}",
                                    Link { class: "entry-card__title", to: AppRoute::ReaderPage { entry_id: entry.id }, "{entry.title}" }
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

fn format_entry_date_utc(published_at: Option<OffsetDateTime>) -> Option<String> {
    const ENTRY_DATE_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day]");

    published_at.and_then(|value| value.to_offset(UtcOffset::UTC).format(ENTRY_DATE_FORMAT).ok())
}

fn set_status_info(mut status: Signal<String>, mut status_tone: Signal<String>, message: String) {
    status.set(message);
    status_tone.set("info".to_string());
}

fn set_status_error(mut status: Signal<String>, mut status_tone: Signal<String>, message: String) {
    status.set(message);
    status_tone.set("error".to_string());
}
