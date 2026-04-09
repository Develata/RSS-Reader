use dioxus::prelude::*;
use rssr_domain::EntrySummary;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::router::AppRoute;

use super::facade::EntriesPageFacade;

pub(super) fn render_entry_card(
    entry: EntrySummary,
    facade: EntriesPageFacade,
    list_edge: &'static str,
) -> Element {
    let read_title = entry.title.clone();
    let starred_title = entry.title.clone();
    let read_facade = facade.clone();

    rsx! {
        li { class: "entry-card entry-card--reading", key: "{entry.id}", "data-list-edge": "{list_edge}",
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
                    class: "button",
                    "data-variant": "secondary",
                    "data-slot": "entry-card-action",
                    "data-action": "mark-read",
                    onclick: move |_| {
                        read_facade.toggle_read(entry.id, read_title.clone(), entry.is_read);
                    },
                    if entry.is_read { "标未读" } else { "标已读" }
                }
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-slot": "entry-card-action",
                    "data-action": "toggle-starred",
                    onclick: move |_| {
                        facade.toggle_starred(entry.id, starred_title.clone(), entry.is_starred);
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
