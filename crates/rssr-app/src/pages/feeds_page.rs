use dioxus::prelude::*;
use rssr_domain::FeedSummary;

use super::feeds_page_sections::{ConfigExchangeSection, FeedComposeSection, SavedFeedsSection};
use crate::{
    app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation, router::AppRoute,
    status::set_status_error,
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
    let mut feeds = use_signal(Vec::<FeedSummary>::new);
    let mut feed_count = use_signal(|| 0_usize);
    let mut entry_count = use_signal(|| 0_usize);
    let status = use_signal(String::new);
    let status_tone = use_signal(|| "info".to_string());

    use_resource(move || async move {
        let _ = reload_tick();
        match AppServices::shared().await {
            Ok(services) => {
                match services.list_feeds().await {
                    Ok(items) => {
                        feed_count.set(items.len());
                        feeds.set(items);
                    }
                    Err(err) => {
                        set_status_error(status, status_tone, format!("读取订阅失败：{err}"));
                    }
                }

                match services.list_entries(&rssr_domain::EntryQuery::default()).await {
                    Ok(entries) => entry_count.set(entries.len()),
                    Err(err) => {
                        set_status_error(status, status_tone, format!("读取文章统计失败：{err}"));
                    }
                }
            }
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
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
            FeedComposeSection { feed_url, reload_tick, status, status_tone }
            ConfigExchangeSection {
                config_text,
                opml_text,
                pending_config_import,
                reload_tick,
                status,
                status_tone,
            }
            SavedFeedsSection { feeds, pending_delete_feed, reload_tick, status, status_tone }
        }
    }
}
