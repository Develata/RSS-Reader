use anyhow::Result;
use rssr_domain::{ConfigFingerprint, Feed, NewFeedSubscription, UserSettings};
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
    /// The configuration the caller read; the store refuses with
    /// [`rssr_domain::DomainError::Conflict`] when its current configuration differs.
    pub expected: ConfigFingerprint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigReplacementOutcome {
    pub removed_feed_count: usize,
    pub settings_updated: bool,
    /// The settings committed by the replacement.
    pub settings: UserSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivatedSubscription {
    pub feed: Feed,
    pub generation: i64,
    /// Ticket for the response fetched before activation (the add flow's prepared fetch),
    /// issued in the activation transaction together with the subscription generation.
    pub ticket: crate::RefreshTicket,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait SubscriptionActivationPort: Send + Sync {
    /// Persist one subscription activation/reactivation and return the generation created by that
    /// exact mutation. Callers must carry this token into any prepared refresh response instead of
    /// re-reading a later "current generation".
    async fn activate_subscription(
        &self,
        new_feed: NewFeedSubscription,
    ) -> Result<ActivatedSubscription>;
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
pub trait SettingsUpdatePort: Send + Sync {
    /// In one write of the store: merges `patch` into the stored settings with
    /// [`crate::SettingsPatch::merge`], saves the result when it differs, and returns it.
    async fn update_settings(&self, patch: crate::SettingsPatch) -> Result<UserSettings>;
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait ConfigReplacementPort: Send + Sync {
    /// Replace the persisted configuration from desired state. Implementations must compare
    /// `plan.expected` and derive the removal diff only after acquiring their backend write
    /// lock/transaction.
    async fn replace_config(&self, plan: ConfigReplacementPlan)
    -> Result<ConfigReplacementOutcome>;
}
