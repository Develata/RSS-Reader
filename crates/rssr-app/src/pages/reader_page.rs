use dioxus::prelude::*;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    app::AppNav,
    bootstrap::{AppServices, ReaderNavigation},
    components::status_banner::StatusBanner,
    hooks::use_reader_shortcuts::use_reader_shortcuts,
    router::AppRoute,
};

#[component]
pub fn ReaderPage(entry_id: i64) -> Element {
    let navigator = use_navigator();
    let mut title = use_signal(|| "正在加载…".to_string());
    let mut body_text = use_signal(String::new);
    let mut body_html = use_signal(|| None::<String>);
    let mut source = use_signal(String::new);
    let mut published_at = use_signal(|| "未知发布时间".to_string());
    let mut navigation_state = use_signal(ReaderNavigation::default);
    let mut is_read = use_signal(|| false);
    let mut is_starred = use_signal(|| false);
    let reload_tick = use_signal(|| 0_u64);
    let mut status = use_signal(String::new);
    let mut status_tone = use_signal(|| "info".to_string());
    let mut error = use_signal(|| None::<String>);
    let shortcuts =
        use_reader_shortcuts(entry_id, is_read, is_starred, reload_tick, status, status_tone);
    let reload_version = reload_tick();

    use_resource(use_reactive!(|(entry_id, reload_version)| async move {
        let _ = reload_version;
        title.set("正在加载…".to_string());
        body_text.set(String::new());
        body_html.set(None);
        source.set(String::new());
        published_at.set("未知发布时间".to_string());
        navigation_state.set(ReaderNavigation::default());
        is_read.set(false);
        is_starred.set(false);
        status.set(String::new());
        status_tone.set("info".to_string());
        error.set(None);

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
                    navigation_state
                        .set(services.reader_navigation(entry_id).await.unwrap_or_default());
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
    }));

    rsx! {
        article {
            class: "reader-page",
            "data-page": "reader",
            tabindex: 0,
            onkeydown: move |event| shortcuts.call(event),
            AppNav {}
            header { class: "reader-header",
                h2 { class: "reader-title", "{title}" }
            }
            div { class: "reader-toolbar inline-actions",
                button {
                    class: "button secondary",
                    "data-nav": "back",
                    onclick: move |_| navigator.go_back(),
                    "返回上一页"
                }
            }
            div { class: "reader-meta-block",
                p { class: "reader-meta", "来源：{source}" }
                p { class: "reader-meta", "发布时间：{published_at}" }
                p { class: "reader-meta", "快捷键：`M` 切换已读，`F` 切换收藏" }
            }
            if let Some(message) = error() {
                StatusBanner { message, tone: "error".to_string() }
            } else {
                if !status().is_empty() {
                    StatusBanner { message: status(), tone: status_tone() }
                }
                div { class: "reader-body",
                    if let Some(html) = body_html() {
                        div { class: "reader-html", dangerous_inner_html: "{html}" }
                    } else {
                        pre { "{body_text}" }
                    }
                }
                div { class: "reader-pagination reader-pagination--context inline-actions",
                    if let Some(previous_feed_entry_id) = navigation_state().previous_feed_entry_id {
                        button {
                            class: "button secondary",
                            "data-nav": "previous-feed-entry",
                            onclick: move |_| { navigator.push(AppRoute::ReaderPage { entry_id: previous_feed_entry_id }); },
                            "上一篇同订阅文章"
                        }
                    }
                    if let Some(next_feed_entry_id) = navigation_state().next_feed_entry_id {
                        button {
                            class: "button secondary",
                            "data-nav": "next-feed-entry",
                            onclick: move |_| { navigator.push(AppRoute::ReaderPage { entry_id: next_feed_entry_id }); },
                            "下一篇同订阅文章"
                        }
                    }
                }
                nav { class: "reader-bottom-bar", "aria-label": "阅读快捷操作",
                    button {
                        class: if previous_action_target(navigation_state()).is_some() {
                            "reader-bottom-bar__button"
                        } else {
                            "reader-bottom-bar__button is-disabled"
                        },
                        disabled: previous_action_target(navigation_state()).is_none(),
                        "data-nav": "previous-entry",
                        onclick: move |_| {
                            if let Some(target) = previous_action_target(navigation_state()) {
                                navigator.push(AppRoute::ReaderPage { entry_id: target });
                            }
                        },
                        span { class: "reader-bottom-bar__icon", "‹" }
                        span { class: "reader-bottom-bar__label", "上一篇" }
                    }
                    button {
                        class: "reader-bottom-bar__button",
                        "data-action": "mark-read",
                        onclick: move |_| {
                            let mut reload_tick = reload_tick;
                            spawn(async move {
                                match AppServices::shared().await {
                                    Ok(services) => match services.set_read(entry_id, !is_read()).await {
                                        Ok(()) => {
                                            set_status_info(
                                                status,
                                                status_tone,
                                                if is_read() {
                                                    "已将当前文章标记为未读。".to_string()
                                                } else {
                                                    "已将当前文章标记为已读。".to_string()
                                                },
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
                        span { class: "reader-bottom-bar__icon", if is_read() { "○" } else { "✓" } }
                        span { class: "reader-bottom-bar__label", if is_read() { "未读" } else { "已读" } }
                    }
                    button {
                        class: if is_starred() {
                            "reader-bottom-bar__button is-active"
                        } else {
                            "reader-bottom-bar__button"
                        },
                        "data-action": "toggle-starred",
                        onclick: move |_| {
                            let mut reload_tick = reload_tick;
                            spawn(async move {
                                match AppServices::shared().await {
                                    Ok(services) => match services.set_starred(entry_id, !is_starred()).await {
                                        Ok(()) => {
                                            set_status_info(
                                                status,
                                                status_tone,
                                                if is_starred() {
                                                    "已取消收藏当前文章。".to_string()
                                                } else {
                                                    "已收藏当前文章。".to_string()
                                                },
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
                        span { class: "reader-bottom-bar__icon", if is_starred() { "★" } else { "☆" } }
                        span { class: "reader-bottom-bar__label", "收藏" }
                    }
                    button {
                        class: if next_action_target(navigation_state()).is_some() {
                            "reader-bottom-bar__button"
                        } else {
                            "reader-bottom-bar__button is-disabled"
                        },
                        disabled: next_action_target(navigation_state()).is_none(),
                        "data-nav": "next-entry",
                        onclick: move |_| {
                            if let Some(target) = next_action_target(navigation_state()) {
                                navigator.push(AppRoute::ReaderPage { entry_id: target });
                            }
                        },
                        span { class: "reader-bottom-bar__icon", "›" }
                        span { class: "reader-bottom-bar__label", "下一篇" }
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

fn previous_action_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.previous_unread_entry_id.or(navigation.previous_feed_entry_id)
}

fn next_action_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.next_unread_entry_id.or(navigation.next_feed_entry_id)
}

fn set_status_info(mut status: Signal<String>, mut status_tone: Signal<String>, message: String) {
    status.set(message);
    status_tone.set("info".to_string());
}

fn set_status_error(mut status: Signal<String>, mut status_tone: Signal<String>, message: String) {
    status.set(message);
    status_tone.set("error".to_string());
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
        let published_at = OffsetDateTime::parse(
            "2026-03-29T19:45:33+08:00",
            &time::format_description::well_known::Rfc3339,
        )
        .expect("parse rfc3339");

        assert_eq!(
            format_reader_datetime_utc(Some(published_at)).as_deref(),
            Some("2026-03-29 11:45 UTC")
        );
    }
}
