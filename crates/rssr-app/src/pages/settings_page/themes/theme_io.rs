#[cfg(not(target_arch = "wasm32"))]
use dioxus::prelude::spawn;

use crate::pages::settings_page::facade::SettingsPageFacade;

#[cfg(not(target_arch = "wasm32"))]
use super::theme_file_io::{pick_css_file_contents, save_css_file};

#[cfg(target_arch = "wasm32")]
use super::theme_file_io::save_css_file_in_browser;

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn import_css_file(facade: &SettingsPageFacade) {
    use super::theme_apply::apply_custom_css_from_raw;

    let facade = facade.clone();
    spawn(async move {
        match pick_css_file_contents().await {
            Ok(Some(raw)) => {
                apply_custom_css_from_raw(&facade, raw, "已从文件载入并应用自定义 CSS。")
            }
            Ok(None) => facade.set_status("已取消载入 CSS 文件。", "info"),
            Err(err) => facade.set_status(format!("载入 CSS 文件失败：{err}"), "error"),
        }
    });
}

pub(super) fn export_css_file(raw: String, facade: &SettingsPageFacade) {
    if raw.trim().is_empty() {
        facade.set_status("当前没有可导出的自定义 CSS。", "info");
        return;
    }

    #[cfg(target_arch = "wasm32")]
    {
        match save_css_file_in_browser(&raw) {
            Ok(()) => facade.set_status("已导出当前自定义 CSS。", "info"),
            Err(err) => facade.set_status(format!("导出 CSS 文件失败：{err}"), "error"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let facade = facade.clone();
        spawn(async move {
            match save_css_file(&raw).await {
                Ok(true) => facade.set_status("已导出当前自定义 CSS。", "info"),
                Ok(false) => facade.set_status("已取消导出 CSS 文件。", "info"),
                Err(err) => facade.set_status(format!("导出 CSS 文件失败：{err}"), "error"),
            }
        });
    }
}
