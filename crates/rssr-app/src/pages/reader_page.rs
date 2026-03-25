use dioxus::prelude::*;

use crate::{app::AppNav, bootstrap::AppServices};

#[component]
pub fn ReaderPage(entry_id: i64) -> Element {
    let mut title = use_signal(|| "正在加载…".to_string());
    let mut body = use_signal(String::new);
    let mut source = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let _ = use_resource(move || async move {
        match AppServices::shared().await {
            Ok(services) => match services.get_entry(entry_id).await {
                Ok(Some(entry)) => {
                    title.set(entry.title);
                    body.set(
                        entry
                            .content_text
                            .or(entry.summary)
                            .or(entry.content_html)
                            .unwrap_or_else(|| "暂无正文".to_string()),
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
        article {
            AppNav {}
            h2 { "{title}" }
            p { "来源：{source}" }
            if let Some(message) = error() {
                p { class: "error", "{message}" }
            } else {
                pre { "{body}" }
            }
        }
    }
}
