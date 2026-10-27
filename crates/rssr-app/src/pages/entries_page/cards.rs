use std::sync::Arc;

use dioxus::prelude::*;
use rssr_domain::EntrySummary;

use crate::{components::icon::Icon, datetime::format_date, router::AppRoute};

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

    let selected = facade.is_selected(entry.id);
    let write_pending = facade.entry_write_pending(entry.id);
    render_entry_card(
        entry,
        facade.session(),
        list_edge_state(position, total),
        selected,
        facade.bulk_busy(),
        write_pending,
    )
}

fn render_entry_card(
    entry: Arc<EntrySummary>,
    session: EntriesPageSession,
    list_edge: &'static str,
    selected: bool,
    busy: bool,
    write_pending: bool,
) -> Element {
    let read_title = entry.title.clone();
    let starred_title = entry.title.clone();
    let read_entry = Arc::clone(&entry);
    let starred_entry = Arc::clone(&entry);
    let id = entry.id;
    let read_icon = if entry.is_read { "check" } else { "circle" };

    rsx! {
        li {
            key: "{entry.id}",
            id: super::groups::entry_anchor_id(entry.id),
            "data-layout": "entry-card",
            "data-position-entry": "{entry.id}",
            "data-variant": "reading",
            "data-list-edge": "{list_edge}",
            "data-selected": selected,
            "data-read": entry.is_read,
            label { class: "entry-select-hit", title: "选择文章",
                input { r#type: "checkbox", class: "entry-select-box", aria_label: "选择文章：{entry.title}",
                    "data-action": "select-entry", checked: selected, disabled: busy,
                    onchange: move |_| session.dispatch(super::intent::EntriesPageIntent::ToggleSelected(id)),
                }
            }
            div { class: "entry-card-copy",
              Link {
                "data-slot": "entry-card-title",
                to: AppRoute::ReaderPage { entry_id: entry.id },
                "{entry.title}"
              }
            div { "data-slot": "entry-card-meta",
                "{entry.feed_title}"
                if let Some(date) = format_date(entry.published_at) { " · {date}" }
              }
            }
            div { "data-layout": "entry-card-actions", aria_busy: write_pending,
                button {
                    class: "workspace-icon-button",
                    "data-variant": "secondary",
                    "data-slot": "entry-card-action",
                    "data-action": "mark-read",
                    aria_label: if entry.is_read { "标为未读" } else { "标为已读" },
                    title: if entry.is_read { "标为未读" } else { "标为已读" },
                    aria_pressed: entry.is_read,
                    disabled: busy || write_pending,
                    onclick: move |_| {
                        session.toggle_read(read_entry.id, read_title.clone(), read_entry.is_read);
                    },
                    Icon { name: read_icon }
                }
                button {
                    class: "workspace-icon-button",
                    "data-variant": "secondary",
                    "data-slot": "entry-card-action",
                    "data-action": "toggle-starred",
                    aria_label: if entry.is_starred { "取消收藏" } else { "收藏文章" },
                    title: if entry.is_starred { "取消收藏" } else { "收藏文章" },
                    aria_pressed: entry.is_starred,
                    disabled: busy || write_pending,
                    onclick: move |_| {
                        session.toggle_starred(
                            starred_entry.id,
                            starred_title.clone(),
                            starred_entry.is_starred,
                        );
                    },
                    Icon { name: "star", filled: entry.is_starred }
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
