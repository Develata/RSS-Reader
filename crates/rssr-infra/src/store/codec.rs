//! Conversions between stored text and domain values. Times are RFC 3339 UTC strings, so their
//! lexical order is their time order (list sorting and the archive cutoff rely on it).

use rssr_domain::DomainError;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use url::Url;

use super::error::{StoreError, StoreResult};

pub fn now() -> String {
    format_time(OffsetDateTime::now_utc())
}

pub fn format_time(value: OffsetDateTime) -> String {
    value.to_offset(time::UtcOffset::UTC).format(&Rfc3339).unwrap_or_default()
}

pub fn format_optional_time(value: Option<OffsetDateTime>) -> Option<String> {
    value.map(format_time)
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

/// A stored value the program cannot read back.
fn corrupt(detail: String) -> StoreError {
    StoreError::Domain(DomainError::Persistence(format!("资料库中的值无法解析：{detail}")))
}
