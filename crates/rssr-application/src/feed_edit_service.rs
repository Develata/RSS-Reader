//! Edits a subscription in place, preserving its identity and existing articles.
use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{DomainError, FeedEditDetails, FeedEditRequest, parse_and_normalize_feed_url};

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait FeedEditPort: Send + Sync {
    async fn edit_details(&self, feed_id: i64) -> anyhow::Result<FeedEditDetails>;
    /// One write: update the feed, migrate selected source URLs, and fence refreshes
    /// issued for the former URL. A duplicate URL refuses the entire write.
    async fn update_subscription(&self, edit: FeedEditRequest) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct FeedEditService {
    port: Arc<dyn FeedEditPort>,
}

impl FeedEditService {
    pub fn new(port: Arc<dyn FeedEditPort>) -> Self {
        Self { port }
    }

    pub async fn details(&self, feed_id: i64) -> anyhow::Result<FeedEditDetails> {
        if feed_id <= 0 {
            return Err(DomainError::InvalidInput("来源 ID 不合法".into()).into());
        }
        self.port.edit_details(feed_id).await.context("读取来源失败")
    }

    pub async fn update(&self, mut edit: FeedEditRequest) -> anyhow::Result<()> {
        if edit.feed_id <= 0 {
            return Err(DomainError::InvalidInput("来源 ID 不合法".into()).into());
        }
        edit.url = parse_and_normalize_feed_url(edit.url.trim())?.to_string();
        edit.custom_title =
            edit.custom_title.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        if edit.custom_title.as_ref().is_some_and(|s| s.chars().count() > 200) {
            return Err(DomainError::InvalidInput("自定义名称不能超过 200 个字".into()).into());
        }
        self.port.update_subscription(edit).await.context("保存来源失败")
    }
}
