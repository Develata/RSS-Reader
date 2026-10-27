mod edit;
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
    hooks::use_mobile_back_navigation::use_mobile_back_navigation_with_dismiss, router::AppRoute,
    ui::use_reactive_task,
};

#[component]
pub fn FeedsPage() -> Element {
    let facade = use_feeds_page_workspace();
    let session = facade.session();
    use_mobile_back_navigation_with_dismiss(Some(AppRoute::EntriesPage {}), move || {
        session.dismiss_edit()
    });

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
                div { "data-layout": "stat-card", "data-stat": "unread",
                    div { "data-slot": "stat-card-label", "未读" }
                    div { "data-slot": "stat-card-value", "{facade.unread_count()}" }
                }
                div { "data-layout": "stat-card", "data-stat": "archived",
                    div { "data-slot": "stat-card-label", "已归档" }
                    div { "data-slot": "stat-card-value", "{facade.archived_count()}" }
                }
            }
            StatusBanner { message: facade.status_message().to_string(), tone: facade.status_tone().to_string(), announce: true }
            FeedComposeSection { facade: facade.clone() }
            SavedFeedsSection { facade: facade.clone() }
            ConfigExchangeSection { facade: facade.clone() }
            if facade.edit_open() { edit::FeedEditDialog { facade } }
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
