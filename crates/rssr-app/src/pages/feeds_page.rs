use dioxus::prelude::*;
use rssr_domain::FeedSummary;

use crate::{app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner};

#[component]
pub fn FeedsPage() -> Element {
    let mut feed_url = use_signal(String::new);
    let reload_tick = use_signal(|| 0_u64);
    let mut feeds = use_signal(Vec::<FeedSummary>::new);
    let mut status = use_signal(|| "输入一个 feed URL 后点击添加。".to_string());

    let _ = use_resource(move || async move {
        let _ = reload_tick();
        match AppServices::shared().await {
            Ok(services) => match services.list_feeds().await {
                Ok(items) => feeds.set(items),
                Err(err) => status.set(format!("读取订阅失败：{err}")),
            },
            Err(err) => status.set(format!("初始化应用失败：{err}")),
        }
    });

    rsx! {
        section { class: "page page-feeds",
            AppNav {}
            h2 { "订阅" }
            p { class: "page-intro", "把 feed URL 保存到本地库，并立即执行首次刷新。" }
            StatusBanner { message: status(), tone: "info".to_string() }
            div { class: "feed-form",
                input {
                    class: "text-input",
                    value: "{feed_url}",
                    placeholder: "https://example.com/feed.xml",
                    oninput: move |event| feed_url.set(event.value())
                }
                button {
                    class: "button",
                    onclick: move |_| {
                        let url = feed_url();
                        let mut status = status;
                        let mut reload_tick = reload_tick;
                        spawn(async move {
                            match AppServices::shared().await {
                                Ok(services) => match services.add_subscription(&url).await {
                                    Ok(()) => {
                                        status.set("订阅已保存并完成首次刷新。".to_string());
                                        feed_url.set(String::new());
                                        reload_tick += 1;
                                    }
                                    Err(err) => status.set(format!("保存订阅失败：{err}")),
                                },
                                Err(err) => status.set(format!("初始化应用失败：{err}")),
                            }
                        });
                    },
                    "添加订阅"
                }
                button {
                    class: "button secondary",
                    onclick: move |_| {
                        let mut status = status;
                        let mut reload_tick = reload_tick;
                        spawn(async move {
                            match AppServices::shared().await {
                                Ok(services) => match services.refresh_all().await {
                                    Ok(()) => {
                                        status.set("刷新完成。".to_string());
                                        reload_tick += 1;
                                    }
                                    Err(err) => status.set(format!("刷新失败：{err}")),
                                },
                                Err(err) => status.set(format!("初始化应用失败：{err}")),
                            }
                        });
                    },
                    "刷新全部"
                }
            }
            if feeds().is_empty() {
                StatusBanner { message: "还没有订阅，先添加一个 feed URL。".to_string(), tone: "info".to_string() }
            } else {
                ul { class: "feed-list",
                    for feed in feeds() {
                        li { class: "feed-card", key: "{feed.id}",
                            div { class: "feed-card__title", "{feed.title}" }
                            div { class: "feed-card__meta", "未读 {feed.unread_count}" }
                        }
                    }
                }
            }
        }
    }
}
