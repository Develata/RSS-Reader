use dioxus::prelude::*;
use dioxus_router::components::Router;

use crate::pages::{
    entries_page::EntriesPage, feeds_page::FeedsPage, home::HomePage, reader_page::ReaderPage,
    settings_page::SettingsPage,
};

#[derive(Debug, Clone, dioxus_router::prelude::Routable, PartialEq)]
pub enum AppRoute {
    #[route("/", HomePage)]
    HomePage {},
    #[route("/feeds", FeedsPage)]
    FeedsPage {},
    #[route("/entries", EntriesPage)]
    EntriesPage {},
    #[route("/entries/:entry_id", ReaderPage)]
    ReaderPage { entry_id: i64 },
    #[route("/settings", SettingsPage)]
    SettingsPage {},
}

#[component]
pub fn RoutableApp() -> Element {
    rsx! {
        Router::<AppRoute> {}
    }
}
