use dioxus::prelude::*;

use crate::{
    bootstrap::AppServices,
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
                    label { class: "sr-only", r#for: "feed-url-input", "订阅地址" }
                    input {
                        id: "feed-url-input",
                        name: "feed_url",
                        class: "text-input",
                        "data-action": "feed-url-input",
                        value: "{feed_url}",
                        placeholder: "https://example.com/feed.xml",
                        oninput: move |event| feed_url.set(event.value())
                    }
                    button {
                        class: "button",
                        "data-action": "add-feed",
                        onclick: move |_| add_feed(feed_url, reload_tick, status, status_tone),
                        "添加订阅"
                    }
                    button {
                        class: "button secondary",
                        "data-action": "refresh-all",
                        onclick: move |_| refresh_all(reload_tick, status, status_tone),
                        "刷新全部"
                    }
                }
            }
        }
    }
}

fn add_feed(
    mut feed_url: Signal<String>,
    mut reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) {
    let url = feed_url();
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
                        set_status_error(
                            status,
                            status_tone,
                            format!("订阅已保存，但首次刷新失败：{err}"),
                        );
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
}

fn refresh_all(mut reload_tick: Signal<u64>, status: Signal<String>, status_tone: Signal<String>) {
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
}
