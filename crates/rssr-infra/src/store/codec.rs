//! Conversions between stored text and domain values. Times are RFC 3339 UTC strings, so their
//! lexical order is their time order (list sorting and the archive cutoff rely on it). Published
//! times are moreover whole seconds, like archive cutoffs (B2-D7): the refresh commit stores them
//! through `rssr_domain::stored_published_at` and the v0.1.x import rewrites the others.

use rssr_domain::DomainError;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use url::Url;

use super::error::{StoreError, StoreResult};

pub fn now() -> String {
    format_time(OffsetDateTime::now_utc()).expect("the current time is RFC 3339 formattable")
}

/// RFC 3339 has four-digit years; a time outside 0000-9999 cannot be stored and fails the
/// write (it is never stored as an empty string).
pub fn format_time(value: OffsetDateTime) -> StoreResult<String> {
    value
        .to_offset(time::UtcOffset::UTC)
        .format(&Rfc3339)
        .map_err(|error| corrupt_input(format!("时间 {value} 无法保存：{error}")))
}

pub fn format_optional_time(value: Option<OffsetDateTime>) -> StoreResult<Option<String>> {
    value.map(format_time).transpose()
}

pub fn parse_time(raw: &str) -> StoreResult<OffsetDateTime> {
    OffsetDateTime::parse(raw, &Rfc3339).map_err(|error| corrupt(format!("时间 {raw:?}：{error}")))
}

pub fn parse_optional_time(raw: Option<String>) -> StoreResult<Option<OffsetDateTime>> {
    raw.as_deref().map(parse_time).transpose()
}

pub fn parse_url(raw: &str) -> StoreResult<Url> {
    Url::parse(raw).map_err(|error| corrupt(format!("地址 {raw:?}：{error}")))
}

pub fn parse_optional_url(raw: Option<String>) -> StoreResult<Option<Url>> {
    raw.as_deref().map(parse_url).transpose()
}

fn corrupt_input(detail: String) -> StoreError {
    StoreError::Domain(DomainError::Persistence(detail))
}

/// A stored value the program cannot read back.
fn corrupt(detail: String) -> StoreError {
    StoreError::Domain(DomainError::Persistence(format!("资料库中的值无法解析：{detail}")))
}
