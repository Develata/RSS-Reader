//! Build-time RSS snapshot shared by the native preparation tool and the Pages demo.
//! This data format is independent of the demo's current storage adapter.
use rssr_infra::parser::ParsedFeed;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub title: String,
    pub url: Url,
    pub max_entries: usize,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: u32,
    pub fetched_at: OffsetDateTime,
    pub feeds: Vec<SnapshotFeed>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotFeed {
    pub source: Source,
    pub final_url: Url,
    /// SHA-256 of the downloaded RSS bytes, before parsing or selecting entries.
    pub sha256: String,
    pub feed: ParsedFeed,
}
