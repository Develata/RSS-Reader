use dioxus::prelude::*;

use crate::{
    components::status_banner::StatusBanner, pages::feeds_page::facade::FeedsPageFacade,
    router::AppRoute,
};

use super::support::{feed_refresh_state_attr, feed_refresh_status_text};
use crate::components::icon::Icon;

#[component]
pub(crate) fn SavedFeedsSection(facade: FeedsPageFacade) -> Element {
    let refresh_facade = facade.clone();
    let refreshing = facade.is_refreshing();

    rsx! {
        div { "data-layout": "exchange-header", "data-section": "saved-feeds",
            h2 { "data-slot": "card-title", "已保存订阅" }
            button {
                class: "button",
                "data-variant": "secondary",
                "data-action": "refresh-all",
                r#type: "button",
                disabled: refreshing,
                aria_busy: refreshing,
                onclick: move |_| refresh_facade.refresh_all(),
                if refreshing { "正在刷新…" } else { "刷新全部" }
            }
        }
        if facade.feeds().is_empty() {
            div { "data-layout": "feed-list-state", "data-state": "{facade.feeds_list_state()}",
                StatusBanner { message: facade.empty_feeds_message().to_string(), tone: "info".to_string() }
            }
        } else {
            ul { "data-layout": "feed-list", "data-state": "{facade.feeds_list_state()}",
                for feed in facade.feeds() {
                    { render_feed_card(feed, facade.clone()) }
                }
            }
        }
    }
}

fn render_feed_card(feed: &rssr_domain::FeedSummary, facade: FeedsPageFacade) -> Element {
    let feed_id = feed.id;
    let refresh_feed_title = feed.title.clone();
    let delete_feed_title = feed.title.clone();
    let refresh_facade = facade.clone();
    let edit_facade = facade.clone();
    let remove_facade = facade.clone();
    let cancel_facade = facade.clone();
    let confirming = facade.is_delete_pending_for(feed_id);
    let removing = facade.is_removing_feed(feed_id);

    rsx! {
        li {
            key: "{feed_id}",
            "data-layout": "feed-card",
            "data-state": "{facade.remove_feed_state(feed_id)}",
            "data-feed-id": "{feed.id}",
            "data-feed-title": "{feed.title}",
            "data-feed-url": "{feed.url}",
            "data-entry-count": "{feed.entry_count}",
            "data-unread-count": "{feed.unread_count}",
            "data-refresh-state": "{feed_refresh_state_attr(feed)}",
            "data-last-fetched-at": "{feed.last_fetched_at.map(|value| value.unix_timestamp_nanos()).unwrap_or_default()}",
            "data-last-success-at": "{feed.last_success_at.map(|value| value.unix_timestamp_nanos()).unwrap_or_default()}",
            "data-fetch-error": "{feed.fetch_error.clone().unwrap_or_default()}",
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
            if confirming {
                p { id: "remove-feed-hint-{feed_id}", "data-slot": "confirm-hint", "将删除「{feed.title}」及其本地文章。" }
            }
            div { "data-layout": "feed-card-actions", "data-state": "{facade.remove_feed_state(feed_id)}",
                button { class: "workspace-icon-button", "data-action": "edit-feed", aria_label: "编辑来源：{feed.title}", title: "编辑来源", disabled: removing, onclick: move |_| edit_facade.edit_feed(feed_id), Icon { name: "edit" } }
                button {
                    class: "workspace-icon-button",
                    "data-variant": "secondary",
                    "data-slot": "entry-card-action",
                    "data-action": "refresh-feed",
                    aria_label: "刷新来源：{feed.title}", title: "刷新来源 · {feed_refresh_status_text(&feed)}",
                    disabled: removing,
                    onclick: move |_| refresh_facade.refresh_feed(feed_id, refresh_feed_title.clone()),
                    Icon { name: "refresh" }
                }
                button {
                    class: if confirming || removing { "button" } else { "workspace-icon-button" },
                    "data-variant": "{facade.remove_feed_button_variant(feed_id)}",
                    "data-slot": "entry-card-action",
                    "data-state": "{facade.remove_feed_state(feed_id)}",
                    "data-action": "remove-feed",
                    aria_label: "{facade.remove_feed_button_label(feed_id)}：{feed.title}", title: "{facade.remove_feed_button_label(feed_id)}",
                    aria_describedby: confirming.then(|| format!("remove-feed-hint-{feed_id}")),
                    aria_busy: removing,
                    disabled: facade.is_delete_busy(),
                    onclick: move |_| remove_facade.remove_feed(feed_id, delete_feed_title.clone()),
                    Icon { name: "trash" }
                    if confirming || removing { span { "{facade.remove_feed_button_label(feed_id)}" } }
                }
                if confirming {
                    button {
                        class: "button", "data-variant": "secondary", "data-action": "cancel-remove-feed",
                        r#type: "button", aria_label: "取消删除：{feed.title}",
                        onclick: move |_| {
                            cancel_facade.cancel_remove_feed(feed_id);
                            document::eval(&format!("document.querySelector('[data-feed-id=\"{feed_id}\"] [data-action=remove-feed]')?.focus({{preventScroll:true}})"));
                        },
                        "取消"
                    }
                }
            }
        }
    }
}
