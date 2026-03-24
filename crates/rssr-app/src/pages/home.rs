use dioxus::prelude::*;

use crate::app::AppNav;

#[component]
pub fn HomePage() -> Element {
    rsx! {
        section {
            AppNav {}
            h2 { "首页" }
            p { "这里会展示阅读器的默认入口视图。" }
        }
    }
}
