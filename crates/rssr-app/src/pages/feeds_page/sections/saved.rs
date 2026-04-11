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
            div { "data-layout": "feed-list-state", "data-state": "{facade.feeds_list_state()}",
                StatusBanner { message: facade.empty_feeds_message().to_string(), tone: "info".to_string() }
            }
        };
    }

    rsx! {
        div { "data-layout": "exchange-header", "data-section": "saved-feeds",
            h3 { "data-slot": "card-title", "已保存订阅" }
        }
        ul { "data-layout": "feed-list", "data-state": "{facade.feeds_list_state()}",
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
        li { key: "{feed_id}", "data-layout": "feed-card", "data-state": "{facade.remove_feed_state(feed_id)}",
            Link {
                "data-slot": "feed-card-title",
                "data-nav": "feed-entries",
                to: AppRoute::FeedEntriesPage { feed_id },
                "{feed.title}"
            }
            p { "data-slot": "feed-card-url", "{feed.url}" }
            div { "data-layout": "feed-card-meta-group",
                p { "data-slot": "feed-card-meta", "本地文章 {feed.entry_count} · 未读 {feed.unread_count}" }
                p { "data-slot": "feed-card-meta", "{feed_refresh_status_text(&feed)}" }
                if let Some(error) = &feed.fetch_error {
                    p { "data-slot": "feed-card-meta", "data-state": "error", "最近失败：{error}" }
                }
            }
            div { "data-layout": "feed-card-actions",
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
