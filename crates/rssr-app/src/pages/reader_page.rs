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
    let mut error = use_signal(|| None::<String>);

    let _ = use_resource(move || async move {
        match AppServices::shared().await {
            Ok(services) => match services.get_entry(entry_id).await {
                Ok(Some(entry)) => {
                    title.set(entry.title);
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
