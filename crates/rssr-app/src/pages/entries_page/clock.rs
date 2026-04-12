use time::OffsetDateTime;

#[cfg(target_arch = "wasm32")]
pub(super) fn current_time_utc() -> OffsetDateTime {
    let millis = js_sys::Date::now();
    let seconds = (millis / 1_000.0).floor() as i64;
    let nanos = ((millis % 1_000.0) * 1_000_000.0).round() as i64;
    OffsetDateTime::from_unix_timestamp(seconds).expect("valid unix timestamp")
        + time::Duration::nanoseconds(nanos)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn current_time_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}
