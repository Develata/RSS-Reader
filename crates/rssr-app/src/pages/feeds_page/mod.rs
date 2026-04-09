mod commands;
mod effect;
pub(crate) mod intent;
mod reducer;
mod runtime;
mod sections;
mod session;
mod state;

use dioxus::prelude::*;

use self::sections::{ConfigExchangeSection, FeedComposeSection, SavedFeedsSection};
use self::session::FeedsPageSession;
use crate::{
    app::AppNav, components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation, router::AppRoute,
};

#[component]
pub fn FeedsPage() -> Element {
    use_mobile_back_navigation(Some(AppRoute::EntriesPage {}));

    let (session, snapshot) = use_feeds_page_workspace();

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
            FeedComposeSection { session }
            ConfigExchangeSection { session }
            SavedFeedsSection { session }
        }
    }
}

fn use_feeds_page_workspace() -> (FeedsPageSession, state::FeedsPageState) {
    let state = use_signal(state::FeedsPageState::new);
    let session = FeedsPageSession::new(state);
    let reload_tick = session.reload_tick();

    use_resource(use_reactive!(|(reload_tick)| async move {
        let _ = reload_tick;
        session.load_snapshot();
    }));

    (session, session.snapshot())
}
