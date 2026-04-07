mod bindings;
mod commands;
mod dispatch;
mod queries;

use dioxus::prelude::*;
use rssr_domain::FeedSummary;

use self::queries::load_feeds_page_snapshot;
pub(crate) use self::{
    bindings::FeedsPageBindings, commands::FeedsPageCommand,
    dispatch::execute_command as execute_feeds_page_command,
};
use super::feeds_page_sections::{ConfigExchangeSection, FeedComposeSection, SavedFeedsSection};
use crate::{
    app::AppNav, components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation, router::AppRoute,
};

#[component]
pub fn FeedsPage() -> Element {
    use_mobile_back_navigation(Some(AppRoute::EntriesPage {}));

    let feed_url = use_signal(String::new);
    let config_text = use_signal(String::new);
    let opml_text = use_signal(String::new);
    let pending_config_import = use_signal(|| false);
    let pending_delete_feed = use_signal(|| None::<i64>);
    let reload_tick = use_signal(|| 0_u64);
    let feeds = use_signal(Vec::<FeedSummary>::new);
    let feed_count = use_signal(|| 0_usize);
    let entry_count = use_signal(|| 0_usize);
    let status = use_signal(String::new);
    let status_tone = use_signal(|| "info".to_string());
    let bindings = FeedsPageBindings::new(
        feed_url,
        config_text,
        opml_text,
        pending_config_import,
        pending_delete_feed,
        reload_tick,
        feeds,
        feed_count,
        entry_count,
        status,
        status_tone,
    );

    use_resource(move || async move {
        let _ = reload_tick();
        match load_feeds_page_snapshot().await {
            Ok(snapshot) => bindings.apply_snapshot(snapshot),
            Err(err) => bindings.set_status_error(err.to_string()),
        }
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
                    div { class: "stat-card__value", "{feed_count}" }
                }
                div { class: "stat-card",
                    div { class: "stat-card__label", "文章数" }
                    div { class: "stat-card__value", "{entry_count}" }
                }
            }
            StatusBanner { message: status(), tone: status_tone() }
            FeedComposeSection { feed_url, bindings }
            ConfigExchangeSection {
                config_text,
                opml_text,
                pending_config_import,
                bindings,
            }
            SavedFeedsSection { feeds, pending_delete_feed, bindings }
        }
    }
}
