use std::sync::Arc;

use rssr_domain::{EntryQuery, EntrySummary, FeedSummary, UserSettings};

use crate::bootstrap::AppServices;

pub(crate) async fn remember_last_opened_feed(feed_id: i64) {
    if let Ok(services) = AppServices::shared().await {
        let _ = services.remember_last_opened_feed_id(feed_id).await;
    }
}

pub(crate) async fn load_entries_page_preferences() -> Result<UserSettings, String> {
    shared_services().await?.load_settings().await.map_err(|err| format!("读取设置失败：{err}"))
}

pub(crate) async fn load_entries_page_feeds() -> Result<Vec<FeedSummary>, String> {
    shared_services().await?.list_feeds().await.map_err(|err| format!("读取订阅失败：{err}"))
}

pub(crate) async fn load_entries_page_entries(
    query: EntryQuery,
) -> Result<Vec<EntrySummary>, String> {
    shared_services()
        .await?
        .list_entries(&query)
        .await
        .map_err(|err| format!("读取文章失败：{err}"))
}

async fn shared_services() -> Result<Arc<AppServices>, String> {
    AppServices::shared().await.map_err(|err| format!("初始化应用失败：{err}"))
}
