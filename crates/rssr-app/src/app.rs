use dioxus::prelude::*;

use crate::{
    components::{app_notices::AppNotices, status_banner::StatusBanner},
    router::RoutableApp,
    theme::{theme_class, use_theme_controller},
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
    "\n",
    include_str!("../../../assets/styles/reading-workspace.css"),
);

#[component]
#[allow(non_snake_case)]
pub fn App() -> Element {
    crate::ui::reading_position::use_reading_positions();
    let theme = use_theme_controller();
    let shell = use_app_shell_state();
    use_context_provider(|| theme);
    use_context_provider(|| shell);

    use_authenticated_shell_bus(shell, theme);
    use_reread_triggers(shell);

    // A settings-page preview only overrides presentation, never the stored settings signal.
    let current_settings = theme.appearance();
    // Inline style text is already decoded: its parser treats a leading file BOM as part of
    // the first selector. Remove that one signature only for rendering, preserving the draft,
    // export and stored text (including interior BOMs and original line endings).
    let custom_css = current_settings
        .custom_css
        .strip_prefix('\u{feff}')
        .unwrap_or(&current_settings.custom_css);

    rsx! {
        document::Title { "{APP_NAME}" }
        document::Meta {
            name: "viewport",
            content: "width=device-width, initial-scale=1, viewport-fit=cover"
        }
        style { {APP_STYLESHEET} }
        if !custom_css.trim().is_empty() {
            style { id: "user-custom-css", "{custom_css}" }
        }
        div {
            class: "app-shell {theme_class(current_settings.theme)}",
            // Compatibility selector through 0.2.x; density is no longer a preference.
            "data-density": "comfortable",
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
