//! The HTTP protocol between the server (`rssr-web`) and the browser's backend (v0.2 phase C, C3;
//! `docs/design/v0.2-phase-c-plan.md`). Data only, no HTTP library: both sides share it.
//!
//! Every [`crate::Backend`] operation has one route under [`API_PREFIX`]. Reads are `GET`; a read
//! whose input is structured carries it as JSON in one query parameter. Operations with side
//! effects use `POST`, `PUT`, `PATCH` or `DELETE`. Five network-heavy operations run as background
//! jobs ([`JobKind`]): the request returns a [`JobCreated`], and the client polls
//! [`job`] until the [`JobStatus`] is final. Every response body is JSON; every error is a
//! [`crate::BackendError`] with the HTTP status of its [`crate::ErrorCode`].
//!
//! The wire form, which the server and the browser's backend both follow:
//! - a write's body is the JSON of its request type below, except `POST routes::MARK_READ`,
//!   whose body is the [`rssr_domain::MarkReadPreview`] itself;
//! - a success is `200` with the operation's result as JSON; an operation without a result
//!   answers `null`; the exports (`GET routes::CONFIG_JSON`, `GET routes::CONFIG_OPML`) answer
//!   the document as a JSON string;
//! - starting a job answers `202` with a [`JobCreated`]; polling it answers `200` with its
//!   [`JobStatus`]. `wait_ms` is a plain integer (absent: 0; more than [`MAX_JOB_WAIT`] counts as
//!   that). A job whose status is gone (an id that is malformed or unknown, a status kept past
//!   its retention or evicted, a server restarted since) is
//!   [`crate::ErrorCode::StatusUnavailable`]: the server keeps finished statuses for a while at
//!   most, never a guaranteed time;
//! - bodies are bounded ([`MAX_BODY_BYTES`], with the larger bounds of the settings, the
//!   mark-read confirmation and the imports), and so is a structured read's parameter
//!   ([`MAX_QUERY_PARAMETER_BYTES`]); more is [`crate::ErrorCode::TooLarge`];
//! - every answer of the application's routes carries [`REQUEST_ID_HEADER`] (an error carries the
//!   same id in its body) and is not cached.

use std::time::Duration;

use rssr_application::{
    AddSubscriptionOutcome, RefreshAllExecutionOutcome, RefreshFeedExecutionOutcome,
    RemoteConfigPullOutcome, RemoteConfigPushOutcome,
};
use rssr_domain::{ConfigFingerprint, MAX_CUSTOM_CSS_BYTES};
use serde::{Deserialize, Serialize};

use crate::BackendError;

/// The protocol version's path prefix.
pub const API_PREFIX: &str = "/api/v1";
/// The build the page was made with; writes from another build are refused
/// ([`crate::ErrorCode::BuildMismatch`]).
pub const BUILD_HEADER: &str = "x-rssr-build";
/// The session's CSRF token, on every write after login.
pub const CSRF_HEADER: &str = "x-rssr-csrf";
/// The id of the request, on every answer of the application's routes; it matches the server's
/// log entry and the `request_id` of an error.
pub const REQUEST_ID_HEADER: &str = "x-rssr-request-id";
/// The longest query parameter a structured read may carry (proxies commonly limit request
/// lines to 8 KiB).
pub const MAX_QUERY_PARAMETER_BYTES: usize = 4096;
/// The longest a single poll of a job waits for it to finish; the client polls again.
pub const MAX_JOB_WAIT: Duration = Duration::from_secs(10);
/// The largest body of a write, unless its route has its own bound below.
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// `PATCH routes::SETTINGS`: up to [`MAX_CUSTOM_CSS_BYTES`] of custom CSS, which JSON escaping
/// makes up to six times longer (a control character becomes `\u00XX`).
pub const MAX_SETTINGS_BODY_BYTES: usize = 6 * MAX_CUSTOM_CSS_BYTES + 16 * 1024;
/// `POST routes::MARK_READ`: a preview of up to [`rssr_domain::MAX_MARK_READ_SELECTION`] ids (at
/// most 20 bytes each, with the comma) and its query (at most [`MAX_QUERY_PARAMETER_BYTES`] when
/// it was previewed).
pub const MAX_MARK_READ_BODY_BYTES: usize = 256 * 1024;
/// `PUT routes::CONFIG_JSON` and `POST routes::CONFIG_OPML`: a configuration document.
pub const MAX_IMPORT_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Route patterns as the server registers them (`{name}` is a path parameter).
pub mod routes {
    pub const SESSION: &str = "/api/v1/session";
    pub const SETTINGS: &str = "/api/v1/settings";
    pub const STARTUP_TARGET: &str = "/api/v1/startup-target";
    pub const ENTRIES_BOOTSTRAP: &str = "/api/v1/entries/bootstrap";
    pub const OPENED_FEED: &str = "/api/v1/workspace/opened-feed/{feed_id}";
    pub const WORKSPACE: &str = "/api/v1/workspace";
    pub const ENTRIES: &str = "/api/v1/entries";
    pub const ENTRY: &str = "/api/v1/entries/{entry_id}";
    pub const ENTRY_READ: &str = "/api/v1/entries/{entry_id}/read";
    pub const ENTRY_STARRED: &str = "/api/v1/entries/{entry_id}/starred";
    pub const ENTRY_ASSETS: &str = "/api/v1/entries/{entry_id}/localize-assets";
    pub const MARK_READ: &str = "/api/v1/entries/mark-read";
    pub const FEEDS: &str = "/api/v1/feeds";
    pub const FEED: &str = "/api/v1/feeds/{feed_id}";
    pub const CONFIG_JSON: &str = "/api/v1/config/json";
    pub const CONFIG_OPML: &str = "/api/v1/config/opml";
    pub const JOB_REFRESH_ALL: &str = "/api/v1/jobs/refresh-all";
    pub const JOB_REFRESH_FEED: &str = "/api/v1/jobs/refresh-feed/{feed_id}";
    pub const JOB_ADD_SUBSCRIPTION: &str = "/api/v1/jobs/add-subscription";
    pub const JOB_REMOTE_PUSH: &str = "/api/v1/jobs/remote-config/push";
    pub const JOB_REMOTE_PULL: &str = "/api/v1/jobs/remote-config/pull";
    pub const JOB: &str = "/api/v1/jobs/{job_id}";
}

