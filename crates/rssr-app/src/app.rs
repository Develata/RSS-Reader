use dioxus::prelude::*;

use crate::{
    bootstrap::AppServices,
    router::{AppRoute, RoutableApp},
    theme::{ThemeController, theme_class},
};

#[component]
#[allow(non_snake_case)]
pub fn App() -> Element {
    let mut settings = use_signal(AppServices::default_settings);
    use_context_provider(|| ThemeController { settings });

    let _ = use_resource(move || async move {
        if let Ok(services) = AppServices::shared().await {
            if let Ok(loaded) = services.load_settings().await {
                settings.set(loaded);
            }
        }
    });

    rsx! {
        style { {include_str!("../../../assets/styles.css")} }
        if !settings().custom_css.trim().is_empty() {
            style { id: "user-custom-css", "{settings().custom_css}" }
        }
        div { class: "app-shell {theme_class(settings().theme)}",
            RoutableApp {}
        }
    }
}

#[component]
pub fn AppNav() -> Element {
    rsx! {
        nav { class: "app-nav-shell",
            div { class: "app-nav",
                Link { class: "app-nav__link", "data-nav": "home", to: AppRoute::HomePage {}, "首页" }
                Link { class: "app-nav__link", "data-nav": "feeds", to: AppRoute::FeedsPage {}, "订阅" }
                Link { class: "app-nav__link", "data-nav": "entries", to: AppRoute::EntriesPage {}, "文章" }
                Link { class: "app-nav__link", "data-nav": "settings", to: AppRoute::SettingsPage {}, "设置" }
            }
        }
    }
}
