use dioxus::prelude::*;

use super::{
    settings_page_appearance::AppearanceSettingsCard, settings_page_sync::WebDavSettingsCard,
    settings_page_themes::detect_preset_key,
};
use crate::{
    app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation, status::set_status_error,
    theme::ThemeController,
};

const REPOSITORY_URL: &str = "https://github.com/Develata/RSS-Reader";

#[component]
pub fn SettingsPage() -> Element {
    use_mobile_back_navigation(Some(crate::router::AppRoute::EntriesPage {}));

    let theme = use_context::<ThemeController>();
    let mut draft = use_signal(|| (theme.settings)());
    let mut preset_choice =
        use_signal(|| detect_preset_key(&(theme.settings)().custom_css).to_string());
    let endpoint = use_signal(String::new);
    let remote_path = use_signal(|| "config/rss-reader.json".to_string());
    let status = use_signal(String::new);
    let status_tone = use_signal(|| "info".to_string());

    use_resource(move || async move {
        match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(settings) => {
                    preset_choice.set(detect_preset_key(&settings.custom_css).to_string());
                    draft.set(settings);
                }
                Err(err) => set_status_error(status, status_tone, format!("读取设置失败：{err}")),
            },
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });

    rsx! {
        section { class: "page page-settings", "data-page": "settings",
            AppNav {}
            div { class: "page-header",
                h2 { "设置" }
                button {
                    class: "icon-link-button",
                    "data-action": "open-github-repo",
                    r#type: "button",
                    aria_label: "打开项目 GitHub 仓库",
                    title: "打开项目 GitHub 仓库",
                    onclick: move |_| {
                        if let Err(err) = open_repository_url() {
                            set_status_error(status, status_tone, format!("打开 GitHub 仓库失败：{err}"));
                        }
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        view_box: "0 0 24 24",
                        width: "18",
                        height: "18",
                        "aria-hidden": "true",
                        fill: "currentColor",
                        path {
                            d: "M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.009-.866-.014-1.7-2.782.605-3.369-1.344-3.369-1.344-.455-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.071 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.091-.647.349-1.088.635-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.027A9.564 9.564 0 0 1 12 6.844c.85.004 1.705.115 2.504.337 1.909-1.297 2.748-1.027 2.748-1.027.546 1.378.203 2.397.1 2.65.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.31.678.921.678 1.857 0 1.34-.012 2.422-.012 2.75 0 .268.18.58.688.481A10.02 10.02 0 0 0 22 12.017C22 6.484 17.523 2 12 2z"
                        }
                    }
                }
            }
            StatusBanner { message: status(), tone: status_tone() }
            div { class: "settings-grid",
                AppearanceSettingsCard {
                    theme,
                    draft,
                    preset_choice,
                    status,
                    status_tone,
                }
                WebDavSettingsCard {
                    theme,
                    draft,
                    preset_choice,
                    endpoint,
                    remote_path,
                    status,
                    status_tone,
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn open_repository_url() -> Result<(), String> {
    webbrowser::open(REPOSITORY_URL).map(|_| ()).map_err(|err| err.to_string())
}

#[cfg(target_arch = "wasm32")]
fn open_repository_url() -> Result<(), String> {
    web_sys::window()
        .ok_or_else(|| "浏览器窗口不可用".to_string())?
        .open_with_url_and_target(REPOSITORY_URL, "_blank")
        .map(|_| ())
        .map_err(|err| format!("{err:?}"))
}
