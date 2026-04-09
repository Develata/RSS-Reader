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
    let snapshot = &facade.snapshot;

    rsx! {
        section { class: "page page-feeds", "data-page": "feeds",
            AppNav {}
            div { class: "reading-header reading-header--feeds",
                h2 { "订阅" }
            }
            div { class: "stats-grid stats-grid--airy",
                div { class: "stat-card",
                    div { class: "stat-card__label", "订阅数" }
                    div { class: "stat-card__value", "{snapshot.feed_count}" }
                }
                div { class: "stat-card",
                    div { class: "stat-card__label", "文章数" }
                    div { class: "stat-card__value", "{snapshot.entry_count}" }
                }
            }
            StatusBanner { message: snapshot.status.clone(), tone: snapshot.status_tone.clone() }
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
