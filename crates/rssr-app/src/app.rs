use dioxus::prelude::*;

use crate::router::{AppRoute, RoutableApp};

#[component]
#[allow(non_snake_case)]
pub fn App() -> Element {
    rsx! {
        div { class: "app-shell",
            h1 { "RSS Reader" }
            p { "极简个人 RSS 阅读器项目骨架" }
            RoutableApp {}
        }
    }
}

#[component]
pub fn AppNav() -> Element {
    rsx! {
        nav {
            Link { to: AppRoute::HomePage {}, "首页" }
            " · "
            Link { to: AppRoute::FeedsPage {}, "订阅" }
            " · "
            Link { to: AppRoute::SettingsPage {}, "设置" }
        }
    }
}
