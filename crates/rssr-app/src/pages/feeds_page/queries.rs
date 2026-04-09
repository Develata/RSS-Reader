use anyhow::Context;
use rssr_domain::{EntryQuery, FeedSummary};

use crate::bootstrap::AppServices;

#[derive(Debug)]
pub(crate) struct FeedsPageSnapshot {
    pub(crate) feeds: Vec<FeedSummary>,
    pub(crate) feed_count: usize,
    pub(crate) entry_count: usize,
}

pub(crate) async fn load_feeds_page_snapshot() -> anyhow::Result<FeedsPageSnapshot> {
    let services = AppServices::shared().await.context("初始化应用失败")?;
    let feeds = services.list_feeds().await.context("读取订阅失败")?;
    let entry_count =
        services.list_entries(&EntryQuery::default()).await.context("读取文章统计失败")?.len();
    Ok(FeedsPageSnapshot { feed_count: feeds.len(), entry_count, feeds })
}
