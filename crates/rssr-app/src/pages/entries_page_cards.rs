use dioxus::prelude::*;
use rssr_domain::EntrySummary;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    bootstrap::AppServices,
    router::AppRoute,
    status::{set_status_error, set_status_info},
};

pub(super) fn render_entry_card(
    entry: EntrySummary,
    reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
    let read_title = entry.title.clone();
    let starred_title = entry.title.clone();

    rsx! {
        li { class: "entry-card entry-card--reading", key: "{entry.id}",
            Link {
                class: "entry-card__title",
                to: AppRoute::ReaderPage { entry_id: entry.id },
                "{entry.title}"
            }
            div { class: "entry-card__meta",
                "{entry.feed_title}"
                if let Some(date) = format_entry_date_utc(entry.published_at) { " · {date}" }
                if entry.is_read { " · 已读" } else { " · 未读" }
                if entry.is_starred { " · 已收藏" }
            }
            div { class: "entry-card__actions",
                button {
                    class: "button secondary",
                    "data-action": "mark-read",
                    onclick: move |_| {
                        let mut reload_tick = reload_tick;
                        let title = read_title.clone();
                        spawn(async move {
                            match AppServices::shared().await {
                                Ok(services) => match services.set_read(entry.id, !entry.is_read).await {
                                    Ok(()) => {
                                        set_status_info(
                                            status,
                                            status_tone,
                                            format!(
                                                "已将《{}》{}。",
                                                title,
                                                if entry.is_read { "标记为未读" } else { "标记为已读" }
                                            ),
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
                    if entry.is_read { "标未读" } else { "标已读" }
                }
                button {
                    class: "button secondary",
                    "data-action": "toggle-starred",
                    onclick: move |_| {
                        let mut reload_tick = reload_tick;
                        let title = starred_title.clone();
                        spawn(async move {
                            match AppServices::shared().await {
                                Ok(services) => match services.set_starred(entry.id, !entry.is_starred).await {
                                    Ok(()) => {
                                        set_status_info(
                                            status,
                                            status_tone,
                                            format!(
                                                "已{}《{}》。",
                                                if entry.is_starred { "取消收藏" } else { "收藏" },
                                                title
                                            ),
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
                    if entry.is_starred { "取消收藏" } else { "收藏" }
                }
            }
        }
    }
}

fn format_entry_date_utc(published_at: Option<OffsetDateTime>) -> Option<String> {
    const ENTRY_DATE_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day]");

    published_at.and_then(|value| value.to_offset(UtcOffset::UTC).format(ENTRY_DATE_FORMAT).ok())
}
