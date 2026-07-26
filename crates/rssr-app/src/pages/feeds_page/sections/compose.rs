use dioxus::prelude::*;

use crate::pages::feeds_page::facade::FeedsPageFacade;

#[component]
pub(crate) fn FeedComposeSection(facade: FeedsPageFacade) -> Element {
    let input_facade = facade.clone();
    let add_facade = facade.clone();

    rsx! {
        div { "data-layout": "feed-workbench-single",
            div { "data-layout": "feed-compose-card",
                div { "data-slot": "feed-compose-card-header",
                    h3 { "data-slot": "card-title", "新增订阅" }
                }
                div { "data-layout": "feed-form",
                    label { class: "sr-only", r#for: "feed-url-input", "订阅地址" }
                    input {
                        id: "feed-url-input",
                        name: "feed_url",
                        class: "text-input",
                        "data-field": "feed-url-input",
                        value: "{facade.feed_url()}",
                        placeholder: "https://example.com/feed.xml",
                        // 这里刻意**不**拦截 Ctrl/Cmd+V：输入框本身的原生粘贴在所有平台都可用。
                        // 之前的做法是先 prevent_default 再走 ClipboardPort 读剪贴板，但桌面端的
                        // ClipboardPort 实现是无条件报错，Firefox 上 navigator.clipboard.readText
                        // 不存在时又会静默返回空——结果原生粘贴被吞掉，用户每次粘贴要么吃一个
                        // 错误横幅，要么什么都没发生。只有 Chromium 系 Web 端这条路径是通的。
                        oninput: move |event| input_facade.set_feed_url(event.value())
                    }
                    button {
                        class: "button",
                        "data-variant": "primary",
                        "data-action": "add-feed",
                        onclick: move |_| add_facade.add_feed(),
                        "添加订阅"
                    }
                    button {
                        class: "button",
                        "data-variant": "secondary",
                        "data-action": "refresh-all",
                        onclick: move |_| facade.refresh_all(),
                        "刷新全部"
                    }
                }
            }
        }
    }
}
