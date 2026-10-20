use dioxus::prelude::*;

use crate::{
    components::{app_notices::AppNotices, status_banner::StatusBanner},
    router::RoutableApp,
    theme::{density_state, theme_class, use_theme_controller},
    ui::{use_app_shell_state, use_authenticated_shell_bus, use_reread_triggers},
};

pub(crate) const APP_NAME: &str = "RSS-Reader";
const APP_STYLESHEET: &str = concat!(
    include_str!("../../../assets/styles/tokens.css"),
    "\n",
    include_str!("../../../assets/styles/shell.css"),
    "\n",
    include_str!("../../../assets/styles/workspaces.css"),
    "\n",
    include_str!("../../../assets/styles/entries.css"),
    "\n",
    include_str!("../../../assets/styles/reader.css"),
    "\n",
    include_str!("../../../assets/styles/responsive.css"),
);

#[component]
#[allow(non_snake_case)]
pub fn App() -> Element {
    crate::ui::reading_position::use_reading_positions();
    let theme = use_theme_controller();
    let settings = theme.settings;
    let shell = use_app_shell_state();
    use_context_provider(|| theme);
    use_context_provider(|| shell);

    use_authenticated_shell_bus(shell, theme);
    use_reread_triggers(shell);

    // 每次 `settings()` 都会克隆一整份 UserSettings（含完整 custom_css 文本），只读一次后面复用。
    let current_settings = settings();

    rsx! {
        document::Title { "{APP_NAME}" }
        document::Meta {
            name: "viewport",
            content: "width=device-width, initial-scale=1, viewport-fit=cover"
        }
        style { {APP_STYLESHEET} }
        if !current_settings.custom_css.trim().is_empty() {
            style { id: "user-custom-css", "{current_settings.custom_css}" }
        }
        div {
            class: "app-shell {theme_class(current_settings.theme)}",
            "data-density": "{density_state(current_settings.list_density)}",
            style: "--reader-font-scale: {current_settings.reader_font_scale};",
            if crate::demo::ENABLED {
                aside { "data-slot": "pages-demo-notice", role: "note",
                    StatusBanner {
                        message: crate::demo::NOTICE.to_string(),
                        tone: "info".to_string(),
                    }
                }
            }
            AppNotices {}
            RoutableApp {}
        }
    }
}

pub use crate::components::app_nav::AppNav;
