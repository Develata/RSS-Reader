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
            StatusBanner { message: "还没有订阅，先添加一个 feed URL。".to_string(), tone: "info".to_string() }
        };
    }

    rsx! {
        div { class: "exchange-header exchange-header--saved",
            h3 { "已保存订阅" }
        }
        ul { class: "feed-list",
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
    let is_delete_pending = facade.is_delete_pending_for(feed_id);
    let refresh_facade = facade.clone();

    rsx! {
        li { class: "feed-card", key: "{feed_id}",
            Link {
                class: "feed-card__title",
                "data-nav": "feed-entries",
                to: AppRoute::FeedEntriesPage { feed_id },
                "{feed.title}"
            }
            p { class: "feed-card__url", "{feed.url}" }
            div { class: "feed-card__meta-group",
                p { class: "feed-card__meta", "本地文章 {feed.entry_count} · 未读 {feed.unread_count}" }
                p { class: "feed-card__meta", "{feed_refresh_status_text(&feed)}" }
                if let Some(error) = &feed.fetch_error {
                    p { class: "feed-card__meta feed-card__meta--error", "最近失败：{error}" }
                }
            }
            div { class: "entry-card__actions",
                button {
                    class: "button secondary",
                    "data-action": "refresh-feed",
                    onclick: move |_| refresh_facade.refresh_feed(feed_id, refresh_feed_title.clone()),
                    "刷新此订阅"
                }
                button {
                    class: if is_delete_pending { "button danger" } else { "button secondary danger-outline" },
                    "data-action": "remove-feed",
                    onclick: move |_| facade.remove_feed(feed_id, delete_feed_title.clone()),
                    if is_delete_pending { "确认删除" } else { "删除订阅" }
                }
            }
        }
    }
}
