use dioxus::prelude::*;

use crate::router::{AppRoute, RoutableApp};

#[component]
#[allow(non_snake_case)]
pub fn App() -> Element {
    rsx! {
        style { {include_str!("../../../assets/styles.css")} }
        div { class: "app-shell",
            header { class: "app-header",
                p { class: "app-eyebrow", "Local-first RSS" }
                h1 { "RSS Reader" }
                p { class: "app-subtitle", "极简、快速、以本地 SQLite 为事实来源的个人阅读器。" }
            }
            RoutableApp {}
        }
    }
}

#[component]
pub fn AppNav() -> Element {
    rsx! {
        nav { class: "app-nav",
            Link { class: "app-nav__link", to: AppRoute::HomePage {}, "首页" }
            Link { class: "app-nav__link", to: AppRoute::FeedsPage {}, "订阅" }
            Link { class: "app-nav__link", to: AppRoute::EntriesPage {}, "文章" }
            Link { class: "app-nav__link", to: AppRoute::SettingsPage {}, "设置" }
        }
    }
}
