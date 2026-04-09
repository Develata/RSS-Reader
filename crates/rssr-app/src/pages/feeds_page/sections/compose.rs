use dioxus::prelude::*;

use crate::pages::feeds_page::session::FeedsPageSession;

#[component]
pub(crate) fn FeedComposeSection(session: FeedsPageSession) -> Element {
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
                        value: "{session.feed_url()}",
                        placeholder: "https://example.com/feed.xml",
                        onkeydown: move |event| {
                            if !is_paste_shortcut(&event) {
                                return;
                            }

                            event.prevent_default();
                            spawn(async move {
                                session.paste_feed_url_result(paste_feed_url_from_clipboard().await);
                            });
                        },
                        oninput: move |event| session.set_feed_url(event.value())
                    }
                    button {
                        class: "button",
                        "data-action": "add-feed",
                        onclick: move |_| session.add_feed(),
                        "添加订阅"
                    }
                    button {
                        class: "button secondary",
                        "data-action": "refresh-all",
                        onclick: move |_| session.refresh_all(),
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

async fn paste_feed_url_from_clipboard() -> Result<Option<String>, String> {
    document::eval(
        r#"
        if (typeof navigator === "undefined" || !navigator.clipboard || !navigator.clipboard.readText) {
            return null;
        }
        return navigator.clipboard.readText();
        "#,
    )
    .join::<Option<String>>()
    .await
    .map_err(|err| err.to_string())
}
