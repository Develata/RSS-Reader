//! Host clock, kept outside the shared application and SQL semantics.
use rssr_application::ClockPort;
use time::OffsetDateTime;

pub struct PlatformClock;

impl ClockPort for PlatformClock {
    fn now_utc(&self) -> OffsetDateTime {
        now_utc()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

#[cfg(target_arch = "wasm32")]
pub fn now_utc() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos((js_sys::Date::now() * 1_000_000.0) as i128)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
}
