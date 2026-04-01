use dioxus::prelude::*;
use rssr_domain::FeedSummary;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner, router::AppRoute,
};

#[component]
pub fn FeedsPage() -> Element {
    let mut feed_url = use_signal(String::new);
    let mut config_text = use_signal(String::new);
    let mut opml_text = use_signal(String::new);
    let pending_delete_feed = use_signal(|| None::<i64>);
    let reload_tick = use_signal(|| 0_u64);
    let mut feeds = use_signal(Vec::<FeedSummary>::new);
    let mut feed_count = use_signal(|| 0_usize);
    let mut entry_count = use_signal(|| 0_usize);
    let status = use_signal(|| "输入一个 feed URL 后点击添加。".to_string());
    let status_tone = use_signal(|| "info".to_string());

    let _ = use_resource(move || async move {
        let _ = reload_tick();
        match AppServices::shared().await {
            Ok(services) => {
                match services.list_feeds().await {
                    Ok(items) => {
                        feed_count.set(items.len());
                        feeds.set(items);
                    }
                    Err(err) => {
                        set_status_error(status, status_tone, format!("读取订阅失败：{err}"));
                    }
                }

                match services.list_entries(&rssr_domain::EntryQuery::default()).await {
                    Ok(entries) => entry_count.set(entries.len()),
                    Err(err) => {
                        set_status_error(status, status_tone, format!("读取文章统计失败：{err}"));
                    }
                }
            }
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });

    rsx! {
        section { class: "page page-feeds", "data-page": "feeds",
            AppNav {}
            h2 { "订阅" }
            p { class: "page-intro", "把 feed URL 保存到本地库，并立即执行首次刷新。" }
            div { class: "stats-grid",
                div { class: "stat-card",
                    div { class: "stat-card__label", "订阅数" }
                    div { class: "stat-card__value", "{feed_count}" }
                }
                div { class: "stat-card",
                    div { class: "stat-card__label", "文章数" }
                    div { class: "stat-card__value", "{entry_count}" }
                }
            }
            StatusBanner { message: status(), tone: status_tone() }
            div { class: "feed-form",
                input {
                    class: "text-input",
                    "data-action": "feed-url-input",
                    value: "{feed_url}",
                    placeholder: "https://example.com/feed.xml",
                    oninput: move |event| feed_url.set(event.value())
                }
                button {
                    class: "button",
                    "data-action": "add-feed",
                    onclick: move |_| {
                        let url = feed_url();
                        let mut reload_tick = reload_tick;
                        spawn(async move {
                            match AppServices::shared().await {
                                Ok(services) => match services.add_subscription(&url).await {
                                    Ok(()) => {
                                        set_status_info(status, status_tone, "订阅已保存并完成首次刷新。".to_string());
                                        feed_url.set(String::new());
                                        reload_tick += 1;
                                    }
                                    Err(err) => {
                                        if err.to_string().contains("首次刷新订阅失败") {
                                            set_status_error(status, status_tone, format!("订阅已保存，但首次刷新失败：{err}"));
                                            feed_url.set(String::new());
                                            reload_tick += 1;
                                        } else {
                                            set_status_error(status, status_tone, format!("保存订阅失败：{err}"));
                                        }
                                    }
                                },
                                Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
                            }
                        });
                    },
                    "添加订阅"
                }
                button {
                    class: "button secondary",
                    "data-action": "refresh-all",
                    onclick: move |_| {
                        let mut reload_tick = reload_tick;
                        spawn(async move {
                            match AppServices::shared().await {
                                Ok(services) => match services.refresh_all().await {
                                    Ok(()) => {
                                        set_status_info(status, status_tone, "刷新完成。".to_string());
                                        reload_tick += 1;
                                    }
                                    Err(err) => set_status_error(status, status_tone, format!("刷新失败：{err}")),
                                },
                                Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
                            }
                        });
                    },
                    "刷新全部"
                }
            }
            div { class: "exchange-grid",
                div { class: "exchange-card",
                    h3 { "配置包 JSON" }
                    textarea {
                        class: "text-area",
                        "data-action": "config-text",
                        value: "{config_text}",
                        placeholder: "{{\n  \"version\": 1,\n  ...\n}}",
                        oninput: move |event| config_text.set(event.value())
                    }
                    div { class: "inline-actions",
                        button {
                            class: "button secondary",
                            "data-action": "export-config",
                            onclick: move |_| {
                                let mut config_text = config_text;
                                spawn(async move {
                                    match AppServices::shared().await {
                                        Ok(services) => match services.export_config_json().await {
                                            Ok(raw) => {
                                                config_text.set(raw);
                                                set_status_info(status, status_tone, "已导出配置包 JSON。".to_string());
                                            }
                                            Err(err) => set_status_error(status, status_tone, format!("导出配置包失败：{err}")),
                                        },
                                        Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
                                    }
                                });
                            },
                            "导出配置"
                        }
                        button {
                            class: "button secondary",
                            "data-action": "import-config",
                            onclick: move |_| {
                                let raw = config_text();
                                let mut reload_tick = reload_tick;
                                spawn(async move {
                                    match AppServices::shared().await {
                                        Ok(services) => match services.import_config_json(&raw).await {
                                            Ok(()) => {
                                                set_status_info(status, status_tone, "配置包已导入。".to_string());
                                                reload_tick += 1;
                                            }
                                            Err(err) => set_status_error(status, status_tone, format!("导入配置包失败：{err}")),
                                        },
                                        Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
                                    }
                                });
                            },
                            "导入配置"
                        }
                    }
                }
                div { class: "exchange-card",
                    h3 { "OPML" }
                    textarea {
                        class: "text-area",
                        "data-action": "opml-text",
                        value: "{opml_text}",
                        placeholder: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
                        oninput: move |event| opml_text.set(event.value())
                    }
                    div { class: "inline-actions",
                        button {
                            class: "button secondary",
                            "data-action": "export-opml",
                            onclick: move |_| {
                                let mut opml_text = opml_text;
                                spawn(async move {
                                    match AppServices::shared().await {
                                        Ok(services) => match services.export_opml().await {
                                            Ok(raw) => {
                                                opml_text.set(raw);
                                                set_status_info(status, status_tone, "已导出 OPML。".to_string());
                                            }
                                            Err(err) => set_status_error(status, status_tone, format!("导出 OPML 失败：{err}")),
                                        },
                                        Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
                                    }
                                });
                            },
                            "导出 OPML"
                        }
                        button {
                            class: "button secondary",
                            "data-action": "import-opml",
                            onclick: move |_| {
                                let raw = opml_text();
                                let mut reload_tick = reload_tick;
                                spawn(async move {
                                    match AppServices::shared().await {
                                        Ok(services) => match services.import_opml(&raw).await {
                                            Ok(()) => {
                                                set_status_info(status, status_tone, "OPML 已导入。".to_string());
                                                reload_tick += 1;
                                            }
                                            Err(err) => set_status_error(status, status_tone, format!("导入 OPML 失败：{err}")),
                                        },
                                        Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
                                    }
                                });
                            },
                            "导入 OPML"
                        }
                    }
                }
            }
            if feeds().is_empty() {
                StatusBanner { message: "还没有订阅，先添加一个 feed URL。".to_string(), tone: "info".to_string() }
            } else {
                ul { class: "feed-list",
                    for feed in feeds() {
                        {
                            let refresh_feed_title = feed.title.clone();
                            let delete_feed_title = feed.title.clone();
                            let is_delete_pending = pending_delete_feed() == Some(feed.id);
                            rsx! {
                                li { class: "feed-card", key: "{feed.id}",
                                    Link {
                                        class: "feed-card__title",
                                        "data-nav": "feed-entries",
                                        to: AppRoute::FeedEntriesPage { feed_id: feed.id },
                                        "{feed.title}"
                                    }
                                    p { class: "feed-card__url", "{feed.url}" }
                                    div { class: "feed-card__meta-group",
                                        p { class: "feed-card__meta", "本地文章 {feed.entry_count} · 未读 {feed.unread_count}" }
                                        p { class: "feed-card__meta", "{feed_refresh_status_text(&feed)}" }
                                        if let Some(error) = &feed.fetch_error {
                                            p { class: "feed-card__meta feed-card__meta--error", "最近失败：{error}" }
                                        }
                                    }
                                    div { class: "entry-card__actions",
                                        button {
                                            class: "button secondary",
                                            "data-action": "refresh-feed",
                                            onclick: move |_| {
                                                let mut reload_tick = reload_tick;
                                                let feed_title = refresh_feed_title.clone();
                                                let feed_id = feed.id;
                                                spawn(async move {
                                                    match AppServices::shared().await {
                                                        Ok(services) => match services.refresh_feed(feed_id).await {
                                                            Ok(()) => {
                                                                set_status_info(status, status_tone, format!("已刷新订阅：{}", feed_title));
                                                                reload_tick += 1;
                                                            }
                                                            Err(err) => set_status_error(status, status_tone, format!("刷新订阅失败：{err}")),
                                                        },
                                                        Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
                                                    }
                                                });
                                            },
                                            "刷新此订阅"
                                        }
                                        button {
                                            class: if is_delete_pending { "button danger" } else { "button secondary danger-outline" },
                                            "data-action": "remove-feed",
                                            onclick: move |_| {
                                                let mut reload_tick = reload_tick;
                                                let mut pending_delete_feed = pending_delete_feed;
                                                let feed_title = delete_feed_title.clone();
                                                let feed_id = feed.id;
                                                if pending_delete_feed() != Some(feed_id) {
                                                    pending_delete_feed.set(Some(feed_id));
                                                    set_status_info(status, status_tone, format!("再次点击即可删除订阅：{}", feed_title));
                                                } else {
                                                    spawn(async move {
                                                        match AppServices::shared().await {
                                                            Ok(services) => match services.remove_feed(feed_id).await {
                                                                Ok(()) => {
                                                                    pending_delete_feed.set(None);
                                                                    set_status_info(status, status_tone, format!("已删除订阅：{}", feed_title));
                                                                    reload_tick += 1;
                                                                }
                                                                Err(err) => {
                                                                    pending_delete_feed.set(None);
                                                                    set_status_error(status, status_tone, format!("删除订阅失败：{err}"));
                                                                }
                                                            },
                                                            Err(err) => {
                                                                pending_delete_feed.set(None);
                                                                set_status_error(status, status_tone, format!("初始化应用失败：{err}"));
                                                            }
                                                        }
                                                    });
                                                }
                                            },
                                            if is_delete_pending { "确认删除" } else { "删除订阅" }
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

fn set_status_info(mut status: Signal<String>, mut status_tone: Signal<String>, message: String) {
    status.set(message);
    status_tone.set("info".to_string());
}

fn set_status_error(mut status: Signal<String>, mut status_tone: Signal<String>, message: String) {
    status.set(message);
    status_tone.set("error".to_string());
}

fn feed_refresh_status_text(feed: &FeedSummary) -> String {
    match (feed.last_success_at, feed.last_fetched_at) {
        (Some(last_success_at), Some(last_fetched_at)) if last_fetched_at > last_success_at => {
            format!(
                "最近尝试：{} · 最近成功：{}",
                format_feed_datetime_utc(Some(last_fetched_at))
                    .unwrap_or_else(|| "未知".to_string()),
                format_feed_datetime_utc(Some(last_success_at))
                    .unwrap_or_else(|| "未知".to_string())
            )
        }
        (Some(last_success_at), _) => format!(
            "最近成功：{}",
            format_feed_datetime_utc(Some(last_success_at)).unwrap_or_else(|| "未知".to_string())
        ),
        (None, Some(last_fetched_at)) => format!(
            "最近尝试：{}",
            format_feed_datetime_utc(Some(last_fetched_at)).unwrap_or_else(|| "未知".to_string())
        ),
        (None, None) => "尚未刷新".to_string(),
    }
}

fn format_feed_datetime_utc(value: Option<OffsetDateTime>) -> Option<String> {
    const FEED_DATE_TIME_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day] [hour]:[minute] UTC");

    value.and_then(|timestamp| {
        timestamp.to_offset(UtcOffset::UTC).format(FEED_DATE_TIME_FORMAT).ok()
    })
}
