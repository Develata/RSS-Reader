mod facade;
pub(crate) mod intent;
mod reducer;
mod sections;
mod session;
mod state;

use dioxus::prelude::*;

use self::facade::FeedsPageFacade;
use self::sections::{ConfigExchangeSection, FeedComposeSection, SavedFeedsSection};
use self::session::FeedsPageSession;
use crate::{
    app::AppNav, components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation, router::AppRoute,
    ui::use_reactive_task,
};

#[component]
pub fn FeedsPage() -> Element {
    use_mobile_back_navigation(Some(AppRoute::EntriesPage {}));

    let facade = use_feeds_page_workspace();

    rsx! {
        section { class: "page page-feeds", "data-page": "feeds",
            AppNav {}
            div { class: "reading-header page-section-header page-section-header--feeds", "data-slot": "page-section-header",
                h2 { class: "page-title page-section-title", "订阅" }
            }
            div { class: "stats-grid stats-grid--airy", "data-layout": "stats-grid", "data-layout-variant": "airy",
                div { class: "stat-card", "data-layout": "stat-card", "data-stat": "feeds",
                    div { class: "stat-card__label", "data-slot": "stat-card-label", "订阅数" }
                    div { class: "stat-card__value", "data-slot": "stat-card-value", "{facade.total_feed_count()}" }
                }
                div { class: "stat-card", "data-layout": "stat-card", "data-stat": "entries",
                    div { class: "stat-card__label", "data-slot": "stat-card-label", "文章数" }
                    div { class: "stat-card__value", "data-slot": "stat-card-value", "{facade.total_entry_count()}" }
                }
            }
            if facade.has_status_message() {
                StatusBanner { message: facade.status_message().to_string(), tone: facade.status_tone().to_string() }
            }
            FeedComposeSection { facade: facade.clone() }
            ConfigExchangeSection { facade: facade.clone() }
            SavedFeedsSection { facade }
        }
    }
}

fn use_feeds_page_workspace() -> FeedsPageFacade {
    let state = use_signal(state::FeedsPageState::new);
    let session = FeedsPageSession::new(state);
    let reload_tick = session.reload_tick();

    use_reactive_task(reload_tick, move |_| {
        session.load_snapshot();
    });

    FeedsPageFacade::new(session, session.snapshot())
}
