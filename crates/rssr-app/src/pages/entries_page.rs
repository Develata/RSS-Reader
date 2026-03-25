use dioxus::prelude::*;
use dioxus_router::prelude::Link;
use rssr_domain::{EntryQuery, EntrySummary};

use crate::{
    app::AppNav, bootstrap::AppServices, components::status_banner::StatusBanner, router::AppRoute,
};

#[component]
pub fn EntriesPage() -> Element {
    let mut entries = use_signal(Vec::<EntrySummary>::new);
    let mut status = use_signal(|| "正在加载文章列表…".to_string());

    let _ = use_resource(move || async move {
        match AppServices::shared().await {
            Ok(services) => match services.list_entries(&EntryQuery::default()).await {
                Ok(items) => {
                    status.set(format!("共 {} 篇文章。", items.len()));
                    entries.set(items);
                }
                Err(err) => status.set(format!("读取文章失败：{err}")),
            },
            Err(err) => status.set(format!("初始化应用失败：{err}")),
        }
    });

    rsx! {
        section { class: "page page-entries",
            AppNav {}
            h2 { "文章" }
            p { class: "page-intro", "文章按发布时间倒序展示。选择一篇即可进入阅读页。" }
            StatusBanner { message: status(), tone: "info".to_string() }
            if entries().is_empty() {
                StatusBanner { message: "没有可显示的文章，先去订阅页添加并刷新 feed。".to_string(), tone: "info".to_string() }
            } else {
                ul { class: "entry-list",
                    for entry in entries() {
                        li { class: "entry-card", key: "{entry.id}",
                            Link { class: "entry-card__title", to: AppRoute::ReaderPage { entry_id: entry.id }, "{entry.title}" }
                            div { class: "entry-card__meta", "{entry.feed_title}" }
                        }
                    }
                }
            }
        }
    }
}
