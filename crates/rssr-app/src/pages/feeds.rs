use dioxus::prelude::*;

use crate::app::AppNav;

#[component]
pub fn FeedsPage() -> Element {
    rsx! {
        section {
            AppNav {}
            h2 { "订阅" }
            p { "这里会展示订阅列表和文章列表。" }
        }
    }
}
