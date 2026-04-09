use dioxus::prelude::*;

use crate::pages::feeds_page::facade::FeedsPageFacade;

#[component]
pub(crate) fn FeedComposeSection(facade: FeedsPageFacade) -> Element {
    let paste_facade = facade.clone();
    let input_facade = facade.clone();
    let add_facade = facade.clone();

    rsx! {
        div { class: "feed-workbench feed-workbench--single",
            div { class: "feed-compose-card",
                div { class: "feed-compose-card__header",
                    h3 { "新增订阅" }
                }
                div { class: "feed-form",
                    label { class: "sr-only", r#for: "feed-url-input", "订阅地址" }
                    input {
                        id: "feed-url-input",
                        name: "feed_url",
                        class: "text-input",
                        "data-field": "feed-url-input",
                        value: "{facade.feed_url()}",
                        placeholder: "https://example.com/feed.xml",
                        onkeydown: move |event| {
                            if !is_paste_shortcut(&event) {
                                return;
                            }

                            event.prevent_default();
                            paste_facade.paste_feed_url();
                        },
                        oninput: move |event| input_facade.set_feed_url(event.value())
                    }
                    button {
                        class: "button",
                        "data-action": "add-feed",
                        onclick: move |_| add_facade.add_feed(),
                        "添加订阅"
                    }
                    button {
                        class: "button secondary",
                        "data-action": "refresh-all",
                        onclick: move |_| facade.refresh_all(),
                        "刷新全部"
                    }
                }
            }
        }
    }
}

fn is_paste_shortcut(event: &KeyboardEvent) -> bool {
    let modifiers = event.modifiers();
    let has_paste_modifier =
        modifiers.contains(Modifiers::META) || modifiers.contains(Modifiers::CONTROL);
    has_paste_modifier && event.key().to_string().eq_ignore_ascii_case("v")
}
