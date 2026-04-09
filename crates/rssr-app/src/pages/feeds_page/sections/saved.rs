use dioxus::prelude::*;

use crate::{
    components::status_banner::StatusBanner, pages::feeds_page::session::FeedsPageSession,
    router::AppRoute,
};

use super::support::feed_refresh_status_text;

#[component]
pub(crate) fn SavedFeedsSection(session: FeedsPageSession) -> Element {
    if session.feeds().is_empty() {
        return rsx! {
            StatusBanner { message: "还没有订阅，先添加一个 feed URL。".to_string(), tone: "info".to_string() }
        };
    }

    rsx! {
        div { class: "exchange-header exchange-header--saved",
            h3 { "已保存订阅" }
        }
        ul { class: "feed-list",
            for feed in session.feeds() {
                { render_feed_card(feed, session) }
            }
        }
    }
}

fn render_feed_card(feed: rssr_domain::FeedSummary, session: FeedsPageSession) -> Element {
    let refresh_feed_title = feed.title.clone();
    let delete_feed_title = feed.title.clone();
    let is_delete_pending = session.is_delete_pending_for(feed.id);

    rsx! {
        li { class: "feed-card", key: "{feed.id}",
            Link {
                class: "feed-card__title",
                "data-nav": "feed-entries",
                to: AppRoute::FeedEntriesPage { feed_id: feed.id },
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
                    onclick: move |_| session.refresh_feed(feed.id, refresh_feed_title.clone()),
                    "刷新此订阅"
                }
                button {
                    class: if is_delete_pending { "button danger" } else { "button secondary danger-outline" },
                    "data-action": "remove-feed",
                    onclick: move |_| session.remove_feed(feed.id, delete_feed_title.clone()),
                    if is_delete_pending { "确认删除" } else { "删除订阅" }
                }
            }
        }
    }
}
