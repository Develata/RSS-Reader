use super::facade::FeedsPageFacade;
use crate::components::{dialog::Dialog, icon::Icon};
use dioxus::prelude::*;

#[component]
pub(super) fn FeedEditDialog(facade: FeedsPageFacade) -> Element {
    let close = facade.clone();
    let cancel = facade.clone();
    let dismiss = facade.clone();
    let name = facade.clone();
    let url = facade.clone();
    let reset = facade.clone();
    let save = facade.clone();
    let details = facade.edit_details();
    let saving = facade.edit_busy() && details.is_some();
    rsx! {
        Dialog { id: "feed-edit-dialog", label: "编辑来源", on_close: move |_| close.edit_close(),
            div { class: "workspace-dialog-heading",
                h2 { "编辑来源" }
                button { class: "workspace-icon-button", aria_label: "关闭来源编辑", disabled: saving, onclick: move |_| dismiss.edit_close(), Icon { name: "close" } }
            }
            if let Some(error) = facade.edit_error() { p { role: "alert", class: "feed-edit-error", "{error}" } }
            if let Some(details) = details {
                form { class: "feed-edit-form", onsubmit: move |event| { event.prevent_default(); save.edit_save(); },
                    label { r#for: "feed-edit-name", "自定义名称" }
                    input { id: "feed-edit-name", class: "text-input", value: details.custom_title.clone().unwrap_or_default(),
                        placeholder: details.original_title.as_deref().unwrap_or("使用来源原名"), maxlength: 200, disabled: saving,
                        oninput: move |event| name.edit_title(event.value()),
                    }
                    div { class: "feed-edit-original",
                        small { "原始名称：{details.original_title.as_deref().unwrap_or(\"尚未获取\")}" }
                        button { r#type: "button", class: "text-button", disabled: saving, onclick: move |_| reset.edit_title(String::new()), "使用原名" }
                    }
                    label { r#for: "feed-edit-url", "RSS / Atom 地址" }
                    input { id: "feed-edit-url", class: "text-input", r#type: "url", required: true, value: details.url.clone(), disabled: saving,
                        oninput: move |event| url.edit_url(event.value()),
                    }
                    p { class: "feed-edit-hint", "更换地址后，已有文章及阅读、收藏状态保留。" }
                    div { class: "workspace-dialog-actions",
                        button { r#type: "button", class: "button", "data-variant": "secondary", disabled: saving, onclick: move |_| cancel.edit_close(), "取消" }
                        button { r#type: "submit", class: "button", "data-action": "save-feed-edit", disabled: saving || details.url.trim().is_empty(), if saving { "正在保存…" } else { "保存" } }
                    }
                }
            } else if facade.edit_busy() { p { role: "status", "正在读取来源…" } }
        }
    }
}
