use dioxus::prelude::*;

use crate::app::AppNav;

#[component]
pub fn SettingsPage() -> Element {
    rsx! {
        section {
            AppNav {}
            h2 { "设置" }
            p { "这里会展示主题、导入导出和配置交换设置。" }
        }
    }
}
