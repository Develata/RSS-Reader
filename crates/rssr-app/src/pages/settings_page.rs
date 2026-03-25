use dioxus::prelude::*;

use crate::app::AppNav;

#[component]
pub fn SettingsPage() -> Element {
    rsx! {
        section {
            AppNav {}
            h2 { "设置" }
            p { "主题、导入导出和配置交换入口会在 US3 接入。" }
        }
    }
}
