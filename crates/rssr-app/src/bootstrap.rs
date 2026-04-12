use std::sync::Arc;

use rssr_application::{RemoteConfigPullOutcome, RemoteConfigPushOutcome};

#[cfg(not(target_arch = "wasm32"))]
#[path = "bootstrap/native.rs"]
mod imp;

#[cfg(target_arch = "wasm32")]
#[path = "bootstrap/web.rs"]
mod imp;

pub use imp::{AppServices, ReaderNavigation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AddSubscriptionOutcome {
    SavedAndRefreshed,
    SavedRefreshFailed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefreshAllExecutionOutcome {
    pub(crate) failure_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefreshFeedExecutionOutcome {
    pub(crate) failure_message: Option<String>,
}

#[derive(Clone)]
pub(crate) struct HostCapabilities {
    pub(crate) auto_refresh: Arc<dyn AutoRefreshPort>,
    pub(crate) refresh: Arc<dyn RefreshPort>,
    pub(crate) remote_config: Arc<dyn RemoteConfigPort>,
}

pub(crate) trait AutoRefreshPort {
    fn ensure_started(&self);
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub(crate) trait RefreshPort {
    async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<AddSubscriptionOutcome>;
    async fn refresh_all(&self) -> anyhow::Result<RefreshAllExecutionOutcome>;
    async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<RefreshFeedExecutionOutcome>;
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub(crate) trait RemoteConfigPort {
    async fn push(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<RemoteConfigPushOutcome>;
    async fn pull(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<RemoteConfigPullOutcome>;
}

fn auto_refresh_wait_duration(
    last_refresh_started_at: Option<time::OffsetDateTime>,
    refresh_interval_minutes: u32,
    now: time::OffsetDateTime,
) -> std::time::Duration {
    match last_refresh_started_at {
        None => std::time::Duration::ZERO,
        Some(last_refresh_started_at) => {
            let next_refresh_at =
                last_refresh_started_at + time::Duration::minutes(refresh_interval_minutes as i64);
            if now >= next_refresh_at {
                std::time::Duration::ZERO
            } else {
                (next_refresh_at - now).try_into().unwrap_or(std::time::Duration::ZERO)
            }
        }
    }
}

fn should_trigger_auto_refresh(
    last_refresh_started_at: Option<time::OffsetDateTime>,
    refresh_interval_minutes: u32,
    now: time::OffsetDateTime,
) -> bool {
    match last_refresh_started_at {
        None => true,
        Some(last_refresh_started_at) => {
            now >= last_refresh_started_at
                + time::Duration::minutes(refresh_interval_minutes as i64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{auto_refresh_wait_duration, should_trigger_auto_refresh};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    #[test]
    fn auto_refresh_triggers_immediately_when_never_run() {
        let now = OffsetDateTime::parse("2026-04-01T12:00:00Z", &Rfc3339).expect("parse now");
        assert!(should_trigger_auto_refresh(None, 30, now));
    }

    #[test]
    fn auto_refresh_waits_until_interval_has_elapsed() {
        let last = OffsetDateTime::parse("2026-04-01T12:00:00Z", &Rfc3339).expect("parse last");
        let before = OffsetDateTime::parse("2026-04-01T12:29:59Z", &Rfc3339).expect("parse before");
        let after = OffsetDateTime::parse("2026-04-01T12:30:00Z", &Rfc3339).expect("parse after");

        assert!(!should_trigger_auto_refresh(Some(last), 30, before));
        assert!(should_trigger_auto_refresh(Some(last), 30, after));
    }

    #[test]
    fn auto_refresh_wait_duration_is_zero_when_never_run() {
        let now = OffsetDateTime::parse("2026-04-01T12:00:00Z", &Rfc3339).expect("parse now");
        assert_eq!(auto_refresh_wait_duration(None, 30, now), std::time::Duration::ZERO);
    }

    #[test]
    fn auto_refresh_wait_duration_returns_remaining_interval() {
        let last = OffsetDateTime::parse("2026-04-01T12:00:00Z", &Rfc3339).expect("parse last");
        let now = OffsetDateTime::parse("2026-04-01T12:10:00Z", &Rfc3339).expect("parse now");
        assert_eq!(
            auto_refresh_wait_duration(Some(last), 30, now),
            std::time::Duration::from_secs(20 * 60)
        );
    }

    #[test]
    fn auto_refresh_wait_duration_is_zero_after_due_time() {
        let last = OffsetDateTime::parse("2026-04-01T12:00:00Z", &Rfc3339).expect("parse last");
        let now = OffsetDateTime::parse("2026-04-01T12:31:00Z", &Rfc3339).expect("parse now");
        assert_eq!(auto_refresh_wait_duration(Some(last), 30, now), std::time::Duration::ZERO);
    }
}
