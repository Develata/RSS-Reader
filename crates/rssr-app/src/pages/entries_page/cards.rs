use std::sync::Arc;

use dioxus::prelude::*;
use rssr_domain::EntrySummary;

use crate::{datetime::format_date_utc, router::AppRoute};

use super::{facade::EntriesPageFacade, groups::EntryCardRef, session::EntriesPageSession};

/// 按分组树里的引用渲染一张卡片。
///
/// 条目在渲染时才从最新状态解析，因此分组树只需要携带 `(下标, id)`，不必携带
/// `title` / `is_read` / `is_starred`——这也是切换已读/收藏不再重建分组树的原因。
pub(super) fn render_entry_card_at(
    facade: &EntriesPageFacade,
    card: EntryCardRef,
    position: usize,
    total: usize,
) -> Element {
    let Some(entry) = facade.entry_at(card) else {
        // 条目已不在当前列表里（罕见，见 `entry_at`）：少渲染一张卡片，不要让整页 panic。
        return rsx! {};
    };

    render_entry_card(entry, facade.session(), list_edge_state(position, total))
}

fn render_entry_card(
    entry: Arc<EntrySummary>,
    session: EntriesPageSession,
    list_edge: &'static str,
) -> Element {
    let read_title = entry.title.clone();
    let starred_title = entry.title.clone();
    let read_entry = Arc::clone(&entry);
    let starred_entry = Arc::clone(&entry);

    rsx! {
        li {
            key: "{entry.id}",
            "data-layout": "entry-card",
            "data-variant": "reading",
            "data-list-edge": "{list_edge}",
            Link {
                "data-slot": "entry-card-title",
                to: AppRoute::ReaderPage { entry_id: entry.id },
                "{entry.title}"
            }
            div { "data-slot": "entry-card-meta",
                "{entry.feed_title}"
                if let Some(date) = format_date_utc(entry.published_at) { " · {date}" }
                if entry.is_read { " · 已读" } else { " · 未读" }
                if entry.is_starred { " · 已收藏" }
            }
            div { "data-layout": "entry-card-actions",
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-slot": "entry-card-action",
                    "data-action": "mark-read",
                    onclick: move |_| {
                        session.toggle_read(read_entry.id, read_title.clone(), read_entry.is_read);
                    },
                    if entry.is_read { "标未读" } else { "标已读" }
                }
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-slot": "entry-card-action",
                    "data-action": "toggle-starred",
                    onclick: move |_| {
                        session.toggle_starred(
                            starred_entry.id,
                            starred_title.clone(),
                            starred_entry.is_starred,
                        );
                    },
                    if entry.is_starred { "取消收藏" } else { "收藏" }
                }
            }
        }
    }
}

fn list_edge_state(position: usize, total: usize) -> &'static str {
    match (position, total) {
        (_, 0) => "single",
        (0, 1) => "single",
        (0, _) => "start",
        (index, len) if index + 1 == len => "end",
        _ => "middle",
    }
}

#[cfg(test)]
mod tests {
    use super::list_edge_state;

    #[test]
    fn marks_list_edges_by_position() {
        assert_eq!(list_edge_state(0, 0), "single");
        assert_eq!(list_edge_state(0, 1), "single");
        assert_eq!(list_edge_state(0, 3), "start");
        assert_eq!(list_edge_state(1, 3), "middle");
        assert_eq!(list_edge_state(2, 3), "end");
    }
}
