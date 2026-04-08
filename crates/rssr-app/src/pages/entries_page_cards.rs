use dioxus::prelude::*;
use rssr_domain::EntrySummary;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    pages::entries_page::{EntriesPageBindings, EntriesPageCommand, execute_entries_page_command},
    router::AppRoute,
};

pub(super) fn render_entry_card(entry: EntrySummary, bindings: EntriesPageBindings) -> Element {
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
                        let command = EntriesPageCommand::ToggleRead {
                            entry_id: entry.id,
                            entry_title: read_title.clone(),
                            currently_read: entry.is_read,
                        };
                        spawn(async move {
                            let outcome = execute_entries_page_command(command).await;
                            bindings.apply_command_outcome(outcome);
                        });
                    },
                    if entry.is_read { "标未读" } else { "标已读" }
                }
                button {
                    class: "button secondary",
                    "data-action": "toggle-starred",
                    onclick: move |_| {
                        let command = EntriesPageCommand::ToggleStarred {
                            entry_id: entry.id,
                            entry_title: starred_title.clone(),
                            currently_starred: entry.is_starred,
                        };
                        spawn(async move {
                            let outcome = execute_entries_page_command(command).await;
                            bindings.apply_command_outcome(outcome);
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
