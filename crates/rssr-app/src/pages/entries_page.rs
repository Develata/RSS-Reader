use dioxus::prelude::*;
use dioxus_router::prelude::Link;
use rssr_domain::{EntryQuery, EntrySummary};

use crate::{app::AppNav, bootstrap::AppServices, router::AppRoute};

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
        section {
            AppNav {}
            h2 { "文章" }
            p { "{status}" }
            ul {
                for entry in entries() {
                    li { key: "{entry.id}",
                        Link { to: AppRoute::ReaderPage { entry_id: entry.id }, "{entry.title}" }
                        " · "
                        span { "{entry.feed_title}" }
                    }
                }
            }
        }
    }
}
