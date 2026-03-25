use dioxus::prelude::*;
use time::format_description::well_known::Rfc3339;

use crate::{app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner};

#[component]
pub fn ReaderPage(entry_id: i64) -> Element {
    let mut title = use_signal(|| "正在加载…".to_string());
    let mut body_text = use_signal(String::new);
    let mut body_html = use_signal(|| None::<String>);
    let mut source = use_signal(String::new);
    let mut published_at = use_signal(|| "未知发布时间".to_string());
    let mut is_read = use_signal(|| false);
    let mut is_starred = use_signal(|| false);
    let reload_tick = use_signal(|| 0_u64);
    let mut error = use_signal(|| None::<String>);

    let _ = use_resource(move || async move {
        let _ = reload_tick();
        match AppServices::shared().await {
            Ok(services) => match services.get_entry(entry_id).await {
                Ok(Some(entry)) => {
                    title.set(entry.title);
                    is_read.set(entry.is_read);
                    is_starred.set(entry.is_starred);
                    match entry.content_text.or(entry.summary) {
                        Some(text) => {
                            body_text.set(text);
                            body_html.set(None);
                        }
                        None => {
                            body_html.set(entry.content_html);
                            body_text.set("暂无正文".to_string());
                        }
                    }
                    published_at.set(
                        entry
                            .published_at
                            .and_then(|value| value.format(&Rfc3339).ok())
                            .unwrap_or_else(|| "未知发布时间".to_string()),
                    );
                    source.set(
                        entry
                            .url
                            .map(|url| url.to_string())
                            .unwrap_or_else(|| "无原文链接".to_string()),
                    );
                }
                Ok(None) => error.set(Some("文章不存在".to_string())),
                Err(err) => error.set(Some(err.to_string())),
            },
            Err(err) => error.set(Some(err.to_string())),
        }
    });

    rsx! {
        article { class: "reader-page",
            AppNav {}
            h2 { "{title}" }
            p { class: "reader-meta", "来源：{source}" }
            p { class: "reader-meta", "发布时间：{published_at}" }
            div { class: "entry-card__actions",
                button {
                    class: "button secondary",
                    onclick: move |_| {
                        let mut reload_tick = reload_tick;
                        spawn(async move {
                            if let Ok(services) = AppServices::shared().await {
                                let _ = services.set_read(entry_id, !is_read()).await;
                                reload_tick += 1;
                            }
                        });
                    },
                    if is_read() { "标记为未读" } else { "标记为已读" }
                }
                button {
                    class: "button secondary",
                    onclick: move |_| {
                        let mut reload_tick = reload_tick;
                        spawn(async move {
                            if let Ok(services) = AppServices::shared().await {
                                let _ = services.set_starred(entry_id, !is_starred()).await;
                                reload_tick += 1;
                            }
                        });
                    },
                    if is_starred() { "取消收藏" } else { "收藏文章" }
                }
            }
            if let Some(message) = error() {
                StatusBanner { message, tone: "error".to_string() }
            } else {
                div { class: "reader-body",
                    if let Some(html) = body_html() {
                        div { class: "reader-html", dangerous_inner_html: "{html}" }
                    } else {
                        pre { "{body_text}" }
                    }
                }
            }
        }
    }
}
