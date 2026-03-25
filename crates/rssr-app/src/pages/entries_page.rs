use dioxus::prelude::*;
use dioxus_router::prelude::Link;
use rssr_domain::{EntryQuery, EntrySummary};

use crate::components::entry_filters::EntryFilters;
use crate::{
    app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner, router::AppRoute,
};

#[component]
pub fn EntriesPage() -> Element {
    let mut entries = use_signal(Vec::<EntrySummary>::new);
    let mut search = use_signal(String::new);
    let mut unread_only = use_signal(|| false);
    let mut starred_only = use_signal(|| false);
    let reload_tick = use_signal(|| 0_u64);
    let mut status = use_signal(|| "正在加载文章列表…".to_string());

    let _ = use_resource(move || async move {
        let _ = reload_tick();
        match AppServices::shared().await {
            Ok(services) => match services
                .list_entries(&EntryQuery {
                    unread_only: unread_only(),
                    starred_only: starred_only(),
                    search_title: (!search().trim().is_empty()).then(|| search()),
                    ..EntryQuery::default()
                })
                .await
            {
                Ok(items) => {
                    status.set(format!("共 {} 篇文章。", items.len()));
                    entries.set(items);
                }
                Err(err) => status.set(format!("读取文章失败：{err}")),
            },
            Err(err) => status.set(format!("初始化应用失败：{err}")),
        }
    });

    rsx! {
        section { class: "page page-entries",
            AppNav {}
            h2 { "文章" }
            p { class: "page-intro", "文章按发布时间倒序展示。选择一篇即可进入阅读页。" }
            EntryFilters {
                search: search(),
                unread_only: unread_only(),
                starred_only: starred_only(),
                on_search: move |value| search.set(value),
                on_toggle_unread: move |value| unread_only.set(value),
                on_toggle_starred: move |value| starred_only.set(value),
            }
            StatusBanner { message: status(), tone: "info".to_string() }
            if entries().is_empty() {
                StatusBanner { message: "没有可显示的文章，先去订阅页添加并刷新 feed。".to_string(), tone: "info".to_string() }
            } else {
                ul { class: "entry-list",
                    for entry in entries() {
                        li { class: "entry-card", key: "{entry.id}",
                            Link { class: "entry-card__title", to: AppRoute::ReaderPage { entry_id: entry.id }, "{entry.title}" }
                            div { class: "entry-card__meta",
                                "{entry.feed_title}"
                                if entry.is_read { " · 已读" } else { " · 未读" }
                                if entry.is_starred { " · 已收藏" }
                            }
                            div { class: "entry-card__actions",
                                button {
                                    class: "button secondary",
                                    onclick: move |_| {
                                        let mut reload_tick = reload_tick;
                                        spawn(async move {
                                            if let Ok(services) = AppServices::shared().await {
                                                let _ = services.set_read(entry.id, !entry.is_read).await;
                                                reload_tick += 1;
                                            }
                                        });
                                    },
                                    if entry.is_read { "标未读" } else { "标已读" }
                                }
                                button {
                                    class: "button secondary",
                                    onclick: move |_| {
                                        let mut reload_tick = reload_tick;
                                        spawn(async move {
                                            if let Ok(services) = AppServices::shared().await {
                                                let _ = services.set_starred(entry.id, !entry.is_starred).await;
                                                reload_tick += 1;
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
