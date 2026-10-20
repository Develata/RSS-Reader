mod facade;
pub(crate) mod intent;
pub(crate) mod reducer;
mod sections;
mod session;
pub(crate) mod state;

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
        section { "data-page": "feeds",
            AppNav {}
            div { "data-layout": "page-header", "data-slot": "page-section-header", "data-section": "feeds",
                h1 { "data-slot": "page-title", "订阅" }
            }
            div { "data-layout": "stats-grid", "data-layout-variant": "airy",
                div { "data-layout": "stat-card", "data-stat": "feeds",
                    div { "data-slot": "stat-card-label", "订阅数" }
                    div { "data-slot": "stat-card-value", "{facade.total_feed_count()}" }
                }
                div { "data-layout": "stat-card", "data-stat": "entries",
                    div { "data-slot": "stat-card-label", "文章数" }
                    div { "data-slot": "stat-card-value", "{facade.total_entry_count()}" }
                }
            }
            StatusBanner { message: facade.status_message().to_string(), tone: facade.status_tone().to_string(), announce: true }
            FeedComposeSection { facade: facade.clone() }
            SavedFeedsSection { facade: facade.clone() }
            ConfigExchangeSection { facade }
        }
    }
}

fn use_feeds_page_workspace() -> FeedsPageFacade {
    let state = use_signal(state::FeedsPageState::new);
    let shell = use_context::<crate::ui::AppShellState>();
    let theme = use_context::<crate::theme::ThemeController>();
    let query_generation = use_signal(|| 0);
    let session = FeedsPageSession::new(state, shell, theme, query_generation);
    // The page's read-only refresh: its own writes (reload_tick) and the shell's (C4b item 2).
    let library_revision = shell.library_revision();
    let reload_tick = session.reload_tick();

    use_reactive_task((reload_tick, library_revision), move |_| {
        session.load_snapshot();
    });

    FeedsPageFacade::new(session, session.snapshot())
}
