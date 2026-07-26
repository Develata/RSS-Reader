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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ReaderAssetLocalizationOutcome {
    pub(crate) localized: bool,
}

#[derive(Clone)]
pub(crate) struct HostCapabilities {
    pub(crate) auto_refresh: Arc<dyn AutoRefreshPort>,
    pub(crate) refresh: Arc<dyn RefreshPort>,
    pub(crate) reader_assets: Arc<dyn ReaderAssetPort>,
    pub(crate) remote_config: Arc<dyn RemoteConfigPort>,
    pub(crate) clipboard: Arc<dyn ClipboardPort>,
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
pub(crate) trait ReaderAssetPort {
    async fn localize_entry_assets(
        &self,
        entry_id: i64,
    ) -> anyhow::Result<ReaderAssetLocalizationOutcome>;
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

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub(crate) trait ClipboardPort {
    async fn read_text(&self) -> anyhow::Result<Option<String>>;
}

/// 下次自动刷新的时刻。`OffsetDateTime + Duration` 在越界时是 panic 的，而刷新间隔可能来自
/// 早于 `MAX_REFRESH_INTERVAL_MINUTES` 上限就已经存到本地的设置，所以这里用 checked 加法：
/// 算不出下次时刻就说明间隔长到实际等同于“不再自动刷新”，返回 `None` 让调用方按此处理。
fn next_auto_refresh_at(
    last_refresh_started_at: time::OffsetDateTime,
    refresh_interval_minutes: u32,
) -> Option<time::OffsetDateTime> {
    last_refresh_started_at.checked_add(time::Duration::minutes(refresh_interval_minutes as i64))
}

fn auto_refresh_wait_duration(
    last_refresh_started_at: Option<time::OffsetDateTime>,
    refresh_interval_minutes: u32,
    now: time::OffsetDateTime,
) -> std::time::Duration {
    const FALLBACK_WAIT: std::time::Duration = std::time::Duration::from_secs(3600);

    match last_refresh_started_at {
        None => std::time::Duration::ZERO,
        Some(last_refresh_started_at) => {
            let Some(next_refresh_at) =
                next_auto_refresh_at(last_refresh_started_at, refresh_interval_minutes)
            else {
                return FALLBACK_WAIT;
            };
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
            next_auto_refresh_at(last_refresh_started_at, refresh_interval_minutes)
                .is_some_and(|next_refresh_at| now >= next_refresh_at)
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
    fn out_of_range_refresh_interval_never_panics_the_auto_refresh_loop() {
        let last = OffsetDateTime::parse("2026-04-01T12:00:00Z", &Rfc3339).expect("parse last");

        // `last + Duration::minutes(u32::MAX)` 会越过 OffsetDateTime 的年份上界，
        // 直接相加会 panic 并悄悄杀死后台自动刷新任务。
        assert!(!should_trigger_auto_refresh(Some(last), u32::MAX, last));
        assert!(auto_refresh_wait_duration(Some(last), u32::MAX, last) > std::time::Duration::ZERO);
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
