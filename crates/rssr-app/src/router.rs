use dioxus::prelude::*;
use dioxus_router::components::Router;

use crate::pages::{feeds::FeedsPage, home::HomePage, settings::SettingsPage};

#[derive(Debug, Clone, dioxus_router::prelude::Routable, PartialEq)]
pub enum AppRoute {
    #[route("/", HomePage)]
    HomePage {},
    #[route("/feeds", FeedsPage)]
    FeedsPage {},
    #[route("/settings", SettingsPage)]
    SettingsPage {},
}

#[component]
pub fn RoutableApp() -> Element {
    rsx! {
        Router::<AppRoute> {}
    }
}