/// `routes::OPENED_FEED` for `feed_id`.
pub fn opened_feed(feed_id: i64) -> String {
    format!("{API_PREFIX}/workspace/opened-feed/{feed_id}")
}

/// `routes::ENTRY` for `entry_id`.
pub fn entry(entry_id: i64) -> String {
    format!("{API_PREFIX}/entries/{entry_id}")
}

/// `routes::ENTRY_READ` for `entry_id`.
pub fn entry_read(entry_id: i64) -> String {
    format!("{API_PREFIX}/entries/{entry_id}/read")
}

/// `routes::ENTRY_STARRED` for `entry_id`.
pub fn entry_starred(entry_id: i64) -> String {
    format!("{API_PREFIX}/entries/{entry_id}/starred")
}

/// `routes::ENTRY_ASSETS` for `entry_id`.
pub fn entry_assets(entry_id: i64) -> String {
    format!("{API_PREFIX}/entries/{entry_id}/localize-assets")
}

/// `routes::FEED` for `feed_id`.
pub fn feed(feed_id: i64) -> String {
    format!("{API_PREFIX}/feeds/{feed_id}")
}

/// `routes::JOB_REFRESH_FEED` for `feed_id`.
pub fn job_refresh_feed(feed_id: i64) -> String {
    format!("{API_PREFIX}/jobs/refresh-feed/{feed_id}")
}

/// `routes::JOB` for `job_id`.
pub fn job(job_id: &JobId) -> String {
    format!("{API_PREFIX}/jobs/{}", job_id.0)
}

/// The query of a structured read: `?query=<JSON>` (the entry list, the mark-read preview) or
/// `?input=<JSON>` (the entry page's bootstrap).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonQuery {
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonInput {
    pub input: String,
}

/// `PUT routes::ENTRY_READ`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetRead {
    pub read: bool,
}

/// `PUT routes::ENTRY_STARRED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetStarred {
    pub starred: bool,
}

/// `PUT routes::CONFIG_JSON`: replace the configuration unless it changed since `expected`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportConfigJson {
    pub raw: String,
    pub expected: ConfigFingerprint,
}

/// `POST routes::CONFIG_OPML`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportOpml {
    pub raw: String,
}

/// `POST routes::JOB_ADD_SUBSCRIPTION`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddSubscription {
    pub raw_url: String,
    pub fallback_site_url: Option<url::Url>,
}

/// `POST routes::JOB_REMOTE_PUSH` and `routes::JOB_REMOTE_PULL`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteConfig {
    pub endpoint: String,
    pub remote_path: String,
}

/// A background job: 128 random bits as 32 lowercase hex digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobId(pub String);

impl JobId {
    /// Whether `raw` has the shape of a job id (the server answers others as unknown).
    pub fn parse(raw: &str) -> Option<Self> {
        let well_formed =
            raw.len() == 32 && raw.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        well_formed.then(|| Self(raw.to_owned()))
    }
}

/// What starting a job returns (`202 Accepted`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobCreated {
    pub job_id: JobId,
}

/// `GET routes::JOB`: wait up to `wait_ms` (at most [`MAX_JOB_WAIT`]) for the job to finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct JobWait {
    #[serde(default)]
    pub wait_ms: u64,
}

