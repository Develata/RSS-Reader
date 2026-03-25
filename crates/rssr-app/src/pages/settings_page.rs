use dioxus::prelude::*;

use crate::{app::AppNav, components::status_banner::StatusBanner};

#[component]
pub fn SettingsPage() -> Element {
    rsx! {
        section {
            AppNav {}
            h2 { "设置" }
            StatusBanner {
                message: "主题、导入导出和配置交换入口会在 US3 接入。".to_string(),
                tone: "info".to_string(),
            }
        }
    }
}
