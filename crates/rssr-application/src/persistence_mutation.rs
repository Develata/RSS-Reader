use anyhow::Result;
use rssr_domain::{NewFeedSubscription, UserSettings};

#[derive(Debug, Clone)]
pub struct ConfigReplacementPlan {
    pub upserts: Vec<NewFeedSubscription>,
    pub removed_feed_ids: Vec<i64>,
    pub settings: UserSettings,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait SubscriptionRemovalPort: Send + Sync {
    async fn remove_subscription(&self, feed_id: i64, purge_entries: bool) -> Result<()>;
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait ConfigReplacementPort: Send + Sync {
    async fn replace_config(&self, plan: ConfigReplacementPlan) -> Result<()>;
}
