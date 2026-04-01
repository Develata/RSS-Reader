use dioxus::prelude::*;

use crate::pages::{
    entries_page::{FeedEntriesPage, StartupPage},
    feeds_page::FeedsPage,
    reader_page::ReaderPage,
    settings_page::SettingsPage,
};

#[derive(Debug, Clone, Routable, PartialEq)]
pub enum AppRoute {
    #[route("/", StartupPage)]
    #[route("/entries", EntriesPage)]
    EntriesPage {},
    #[route("/feeds", FeedsPage)]
    FeedsPage {},
    #[route("/feeds/:feed_id/entries", FeedEntriesPage)]
    FeedEntriesPage { feed_id: i64 },
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
