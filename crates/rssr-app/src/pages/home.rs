use dioxus::prelude::*;

use crate::{app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner};

#[component]
pub fn HomePage() -> Element {
    let mut feed_count = use_signal(|| 0_usize);
    let mut entry_count = use_signal(|| 0_usize);
    let mut error = use_signal(|| None::<String>);

    let _ = use_resource(move || async move {
        match AppServices::shared().await {
            Ok(services) => {
                match services.list_feeds().await {
                    Ok(feeds) => feed_count.set(feeds.len()),
                    Err(err) => error.set(Some(err.to_string())),
                }

                match services.list_entries(&rssr_domain::EntryQuery::default()).await {
                    Ok(entries) => entry_count.set(entries.len()),
                    Err(err) => error.set(Some(err.to_string())),
                }
            }
            Err(err) => error.set(Some(err.to_string())),
        }
    });

    rsx! {
        section { class: "page page-home", "data-page": "home",
            AppNav {}
            h2 { "首页" }
            p { class: "page-intro", "当前 MVP 已接入真实 SQLite 数据源，可以直接添加订阅、刷新内容并进入阅读页。" }
            div { class: "stats-grid",
                div { class: "stat-card",
                    div { class: "stat-card__label", "订阅数" }
                    div { class: "stat-card__value", "{feed_count}" }
                }
                div { class: "stat-card",
                    div { class: "stat-card__label", "文章数" }
                    div { class: "stat-card__value", "{entry_count}" }
                }
            }
            if let Some(message) = error() {
                StatusBanner { message, tone: "error".to_string() }
            }
        }
    }
}
