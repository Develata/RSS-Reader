//! Deployment policy only; application/domain behavior remains shared.
pub(crate) const ENABLED: bool = cfg!(all(target_arch = "wasm32", feature = "pages-demo"));
pub(crate) const NOTICE: &str = "公开静态演示：首次打开预载 Develata 每日数学与陶哲轩博客的 RSS 文章快照，保留原文链接。初始内容随站点构建更新，已有浏览器数据保留。演示不提供在线订阅、刷新或远端配置同步；阅读、搜索、收藏与主题设置可用。";

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
pub(crate) fn initial_state()
-> anyhow::Result<rssr_infra::application_adapters::browser::state::BrowserState> {
    use rssr_infra::application_adapters::browser::state::{
        BrowserState, PersistedFeed, upsert_entries,
    };
    let snapshot: snapshot::Snapshot =
        serde_json::from_str(include_str!("../../../assets/pages-demo/snapshot.json"))?;
    anyhow::ensure!(snapshot.schema_version == 1 && !snapshot.feeds.is_empty(), "演示快照无效");
    let mut state = BrowserState::default();
    state.core.settings.refresh_interval_minutes = 0;
    // A checked-in snapshot must remain readable even if built much later than it was fetched.
    state.app_state.entries_workspace.show_archived = true;
    for feed in snapshot.feeds {
        state.core.next_feed_id += 1;
        let id = state.core.next_feed_id;
        state.core.feeds.push(PersistedFeed {
            id,
            url: feed.source.url.to_string(),
            title: feed.feed.title,
            custom_title: Some(feed.source.title),
            site_url: feed.feed.site_url.map(|url| url.to_string()),
            description: feed.feed.description,
            icon_url: None,
            folder: None,
            etag: None,
            last_modified: None,
            last_fetched_at: Some(snapshot.fetched_at),
            last_success_at: Some(snapshot.fetched_at),
            fetch_error: None,
            fetch_error_kind: None,
            is_deleted: false,
            created_at: snapshot.fetched_at,
            updated_at: snapshot.fetched_at,
        });
        upsert_entries(&mut state, id, feed.feed.entries)?;
    }
    Ok(state)
}
