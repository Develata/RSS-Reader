use dioxus::prelude::*;
use rssr_domain::FeedSummary;

use crate::{
    components::status_banner::StatusBanner,
    pages::feeds_page::{FeedsPageBindings, FeedsPageCommand, execute_feeds_page_command},
    router::AppRoute,
};

use super::support::feed_refresh_status_text;

#[component]
pub(crate) fn SavedFeedsSection(
    feeds: Signal<Vec<FeedSummary>>,
    pending_delete_feed: Signal<Option<i64>>,
    bindings: FeedsPageBindings,
) -> Element {
    if feeds().is_empty() {
        return rsx! {
            StatusBanner { message: "还没有订阅，先添加一个 feed URL。".to_string(), tone: "info".to_string() }
        };
    }

    rsx! {
        div { class: "exchange-header exchange-header--saved",
            h3 { "已保存订阅" }
        }
        ul { class: "feed-list",
            for feed in feeds() {
                { render_feed_card(feed, pending_delete_feed, bindings) }
            }
        }
    }
}

fn render_feed_card(
    feed: FeedSummary,
    pending_delete_feed: Signal<Option<i64>>,
    bindings: FeedsPageBindings,
) -> Element {
    let refresh_feed_title = feed.title.clone();
    let delete_feed_title = feed.title.clone();
    let is_delete_pending = pending_delete_feed() == Some(feed.id);

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
                    onclick: move |_| {
                        let command = FeedsPageCommand::RefreshFeed {
                            feed_id: feed.id,
                            feed_title: refresh_feed_title.clone(),
                        };
                        spawn(async move {
                            let outcome = execute_feeds_page_command(command).await;
                            bindings.apply_command_outcome(outcome);
                        });
                    },
                    "刷新此订阅"
                }
                button {
                    class: if is_delete_pending { "button danger" } else { "button secondary danger-outline" },
                    "data-action": "remove-feed",
                    onclick: move |_| {
                        let command = FeedsPageCommand::RemoveFeed {
                            feed_id: feed.id,
                            feed_title: delete_feed_title.clone(),
                            confirmed: pending_delete_feed() == Some(feed.id),
                        };
                        spawn(async move {
                            let outcome = execute_feeds_page_command(command).await;
                            bindings.apply_command_outcome(outcome);
                        });
                    },
                    if is_delete_pending { "确认删除" } else { "删除订阅" }
                }
            }
        }
    }
}
