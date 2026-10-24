use super::facade::EntriesPageFacade;
use crate::components::icon::Icon;
use dioxus::prelude::*;
use rssr_domain::EntryBatchAction;

pub(super) fn render_bulk_controls(facade: &EntriesPageFacade) -> Element {
    let session = facade.session();
    let disabled =
        facade.selected_count() == 0 || facade.bulk_busy() || facade.has_pending_entry_writes();
    rsx! {
        div { class: "entry-bulk-actions", role: "group", aria_label: "所选文章操作", aria_busy: facade.bulk_busy(),
            button { class: "workspace-icon-button", disabled, title: "将所选文章标为已读", aria_label: "将所选文章标为已读", "data-action": "mark-selected-read",
                onclick: move |_| session.apply_selection(EntryBatchAction::MarkRead), Icon { name: "double-check" } }
            button { class: "workspace-icon-button", disabled, title: "归档所选文章", aria_label: "归档所选文章", "data-action": "archive-selected",
                onclick: move |_| session.apply_selection(EntryBatchAction::Archive), Icon { name: "archive" } }
        }
    }
}
