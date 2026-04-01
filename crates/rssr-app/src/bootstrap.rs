#[cfg(not(target_arch = "wasm32"))]
#[path = "bootstrap/native.rs"]
mod imp;

#[cfg(target_arch = "wasm32")]
#[path = "bootstrap/web.rs"]
mod imp;

pub use imp::{AppServices, ReaderNavigation};

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
    use super::should_trigger_auto_refresh;
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
}
