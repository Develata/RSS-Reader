use dioxus::prelude::*;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner,
    hooks::use_reader_shortcuts::use_reader_shortcuts,
};

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
    let shortcuts = use_reader_shortcuts(entry_id, is_read, is_starred, reload_tick);

    let _ = use_resource(move || async move {
        let _ = reload_tick();
        match AppServices::shared().await {
            Ok(services) => match services.get_entry(entry_id).await {
                Ok(Some(entry)) => {
                    title.set(entry.title);
                    is_read.set(entry.is_read);
                    is_starred.set(entry.is_starred);
                    match select_reader_body(entry.content_html, entry.content_text, entry.summary)
                    {
                        ReaderBody::Html(html) => {
                            body_html.set(Some(html));
                            body_text.set(String::new());
                        }
                        ReaderBody::Text(text) => {
                            body_text.set(text);
                            body_html.set(None);
                        }
                    }
                    published_at.set(
                        format_reader_datetime_utc(entry.published_at)
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
        article {
            class: "reader-page",
            "data-page": "reader",
            tabindex: 0,
            onkeydown: move |event| shortcuts.call(event),
            AppNav {}
            h2 { "{title}" }
            p { class: "reader-meta", "来源：{source}" }
            p { class: "reader-meta", "发布时间：{published_at}" }
            p { class: "reader-meta", "快捷键：`M` 切换已读，`F` 切换收藏" }
            div { class: "entry-card__actions",
                button {
                    class: "button secondary",
                    "data-action": "mark-read",
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
                    "data-action": "toggle-starred",
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReaderBody {
    Html(String),
    Text(String),
}

fn select_reader_body(
    content_html: Option<String>,
    content_text: Option<String>,
    summary: Option<String>,
) -> ReaderBody {
    if let Some(html) = content_html.as_deref().and_then(sanitize_remote_html) {
        return ReaderBody::Html(html);
    }

    ReaderBody::Text(content_text.or(summary).unwrap_or_else(|| "暂无正文".to_string()))
}

fn sanitize_remote_html(raw: &str) -> Option<String> {
    let sanitized = ammonia::clean(raw);
    let trimmed = sanitized.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn format_reader_datetime_utc(published_at: Option<OffsetDateTime>) -> Option<String> {
    const READER_DATETIME_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day] [hour]:[minute] UTC");

    published_at
        .and_then(|value| value.to_offset(UtcOffset::UTC).format(READER_DATETIME_FORMAT).ok())
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::{ReaderBody, format_reader_datetime_utc, select_reader_body};

    #[test]
    fn reader_prefers_full_html_over_summary_text() {
        let body = select_reader_body(
            Some("<article><p>Full body</p></article>".to_string()),
            Some("Summary teaser".to_string()),
            Some("Summary teaser".to_string()),
        );

        assert_eq!(body, ReaderBody::Html("<article><p>Full body</p></article>".to_string()));
    }

    #[test]
    fn reader_sanitizes_remote_html() {
        let body = select_reader_body(
            Some(r#"<p onclick="alert(1)">Hello</p><script>alert(2)</script>"#.to_string()),
            None,
            None,
        );

        match body {
            ReaderBody::Html(html) => {
                assert!(html.contains("<p>Hello</p>"));
                assert!(!html.contains("onclick"));
                assert!(!html.contains("<script"));
            }
            ReaderBody::Text(_) => panic!("expected html body"),
        }
    }

    #[test]
    fn reader_formats_published_time_in_utc_without_seconds() {
        let published_at =
            OffsetDateTime::parse("2026-03-29T19:45:33+08:00", &time::format_description::well_known::Rfc3339)
                .expect("parse rfc3339");

        assert_eq!(
            format_reader_datetime_utc(Some(published_at)).as_deref(),
            Some("2026-03-29 11:45 UTC")
        );
    }
}
