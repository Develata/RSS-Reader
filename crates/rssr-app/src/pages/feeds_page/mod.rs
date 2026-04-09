mod bindings;
mod commands;
mod dispatch;
mod queries;
mod sections;
mod session;

use dioxus::prelude::*;

use self::sections::{ConfigExchangeSection, FeedComposeSection, SavedFeedsSection};
use self::session::FeedsPageSession;
pub(crate) use self::{
    bindings::FeedsPageBindings, commands::FeedsPageCommand,
    dispatch::execute_command as execute_feeds_page_command,
};
use crate::{
    app::AppNav, components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation, router::AppRoute,
};

#[component]
pub fn FeedsPage() -> Element {
    use_mobile_back_navigation(Some(AppRoute::EntriesPage {}));

    let reload_tick = use_signal(|| 0_u64);
    let session = FeedsPageSession::new(reload_tick);

    use_resource(move || async move {
        let _ = reload_tick();
        session.load_snapshot().await;
    });

    rsx! {
        section { class: "page page-feeds", "data-page": "feeds",
            AppNav {}
            div { class: "reading-header reading-header--feeds",
                h2 { "订阅" }
            }
            div { class: "stats-grid stats-grid--airy",
                div { class: "stat-card",
                    div { class: "stat-card__label", "订阅数" }
                    div { class: "stat-card__value", "{session.feed_count()}" }
                }
                div { class: "stat-card",
                    div { class: "stat-card__label", "文章数" }
                    div { class: "stat-card__value", "{session.entry_count()}" }
                }
            }
            StatusBanner { message: session.status(), tone: session.status_tone() }
            FeedComposeSection { feed_url: session.feed_url(), bindings: session.bindings() }
            ConfigExchangeSection {
                config_text: session.config_text(),
                opml_text: session.opml_text(),
                pending_config_import: session.pending_config_import(),
                bindings: session.bindings(),
            }
            SavedFeedsSection {
                feeds: session.feeds(),
                pending_delete_feed: session.pending_delete_feed(),
                bindings: session.bindings(),
            }
        }
    }
}
