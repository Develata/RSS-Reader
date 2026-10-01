use anyhow::Result;
use rssr_domain::UserSettings;
use url::Url;

#[derive(Debug, Clone)]
pub struct ConfigReplacementFeed {
    pub url: Url,
    pub title: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigReplacementPlan {
    pub feeds: Vec<ConfigReplacementFeed>,
    pub settings: UserSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigReplacementOutcome {
    pub removed_feed_count: usize,
    pub settings_updated: bool,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait SubscriptionRemovalPort: Send + Sync {
    /// Remove one subscription and perform all persistence-side cleanup owned by that removal.
    /// Callers must not repeat cleanup after this method succeeds.
    async fn remove_subscription(&self, feed_id: i64, purge_entries: bool) -> Result<()>;
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait ConfigReplacementPort: Send + Sync {
    /// Replace the persisted configuration from desired state. Implementations must derive the
    /// removal diff only after acquiring their backend write lock/transaction.
    async fn replace_config(&self, plan: ConfigReplacementPlan) -> Result<ConfigReplacementOutcome>;
}
