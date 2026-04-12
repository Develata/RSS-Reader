pub mod adapters;
pub mod config;
pub mod feed;
mod feed_request;
mod feed_response;
pub mod query;
pub mod state;

use js_sys::Date;
use time::OffsetDateTime;

pub fn now_utc() -> OffsetDateTime {
    let millis = Date::now() as i128;
    OffsetDateTime::from_unix_timestamp_nanos(millis * 1_000_000)
        .expect("browser timestamp should fit in OffsetDateTime")
}
