mod lab;
mod presets;
mod theme_apply;
mod theme_file_io;
mod theme_io;
mod theme_preset;

use crate::pages::settings_page::facade::SettingsPageFacade;
use dioxus::prelude::*;

use self::{lab::ThemeLabSection, presets::ThemePresetSections};

pub(crate) use self::theme_preset::{detect_preset_key, preset_css, preset_display_name};
// 校验规则的唯一来源在 domain；页面层只是转发，保证设置页保存与配置包导入用的是同一套判定。
pub(crate) use rssr_domain::validation::validate_custom_css;

#[component]
pub(crate) fn ThemeSettingsSections(facade: SettingsPageFacade) -> Element {
    rsx! {
        ThemeLabSection { facade: facade.clone() }
        ThemePresetSections { facade }
    }
}
