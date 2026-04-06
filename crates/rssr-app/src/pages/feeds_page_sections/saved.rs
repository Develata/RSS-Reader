use dioxus::prelude::*;
use rssr_domain::FeedSummary;

use crate::{
    bootstrap::AppServices,
    components::status_banner::StatusBanner,
    router::AppRoute,
    status::{set_status_error, set_status_info},
};

use super::support::feed_refresh_status_text;

#[component]
pub(crate) fn SavedFeedsSection(
    feeds: Signal<Vec<FeedSummary>>,
    pending_delete_feed: Signal<Option<i64>>,
    reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
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
                { render_feed_card(feed, pending_delete_feed, reload_tick, status, status_tone) }
            }
        }
    }
}

fn render_feed_card(
    feed: FeedSummary,
    pending_delete_feed: Signal<Option<i64>>,
    reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
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
                    onclick: move |_| refresh_feed(feed.id, refresh_feed_title.clone(), reload_tick, status, status_tone),
                    "刷新此订阅"
                }
                button {
                    class: if is_delete_pending { "button danger" } else { "button secondary danger-outline" },
                    "data-action": "remove-feed",
                    onclick: move |_| remove_feed(
                        feed.id,
                        delete_feed_title.clone(),
                        pending_delete_feed,
                        reload_tick,
                        status,
                        status_tone,
                    ),
                    if is_delete_pending { "确认删除" } else { "删除订阅" }
                }
            }
        }
    }
}

fn refresh_feed(
    feed_id: i64,
    feed_title: String,
    mut reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) {
    spawn(async move {
        match AppServices::shared().await {
            Ok(services) => match services.refresh_feed(feed_id).await {
                Ok(()) => {
                    set_status_info(status, status_tone, format!("已刷新订阅：{}", feed_title));
                    reload_tick += 1;
                }
                Err(err) => set_status_error(status, status_tone, format!("刷新订阅失败：{err}")),
            },
            Err(err) => set_status_error(status, status_tone, format!("初始化应用失败：{err}")),
        }
    });
}

fn remove_feed(
    feed_id: i64,
    feed_title: String,
    mut pending_delete_feed: Signal<Option<i64>>,
    mut reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) {
    if pending_delete_feed() != Some(feed_id) {
        pending_delete_feed.set(Some(feed_id));
        set_status_info(status, status_tone, format!("再次点击即可删除订阅：{}", feed_title));
        return;
    }

    spawn(async move {
        match AppServices::shared().await {
            Ok(services) => match services.remove_feed(feed_id).await {
                Ok(()) => {
                    pending_delete_feed.set(None);
                    set_status_info(status, status_tone, format!("已删除订阅：{}", feed_title));
                    reload_tick += 1;
                }
                Err(err) => {
                    pending_delete_feed.set(None);
                    set_status_error(status, status_tone, format!("删除订阅失败：{err}"));
                }
            },
            Err(err) => {
                pending_delete_feed.set(None);
                set_status_error(status, status_tone, format!("初始化应用失败：{err}"));
            }
        }
    });
}
