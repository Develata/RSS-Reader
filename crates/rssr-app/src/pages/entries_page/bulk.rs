use super::{facade::EntriesPageFacade, intent::EntriesPageIntent};
use dioxus::prelude::*;

pub(super) fn render_bulk_controls(facade: &EntriesPageFacade) -> Element {
    let preview_facade = facade.clone();
    let session = facade.session();
    let busy = facade.bulk_busy();
    let count = facade.bulk_preview().map(|p| p.unread_entry_ids.len());
    rsx! {
        div { "data-layout": "entry-bulk-read", "data-state": if count.is_some_and(|count| count > 0) { "confirm" } else { "idle" }, aria_busy: busy,
            if let Some(count) = count.filter(|count| *count > 0) {
                p { "data-slot": "confirm-hint", "将把当前筛选结果中的 {count} 篇未读文章标为已读。" }
                button { class: "button", "data-action": "confirm-mark-filtered-read", disabled: busy, aria_busy: busy,
                    onclick: move |_| session.confirm_mark_read(), "确认标为已读" }
                button { class: "button", "data-action": "cancel-mark-filtered-read", disabled: busy,
                    onclick: move |_| session.dispatch(EntriesPageIntent::CancelBulk), "取消" }
            } else {
                button { class: "button", "data-action": "preview-mark-filtered-read", disabled: busy,
                    onclick: move |_| preview_facade.preview_mark_read(), "将筛选结果标为已读" }
            }
        }
    }
}
