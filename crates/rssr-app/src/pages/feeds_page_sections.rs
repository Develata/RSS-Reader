use dioxus::prelude::*;
use rssr_domain::FeedSummary;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    bootstrap::AppServices,
    components::status_banner::StatusBanner,
    router::AppRoute,
    status::{set_status_error, set_status_info},
};

#[component]
pub(crate) fn FeedComposeSection(
    feed_url: Signal<String>,
    reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
    rsx! {
        div { class: "feed-workbench feed-workbench--single",
            div { class: "feed-compose-card",
                div { class: "feed-compose-card__header",
                    h3 { "新增订阅" }
                }
                div { class: "feed-form",
                    label {
                        class: "sr-only",
                        r#for: "feed-url-input",
                        "订阅地址"
                    }
                    input {
                        id: "feed-url-input",
                        name: "feed_url",
                        class: "text-input",
                        "data-action": "feed-url-input",
                        value: "{feed_url}",
                        placeholder: "https://example.com/feed.xml",
                        onkeydown: move |event| {
                            if !is_paste_shortcut(&event) {
                                return;
                            }

                            event.prevent_default();
                            spawn(async move {
                                match paste_feed_url_from_clipboard().await {
                                    Ok(Some(text)) => feed_url.set(text),
                                    Ok(None) => {}
                                    Err(err) => set_status_error(
                                        status,
                                        status_tone,
                                        format!("读取系统剪贴板失败：{err}"),
                                    ),
                                }
                            });
                        },
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
            }
        }
    }
}

fn is_paste_shortcut(event: &KeyboardEvent) -> bool {
    let modifiers = event.modifiers();
    let has_paste_modifier =
        modifiers.contains(Modifiers::META) || modifiers.contains(Modifiers::CONTROL);
    has_paste_modifier && event.key().to_string().eq_ignore_ascii_case("v")
}

async fn paste_feed_url_from_clipboard() -> Result<Option<String>, String> {
    document::eval(
        r#"
        if (typeof navigator === "undefined" || !navigator.clipboard || !navigator.clipboard.readText) {
            return null;
        }
        return navigator.clipboard.readText();
        "#,
    )
    .join::<Option<String>>()
    .await
    .map_err(|err| err.to_string())
}

#[component]
pub(crate) fn ConfigExchangeSection(
    config_text: Signal<String>,
    opml_text: Signal<String>,
    reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
    rsx! {
        div { class: "exchange-header",
            h3 { "配置交换" }
        }
        div { class: "exchange-grid",
            div { class: "exchange-card",
                div { class: "settings-card__header",
                    h3 { "配置包 JSON" }
                }
                label {
                    class: "sr-only",
                    r#for: "config-text",
                    "配置包 JSON 文本"
                }
                textarea {
                    id: "config-text",
                    name: "config_text",
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
                div { class: "settings-card__header",
                    h3 { "OPML" }
                }
                label {
                    class: "sr-only",
                    r#for: "opml-text",
                    "OPML 文本"
                }
                textarea {
                    id: "opml-text",
                    name: "opml_text",
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
    }
}

#[component]
pub(crate) fn SavedFeedsSection(
    feeds: Signal<Vec<FeedSummary>>,
    pending_delete_feed: Signal<Option<i64>>,
    reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
    if feeds().is_empty() {
        return rsx! {
            StatusBanner { message: "还没有订阅，先添加一个 feed URL。".to_string(), tone: "info".to_string() }
        };
    }

    rsx! {
        div { class: "exchange-header exchange-header--saved",
            h3 { "已保存订阅" }
        }
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
