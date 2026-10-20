//! The error every [`crate::Backend`] operation returns: a stable code, a message for the
//! user, and a request id to match server logs (`docs/design/v0.2-phase-b2-contracts.md`, D5).

use std::fmt;

use rssr_application::{ErrorKind, classify};

/// Stable error codes. The first seven come from the use cases; the rest are the HTTP
/// protocol's own (v0.2 phase C, `crate::protocol`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidInput,
    NotFound,
    /// The data changed since it was read; read again before retrying.
    Conflict,
    Network,
    /// Busy or timed out; retrying later may work.
    Unavailable,
    Storage,
    Internal,
    /// No valid session: log in again.
    Unauthenticated,
    /// A write from an origin the server does not trust.
    UntrustedOrigin,
    /// A write without the session's CSRF token.
    CsrfMismatch,
    /// A write from a page of another build: reload the page.
    BuildMismatch,
    /// The request (its body or query) exceeds the protocol's limits.
    TooLarge,
    /// A job's status is gone: it finished long ago, results were evicted, or the server
    /// restarted. Read the affected state again.
    StatusUnavailable,
    /// A write may or may not have taken effect: the client sent it and got no complete answer,
    /// or the server could not confirm it (an image localization cut off by its time limit while
    /// it wrote). Read the affected state again; never send the write again on its own (phase C,
    /// "结果未知").
    OutcomeUnknown,
}

impl ErrorCode {
    /// The code as it appears on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Network => "network",
            Self::Unavailable => "unavailable",
            Self::Storage => "storage",
            Self::Internal => "internal",
            Self::Unauthenticated => "unauthenticated",
            Self::UntrustedOrigin => "untrusted_origin",
            Self::CsrfMismatch => "csrf_mismatch",
            Self::BuildMismatch => "build_mismatch",
            Self::TooLarge => "too_large",
            Self::StatusUnavailable => "status_unavailable",
            Self::OutcomeUnknown => "outcome_unknown",
        }
    }

    /// The HTTP status a response with this code carries.
    pub fn http_status(self) -> u16 {
        match self {
            Self::InvalidInput => 400,
            Self::Unauthenticated => 401,
            Self::UntrustedOrigin | Self::CsrfMismatch => 403,
            Self::NotFound => 404,
            Self::Conflict | Self::BuildMismatch => 409,
            Self::StatusUnavailable => 410,
            Self::TooLarge => 413,
            Self::Storage | Self::Internal => 500,
            Self::Network => 502,
            Self::Unavailable => 503,
            Self::OutcomeUnknown => 504,
        }
    }
}

impl From<ErrorKind> for ErrorCode {
    fn from(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::InvalidInput => Self::InvalidInput,
            ErrorKind::NotFound => Self::NotFound,
            ErrorKind::Conflict => Self::Conflict,
            ErrorKind::Network => Self::Network,
            ErrorKind::Unavailable => Self::Unavailable,
            ErrorKind::Storage => Self::Storage,
            ErrorKind::Internal => Self::Internal,
        }
    }
}

/// A failed operation as the user interface sees it. Its `Display` is the message. On the wire:
/// `{ "code": "...", "message": "...", "request_id": null | "..." }`, plus `"reason": "..."` when
/// there is one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackendError {
    pub code: ErrorCode,
    /// For the user; internal details stay in the log.
    pub message: String,
    /// Matches the server's log entry; `None` for an in-process backend.
    pub request_id: Option<String>,
    /// Why the outcome is unknown, when that matters to the user interface: only on
    /// [`ErrorCode::OutcomeUnknown`], and only [`ErrorCode::Unauthenticated`] so far (the session
    /// ended after the server accepted a job, so the page must also sign in again).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<ErrorCode>,
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BackendError {}

impl BackendError {
    /// An error of the protocol itself (not of a use case), for the user.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), request_id: None, reason: None }
    }

    /// The same error, with the reason its outcome is unknown.
    pub fn with_reason(mut self, reason: ErrorCode) -> Self {
        self.reason = Some(reason);
        self
    }

    /// The same error, matched to the server's log entry `request_id`.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

/// Classified once, where the error leaves the use cases (application's `classify`, which also
/// logs what it does not show).
impl From<anyhow::Error> for BackendError {
    fn from(error: anyhow::Error) -> Self {
        let public = classify(&error);
        Self { code: public.kind.into(), message: public.message, request_id: None, reason: None }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use rssr_domain::DomainError;

    use serde_json::json;

    use super::{BackendError, ErrorCode};

    #[test]
    fn an_error_keeps_its_code_through_contexts() {
        let error: BackendError = Err::<(), _>(DomainError::Conflict("配置已被修改".into()))
            .context("替换配置失败")
            .unwrap_err()
            .into();
        assert_eq!(error.code, ErrorCode::Conflict);
        assert_eq!(error.to_string(), "配置已被修改");
        assert_eq!(error.code.as_str(), "conflict");
        assert_eq!(error.request_id, None);
    }

    #[test]
    fn the_wire_form_is_code_message_and_request_id() {
        let error = BackendError::new(ErrorCode::Conflict, "配置已被修改");
        let encoded = serde_json::to_value(&error).unwrap();
        assert_eq!(
            encoded,
            json!({ "code": "conflict", "message": "配置已被修改", "request_id": null })
        );
        assert_eq!(serde_json::from_value::<BackendError>(encoded).unwrap(), error);
        for code in [
            ErrorCode::InvalidInput,
            ErrorCode::NotFound,
            ErrorCode::Conflict,
            ErrorCode::Network,
            ErrorCode::Unavailable,
            ErrorCode::Storage,
            ErrorCode::Internal,
            ErrorCode::Unauthenticated,
            ErrorCode::UntrustedOrigin,
            ErrorCode::CsrfMismatch,
            ErrorCode::BuildMismatch,
            ErrorCode::TooLarge,
            ErrorCode::StatusUnavailable,
            ErrorCode::OutcomeUnknown,
        ] {
            assert_eq!(serde_json::to_value(code).unwrap(), json!(code.as_str()));
            assert!((400..600).contains(&code.http_status()), "{code:?}");
        }
        let error =
            BackendError::new(ErrorCode::BuildMismatch, "请重新加载页面").with_request_id("r9");
        assert_eq!(error.request_id.as_deref(), Some("r9"));
        assert_eq!(error.code.http_status(), 409);

        // A reason appears only when there is one, and reads back.
        let error = BackendError::new(ErrorCode::OutcomeUnknown, "结果未知")
            .with_reason(ErrorCode::Unauthenticated);
        let encoded = serde_json::to_value(&error).unwrap();
        assert_eq!(encoded["reason"], "unauthenticated");
        assert_eq!(serde_json::from_value::<BackendError>(encoded).unwrap(), error);
    }
}
