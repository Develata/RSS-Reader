mod lab;
mod presets;
mod theme_apply;
mod theme_io;
mod theme_preset;
mod theme_validation;

use dioxus::prelude::*;
use rssr_domain::UserSettings;

use crate::{
    bootstrap::AppServices,
    status::{set_status_error, set_status_info},
    theme::ThemeController,
};

use self::{
    lab::ThemeLabSection, presets::ThemePresetSections, theme_validation::validate_custom_css,
};

pub(crate) use self::theme_preset::detect_preset_key;

#[component]
pub(crate) fn ThemeSettingsSections(
    theme: ThemeController,
    draft: Signal<UserSettings>,
    preset_choice: Signal<String>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Element {
    rsx! {
        ThemeLabSection {
            theme,
            draft,
            preset_choice,
            status,
            status_tone,
        }
        ThemePresetSections {
            theme,
            draft,
            preset_choice,
            status,
            status_tone,
        }
        div { class: "settings-card__footer",
            button {
                class: "button",
                "data-action": "save-settings",
                onclick: move |_| {
                    let next = draft();
                    if let Err(err) = validate_custom_css(&next.custom_css) {
                        set_status_error(status, status_tone, format!("自定义 CSS 格式无效：{err}"));
                        return;
                    }
                    spawn(async move {
                        match AppServices::shared().await {
                            Ok(services) => match services.save_settings(&next).await {
                                Ok(()) => {
                                    theme.settings.set(next);
                                    set_status_info(status, status_tone, "设置已保存。");
                                }
                                Err(err) => {
                                    set_status_error(status, status_tone, format!("保存设置失败：{err}"))
                                }
                            },
                            Err(err) => {
                                set_status_error(status, status_tone, format!("初始化应用失败：{err}"))
                            }
                        }
                    });
                },
                "保存设置"
            }
        }
    }
}
