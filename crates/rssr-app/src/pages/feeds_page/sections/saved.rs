use dioxus::prelude::*;

use crate::{
    components::status_banner::StatusBanner, pages::feeds_page::facade::FeedsPageFacade,
    router::AppRoute,
};

use super::support::feed_refresh_status_text;

#[component]
pub(crate) fn SavedFeedsSection(facade: FeedsPageFacade) -> Element {
    if facade.feeds().is_empty() {
        return rsx! {
            div { "data-state": "{facade.feeds_list_state()}",
                StatusBanner { message: facade.empty_feeds_message().to_string(), tone: "info".to_string() }
            }
        };
    }

    rsx! {
        div { class: "exchange-header exchange-header--saved",
            h3 { class: "card-title", "已保存订阅" }
        }
        ul { class: "feed-list", "data-state": "{facade.feeds_list_state()}",
            for feed in facade.feeds() {
                { render_feed_card(feed, facade.clone()) }
            }
        }
    }
}

fn render_feed_card(feed: &rssr_domain::FeedSummary, facade: FeedsPageFacade) -> Element {
    let feed_id = feed.id;
    let refresh_feed_title = feed.title.clone();
    let delete_feed_title = feed.title.clone();
    let refresh_facade = facade.clone();

    rsx! {
        li { class: "feed-card", key: "{feed_id}", "data-state": "{facade.remove_feed_state(feed_id)}",
            Link {
                class: "feed-card__title",
                "data-slot": "feed-card-title",
                "data-nav": "feed-entries",
                to: AppRoute::FeedEntriesPage { feed_id },
                "{feed.title}"
            }
            p { class: "feed-card__url", "{feed.url}" }
            div { class: "feed-card__meta-group",
                p { class: "feed-card__meta", "data-slot": "feed-card-meta", "本地文章 {feed.entry_count} · 未读 {feed.unread_count}" }
                p { class: "feed-card__meta", "data-slot": "feed-card-meta", "{feed_refresh_status_text(&feed)}" }
                if let Some(error) = &feed.fetch_error {
                    p { class: "feed-card__meta feed-card__meta--error", "data-slot": "feed-card-meta", "最近失败：{error}" }
                }
            }
            div { class: "entry-card__actions",
                button {
                    class: "button",
                    "data-variant": "secondary",
                    "data-slot": "entry-card-action",
                    "data-action": "refresh-feed",
                    onclick: move |_| refresh_facade.refresh_feed(feed_id, refresh_feed_title.clone()),
                    "刷新此订阅"
                }
                button {
                    class: "button",
                    "data-variant": "{facade.remove_feed_button_variant(feed_id)}",
                    "data-slot": "entry-card-action",
                    "data-state": "{facade.remove_feed_state(feed_id)}",
                    "data-action": "remove-feed",
                    onclick: move |_| facade.remove_feed(feed_id, delete_feed_title.clone()),
                    "{facade.remove_feed_button_label(feed_id)}"
                }
            }
        }
    }
}
