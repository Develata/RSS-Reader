//! Deployment policy only; application/domain behavior remains shared.
pub(crate) const ENABLED: bool = cfg!(all(target_arch = "wasm32", feature = "pages-demo"));
pub(crate) const NOTICE: &str = "公开静态演示：打开时预载 Develata 每日数学与陶哲轩博客的 RSS 文章快照，保留原文链接。初始内容随站点构建更新，资料库仅在内存中，重新加载页面即重置阅读和收藏等操作。演示不提供在线订阅、刷新或远端配置同步；阅读、搜索、收藏与主题设置可用。";

#[cfg(all(target_arch = "wasm32", feature = "pages-demo"))]
#[path = "../../../build-support/pages_snapshot.rs"]
mod snapshot;

#[cfg(all(target_arch = "wasm32", feature = "pages-demo"))]
pub(crate) fn require_network() -> anyhow::Result<()> {
    if ENABLED {
        return Err(rssr_domain::DomainError::Network(
            "静态演示已禁用此网络操作：GitHub Pages 不提供 feed/CORS 代理。请使用桌面版或自行部署 rssr-web。"
                .to_string(),
        )
        .into());
    }
    Ok(())
}

#[cfg(all(target_arch = "wasm32", feature = "pages-demo"))]
pub(crate) async fn open_library() -> anyhow::Result<rssr_infra::store::LibraryStore> {
    use rssr_application::{FeedRefreshUpdate, RefreshCommit, RefreshStorePort};
    use rssr_domain::{
        AppStateRepository, FeedRepository, NewFeedSubscription, SettingsRepository,
    };
    use rssr_infra::store::{
        LibraryAppState, LibraryFeeds, LibraryRefreshStore, LibrarySettings, LibraryStore,
    };
    let snapshot: snapshot::Snapshot =
        serde_json::from_str(include_str!("../../../assets/pages-demo/snapshot.json"))?;
    anyhow::ensure!(snapshot.schema_version == 1 && !snapshot.feeds.is_empty(), "演示快照无效");
    let store = LibraryStore::open_in_memory().await?;
    let settings = LibrarySettings::new(store.clone());
    settings
        .save(&rssr_domain::UserSettings { refresh_interval_minutes: 0, ..Default::default() })
        .await?;
    LibraryAppState::new(store.clone())
        .update(Box::new(|state| {
            // A snapshot built long after fetching must still show its articles.
            state.entries_workspace.show_archived = true;
        }))
        .await?;
    let feeds = LibraryFeeds::new(store.clone());
    let refresh = LibraryRefreshStore::new(store.clone());
    for feed in snapshot.feeds {
        let subscription = feeds
            .upsert_subscription(&NewFeedSubscription {
                url: feed.source.url,
                title: Some(feed.source.title),
                source_title: feed.feed.title.clone(),
                site_url: feed.feed.site_url.clone(),
                folder: None,
            })
            .await?;
        let target = refresh
            .get_target(subscription.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("演示订阅未建立"))?;
        refresh
            .commit(
                &target,
                RefreshCommit::Updated {
                    update: FeedRefreshUpdate {
                        metadata: Default::default(),
                        feed: feed.feed.into(),
                    },
                },
            )
            .await?;
    }
    Ok(store)
}
