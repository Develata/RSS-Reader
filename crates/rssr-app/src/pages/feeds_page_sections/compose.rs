use dioxus::prelude::*;

use crate::pages::feeds_page::{FeedsPageBindings, FeedsPageCommand, execute_feeds_page_command};

#[component]
pub(crate) fn FeedComposeSection(feed_url: Signal<String>, bindings: FeedsPageBindings) -> Element {
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
                        value: "{feed_url}",
                        placeholder: "https://example.com/feed.xml",
                        oninput: move |event| feed_url.set(event.value())
                    }
                    button {
                        class: "button",
                        "data-action": "add-feed",
                        onclick: move |_| {
                            let command = FeedsPageCommand::AddFeed { raw_url: feed_url() };
                            spawn(async move {
                                let outcome = execute_feeds_page_command(command).await;
                                bindings.apply_command_outcome(outcome);
                            });
                        },
                        "添加订阅"
                    }
                    button {
                        class: "button secondary",
                        "data-action": "refresh-all",
                        onclick: move |_| {
                            spawn(async move {
                                let outcome =
                                    execute_feeds_page_command(FeedsPageCommand::RefreshAll).await;
                                bindings.apply_command_outcome(outcome);
                            });
                        },
                        "刷新全部"
                    }
                }
            }
        }
    }
}