/// The operations that run as background jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    RefreshAll,
    RefreshFeed,
    AddSubscription,
    RemoteConfigPush,
    RemoteConfigPull,
}

/// A job's state. A failed job may still have committed part of its work (a refresh of all
/// subscriptions saves each one as it goes): the state is not the business outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded { result: JobResult },
    Failed { error: BackendError },
}

impl JobStatus {
    pub fn is_final(&self) -> bool {
        matches!(self, Self::Succeeded { .. } | Self::Failed { .. })
    }
}

/// A finished job's result, by kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "outcome", rename_all = "snake_case")]
pub enum JobResult {
    RefreshAll(RefreshAllExecutionOutcome),
    RefreshFeed(RefreshFeedExecutionOutcome),
    AddSubscription(AddSubscriptionOutcome),
    RemoteConfigPush(RemoteConfigPushOutcome),
    RemoteConfigPull(RemoteConfigPullOutcome),
}

impl JobResult {
    pub fn kind(&self) -> JobKind {
        match self {
            Self::RefreshAll(_) => JobKind::RefreshAll,
            Self::RefreshFeed(_) => JobKind::RefreshFeed,
            Self::AddSubscription(_) => JobKind::AddSubscription,
            Self::RemoteConfigPush(_) => JobKind::RemoteConfigPush,
            Self::RemoteConfigPull(_) => JobKind::RemoteConfigPull,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ErrorCode;

    #[test]
    fn concrete_paths_match_their_route_patterns() {
        let pairs = [
            (opened_feed(7), routes::OPENED_FEED),
            (entry(7), routes::ENTRY),
            (entry_read(7), routes::ENTRY_READ),
            (entry_starred(7), routes::ENTRY_STARRED),
            (entry_assets(7), routes::ENTRY_ASSETS),
            (feed(7), routes::FEED),
            (job_refresh_feed(7), routes::JOB_REFRESH_FEED),
        ];
        for (path, pattern) in pairs {
            let filled = pattern.replace("{feed_id}", "7").replace("{entry_id}", "7");
            assert_eq!(path, filled, "{pattern}");
            assert!(path.starts_with(API_PREFIX));
        }
        let id = JobId::parse("0123456789abcdef0123456789abcdef").unwrap();
        assert_eq!(job(&id), routes::JOB.replace("{job_id}", &id.0));
    }

    /// The largest legal settings change and mark-read confirmation fit their bodies.
    #[test]
    fn the_largest_legal_bodies_fit_their_bounds() {
        let patch = rssr_application::SettingsPatch {
            custom_css: Some("\u{1}".repeat(MAX_CUSTOM_CSS_BYTES)),
            ..Default::default()
        };
        let size = serde_json::to_vec(&patch).unwrap().len();
        assert!(size <= MAX_SETTINGS_BODY_BYTES, "{size}");

        // A previewed query fits its parameter, which is at least as long as its JSON.
        let query = rssr_domain::EntryQuery {
            search_title: Some("x".repeat(MAX_QUERY_PARAMETER_BYTES)),
            ..Default::default()
        };
        let preview = rssr_domain::MarkReadPreview {
            query,
            unread_entry_ids: vec![i64::MIN; rssr_domain::MAX_MARK_READ_SELECTION],
        };
        let size = serde_json::to_vec(&preview).unwrap().len();
        assert!(size <= MAX_MARK_READ_BODY_BYTES, "{size}");
    }

    #[test]
    fn job_ids_are_32_lowercase_hex_digits() {
        assert!(JobId::parse("0123456789abcdef0123456789abcdef").is_some());
        for bad in ["", "0123456789ABCDEF0123456789ABCDEF", "0123456789abcdef", "../../etc/passwd"]
        {
            assert!(JobId::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn job_status_on_the_wire() {
        assert_eq!(serde_json::to_value(JobStatus::Queued).unwrap(), json!({ "state": "queued" }));
        let done = JobStatus::Succeeded {
            result: JobResult::RemoteConfigPush(RemoteConfigPushOutcome { exported_feed_count: 3 }),
        };
        let encoded = serde_json::to_value(&done).unwrap();
        assert_eq!(
            encoded,
            json!({
                "state": "succeeded",
                "result": { "kind": "remote_config_push", "outcome": { "exported_feed_count": 3 } }
            })
        );
        assert_eq!(serde_json::from_value::<JobStatus>(encoded).unwrap(), done);
        let failed = JobStatus::Failed {
            error: BackendError {
                code: ErrorCode::Network,
                message: "无法连接".into(),
                request_id: Some("r1".into()),
            },
        };
        let encoded = serde_json::to_value(&failed).unwrap();
        assert_eq!(encoded["error"]["code"], "network");
        assert_eq!(serde_json::from_value::<JobStatus>(encoded).unwrap(), failed);
        assert!(failed.is_final() && !JobStatus::Running.is_final());
    }
}
