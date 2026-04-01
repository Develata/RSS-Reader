use dioxus::prelude::*;

#[component]
pub fn EntryFilters(
    search: String,
    unread_only: bool,
    starred_only: bool,
    on_search: EventHandler<String>,
    on_toggle_unread: EventHandler<bool>,
    on_toggle_starred: EventHandler<bool>,
) -> Element {
    rsx! {
        div { class: "entry-filters",
            label {
                class: "sr-only",
                r#for: "entry-search-title",
                "按标题搜索"
            }
            input {
                id: "entry-search-title",
                name: "search_title",
                class: "text-input",
                "data-action": "search-title",
                value: "{search}",
                placeholder: "按标题搜索",
                oninput: move |event| on_search.call(event.value())
            }
            label { class: "entry-filters__toggle",
                input {
                    name: "filter_unread",
                    r#type: "checkbox",
                    "data-action": "filter-unread",
                    checked: unread_only,
                    onchange: move |event| on_toggle_unread.call(event.checked())
                }
                span { "仅未读" }
            }
            label { class: "entry-filters__toggle",
                input {
                    name: "filter_starred",
                    r#type: "checkbox",
                    "data-action": "filter-starred",
                    checked: starred_only,
                    onchange: move |event| on_toggle_starred.call(event.checked())
                }
                span { "仅收藏" }
            }
        }
    }
}
